//! Versioned corpus parsing, golden-plan reuse, and canonical identities.

use serde::Deserialize;
use skald_golden::{
    build_plan, decode_arguments, load_bytes, PlannedLeafKind, ResolvedByteSource, TestPlan,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fmt, fs,
    os::unix::ffi::OsStrExt,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Corpus {
    pub(crate) name: String,
    pub(crate) version: u64,
    pub(crate) workloads: Vec<Workload>,
}

impl Corpus {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn workloads(&self) -> &[Workload] {
        &self.workloads
    }

    pub fn retain_ids(&mut self, ids: &BTreeSet<String>) -> Result<(), CorpusError> {
        let known = self
            .workloads
            .iter()
            .map(|workload| workload.id.clone())
            .collect::<BTreeSet<_>>();
        if let Some(unknown) = ids.difference(&known).next() {
            return Err(CorpusError::new(format!(
                "unknown workload {unknown:?}; selection must name a manifest workload"
            )));
        }
        self.workloads.retain(|workload| ids.contains(&workload.id));
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Workload {
    pub(crate) id: String,
    pub(crate) category: String,
    pub(crate) kind: WorkloadKind,
    pub(crate) identity: String,
    pub(crate) entry: PathBuf,
    pub(crate) entry_relative: String,
    pub(crate) native_runs: Vec<NativeRun>,
}

impl Workload {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn category(&self) -> &str {
        &self.category
    }

    pub fn entry(&self) -> &Path {
        &self.entry
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkloadKind {
    Golden { build: String },
    Explicit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeRun {
    pub(crate) identity: String,
    pub(crate) arguments: Vec<ByteString>,
    pub(crate) stdin: InputBytes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ByteString {
    Utf8(String),
    Hex(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InputBytes {
    pub(crate) origin: &'static str,
    pub(crate) path: Option<String>,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug)]
pub struct CorpusError {
    message: String,
}

impl CorpusError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CorpusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CorpusError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u64,
    name: String,
    version: u64,
    #[serde(default)]
    workload: Vec<ManifestWorkload>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestWorkload {
    id: String,
    category: String,
    golden_build: Option<String>,
    #[serde(default)]
    golden_runs: Vec<String>,
    entry: Option<PathBuf>,
}

pub fn load_corpus(
    repository_root: impl AsRef<Path>,
    manifest_path: impl AsRef<Path>,
) -> Result<Corpus, CorpusError> {
    let repository_root = canonical_directory(repository_root.as_ref(), "repository root")?;
    let manifest_path = contained_file(&repository_root, manifest_path.as_ref(), "manifest")?;
    let contents = fs::read_to_string(&manifest_path).map_err(|error| {
        CorpusError::new(format!(
            "could not read corpus manifest {}: {error}",
            manifest_path.display()
        ))
    })?;
    let manifest: Manifest = toml::from_str(&contents).map_err(|error| {
        CorpusError::new(format!(
            "invalid corpus manifest {}: {error}",
            manifest_path.display()
        ))
    })?;
    if manifest.schema != 1 {
        return Err(CorpusError::new(format!(
            "unsupported corpus schema {}; expected 1",
            manifest.schema
        )));
    }
    validate_name(&manifest.name, "corpus name")?;
    if manifest.version == 0 {
        return Err(CorpusError::new("corpus version must be positive"));
    }

    let golden_root = repository_root.join("tests/golden");
    let plan = build_plan(
        &golden_root,
        repository_root.join("build/golden/cases"),
        &[],
    )
    .map_err(|error| CorpusError::new(format!("could not validate golden plan: {error}")))?;
    resolve_manifest(repository_root, manifest, &plan)
}

fn resolve_manifest(
    repository_root: PathBuf,
    manifest: Manifest,
    golden_plan: &TestPlan,
) -> Result<Corpus, CorpusError> {
    let mut ids = BTreeSet::new();
    let mut identities = BTreeMap::new();
    let mut workloads = Vec::with_capacity(manifest.workload.len());
    for raw in manifest.workload {
        validate_name(&raw.id, "workload ID")?;
        validate_name(&raw.category, "workload category")?;
        if !ids.insert(raw.id.clone()) {
            return Err(CorpusError::new(format!(
                "duplicate workload ID {:?}",
                raw.id
            )));
        }
        let golden_build = raw.golden_build.clone();
        let explicit_entry = raw.entry.clone();
        let workload = match (golden_build.as_deref(), explicit_entry.as_deref()) {
            (Some(build), None) => resolve_golden(&repository_root, raw, build, golden_plan)?,
            (None, Some(entry)) if raw.golden_runs.is_empty() => {
                resolve_explicit(&repository_root, raw, entry)?
            }
            (Some(_), Some(_)) => {
                return Err(CorpusError::new(format!(
                    "workload {:?} must select exactly one of golden_build or entry",
                    raw.id
                )))
            }
            (None, None) => {
                return Err(CorpusError::new(format!(
                    "workload {:?} must select golden_build or entry",
                    raw.id
                )))
            }
            (None, Some(_)) => {
                return Err(CorpusError::new(format!(
                    "explicit workload {:?} cannot select golden runs",
                    raw.id
                )))
            }
        };
        if let Some(previous) = identities.insert(workload.identity.clone(), workload.id.clone()) {
            return Err(CorpusError::new(format!(
                "workloads {previous:?} and {:?} have the same canonical compilation identity",
                workload.id
            )));
        }
        workloads.push(workload);
    }
    Ok(Corpus {
        name: manifest.name,
        version: manifest.version,
        workloads,
    })
}

fn resolve_golden(
    repository_root: &Path,
    raw: ManifestWorkload,
    build_id: &str,
    plan: &TestPlan,
) -> Result<Workload, CorpusError> {
    let build = plan.build(build_id).ok_or_else(|| {
        CorpusError::new(format!(
            "workload {:?} names unknown golden build {build_id:?}",
            raw.id
        ))
    })?;
    if build.variant() != "default" {
        return Err(CorpusError::new(format!(
            "workload {:?} must use the frozen default golden variant",
            raw.id
        )));
    }
    let test = plan
        .test(build.test_id())
        .expect("a validated golden build must reference its test");
    let entry = test.source().ok_or_else(|| {
        CorpusError::new(format!(
            "golden build {build_id:?} does not have a source entry"
        ))
    })?;
    if build.compiler_args() != [entry.as_os_str().to_owned()] {
        return Err(CorpusError::new(format!(
            "golden build {build_id:?} has compiler arguments outside the frozen default measurement configuration"
        )));
    }
    let entry = contained_file(repository_root, entry, "golden source")?;
    let entry_relative = relative_slash(repository_root, &entry)?;
    let native_runs = resolve_native_runs(repository_root, &raw, build.leaf_ids(), plan)?;
    Ok(Workload {
        id: raw.id,
        category: raw.category,
        kind: WorkloadKind::Golden {
            build: build_id.to_owned(),
        },
        identity: compilation_identity(&entry_relative),
        entry,
        entry_relative,
        native_runs,
    })
}

fn resolve_native_runs(
    repository_root: &Path,
    raw: &ManifestWorkload,
    leaf_ids: &[String],
    plan: &TestPlan,
) -> Result<Vec<NativeRun>, CorpusError> {
    let mut available = BTreeMap::new();
    for leaf_id in leaf_ids {
        let leaf = plan
            .leaf(leaf_id)
            .expect("a validated build must reference existing leaves");
        if let PlannedLeafKind::Run(run) = leaf.kind() {
            available.insert(run.name(), (leaf.id(), run.as_ref()));
        }
    }
    let mut selected = BTreeSet::new();
    let mut runs = Vec::with_capacity(raw.golden_runs.len());
    for name in &raw.golden_runs {
        if !selected.insert(name.as_str()) {
            return Err(CorpusError::new(format!(
                "workload {:?} selects golden run {name:?} more than once",
                raw.id
            )));
        }
        let (identity, run) = available.get(name.as_str()).ok_or_else(|| {
            CorpusError::new(format!(
                "workload {:?} names unknown run {name:?} for its golden build",
                raw.id
            ))
        })?;
        let arguments = decode_arguments(run.args())
            .map_err(|error| CorpusError::new(format!("could not load run arguments: {error}")))?
            .iter()
            .map(encode_os_string)
            .collect();
        let (origin, path) = match run.stdin() {
            ResolvedByteSource::Inline(_) => ("inline", None),
            ResolvedByteSource::File(path) => {
                ("file", Some(relative_slash(repository_root, path)?))
            }
        };
        let bytes = load_bytes(run.stdin())
            .map_err(|error| CorpusError::new(format!("could not load run stdin: {error}")))?;
        runs.push(NativeRun {
            identity: (*identity).to_owned(),
            arguments,
            stdin: InputBytes {
                origin,
                path,
                bytes,
            },
        });
    }
    Ok(runs)
}

fn resolve_explicit(
    repository_root: &Path,
    raw: ManifestWorkload,
    entry: &Path,
) -> Result<Workload, CorpusError> {
    let entry = contained_file(repository_root, entry, "explicit entry")?;
    let entry_relative = relative_slash(repository_root, &entry)?;
    Ok(Workload {
        id: raw.id,
        category: raw.category,
        kind: WorkloadKind::Explicit,
        identity: compilation_identity(&entry_relative),
        entry,
        entry_relative,
        native_runs: Vec::new(),
    })
}

fn compilation_identity(entry: &str) -> String {
    format!(
        "entry={entry};providers=[];stdlib=repository;target=x86_64-sysv;trace=omitted;mir=default;exclusions=[];args=[]"
    )
}

fn canonical_directory(path: &Path, description: &str) -> Result<PathBuf, CorpusError> {
    let path = fs::canonicalize(path).map_err(|error| {
        CorpusError::new(format!(
            "could not canonicalize {description} {}: {error}",
            path.display()
        ))
    })?;
    if !path.is_dir() {
        return Err(CorpusError::new(format!(
            "{description} is not a directory: {}",
            path.display()
        )));
    }
    Ok(path)
}

fn contained_file(root: &Path, path: &Path, description: &str) -> Result<PathBuf, CorpusError> {
    if path.as_os_str().is_empty() {
        return Err(CorpusError::new(format!("{description} path is empty")));
    }
    let candidate = if path.is_absolute() {
        path.to_owned()
    } else {
        lexical_join(root, path, description)?
    };
    let canonical = fs::canonicalize(&candidate).map_err(|error| {
        CorpusError::new(format!(
            "could not canonicalize {description} {}: {error}",
            candidate.display()
        ))
    })?;
    if !canonical.starts_with(root) {
        return Err(CorpusError::new(format!(
            "{description} escapes the repository: {}",
            canonical.display()
        )));
    }
    if !canonical.is_file() {
        return Err(CorpusError::new(format!(
            "{description} is not a file: {}",
            canonical.display()
        )));
    }
    Ok(canonical)
}

fn lexical_join(root: &Path, relative: &Path, description: &str) -> Result<PathBuf, CorpusError> {
    let mut joined = root.to_owned();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => joined.push(part),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(CorpusError::new(format!(
                    "{description} must be a contained repository-relative path"
                )))
            }
        }
    }
    Ok(joined)
}

fn relative_slash(root: &Path, path: &Path) -> Result<String, CorpusError> {
    let relative = path.strip_prefix(root).map_err(|_| {
        CorpusError::new(format!("path escapes the repository: {}", path.display()))
    })?;
    let mut parts = Vec::new();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err(CorpusError::new(
                "stable path contains a non-normal component",
            ));
        };
        parts.push(
            part.to_str()
                .ok_or_else(|| CorpusError::new("stable paths must be UTF-8"))?,
        );
    }
    Ok(parts.join("/"))
}

fn validate_name(value: &str, description: &str) -> Result<(), CorpusError> {
    if value.is_empty()
        || value.starts_with('/')
        || value.ends_with('/')
        || value.split('/').any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        })
    {
        return Err(CorpusError::new(format!(
            "invalid {description} {value:?}; expected slash-separated ASCII name components"
        )));
    }
    Ok(())
}

fn encode_os_string(value: &OsString) -> ByteString {
    match value.to_str() {
        Some(value) => ByteString::Utf8(value.to_owned()),
        None => ByteString::Hex(hex(value.as_os_str().as_bytes())),
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests;
