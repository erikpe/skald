//! Complete and exclusive ownership of files in the spec-driven fixture tree.

use super::{
    model::{
        PlannedLeafKind, ResolvedArgs, ResolvedByteSource, ResolvedStreamExpectation,
        ResolvedWorkingDirectory, TestPlan,
    },
    PlanError,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

pub(super) fn validate(plan: &TestPlan) -> Result<(), PlanError> {
    let mut files = BTreeMap::<PathBuf, BTreeSet<String>>::new();
    let mut roots = Vec::<(PathBuf, String)>::new();
    collect_plan_ownership(plan, &mut files, &mut roots);

    let mut candidates = Vec::new();
    collect_candidates(plan.golden_root(), plan.golden_root(), &mut candidates)?;
    let mut unowned = Vec::new();
    let mut multiply_owned = Vec::new();
    for candidate in candidates {
        let mut owners = files.get(&candidate).cloned().unwrap_or_default();
        for (root, owner) in &roots {
            if candidate.starts_with(root) {
                owners.insert(owner.clone());
            }
        }
        match owners.len() {
            0 => unowned.push(relative(plan, &candidate)),
            1 => {}
            _ => multiply_owned.push((relative(plan, &candidate), owners)),
        }
    }

    if unowned.is_empty() && multiply_owned.is_empty() {
        return Ok(());
    }

    let mut message = String::from("golden fixture ownership audit failed");
    if !unowned.is_empty() {
        message.push_str("\nunreferenced fixture candidates:");
        for path in unowned {
            message.push_str("\n  ");
            message.push_str(&path);
        }
    }
    if !multiply_owned.is_empty() {
        message.push_str("\nfixture candidates owned by multiple specs:");
        for (path, owners) in multiply_owned {
            message.push_str("\n  ");
            message.push_str(&path);
            message.push_str(": ");
            message.push_str(&owners.into_iter().collect::<Vec<_>>().join(", "));
        }
    }
    Err(PlanError::at_path(plan.golden_root(), message))
}

fn collect_plan_ownership(
    plan: &TestPlan,
    files: &mut BTreeMap<PathBuf, BTreeSet<String>>,
    roots: &mut Vec<(PathBuf, String)>,
) {
    let test_specs = plan
        .tests()
        .iter()
        .map(|test| (test.id(), test.spec_id()))
        .collect::<BTreeMap<_, _>>();

    for test in plan.tests() {
        if let Some(source) = test.source() {
            own_file(files, source, test.spec_id());
        }
    }
    for build in plan.builds() {
        let owner = test_specs[build.test_id()];
        for pair in build.base_args().windows(2) {
            if matches!(pair[0].to_str(), Some("--module-root" | "--stdlib-root")) {
                roots.push((PathBuf::from(&pair[1]), owner.to_owned()));
            }
        }
    }
    for leaf in plan.leaves() {
        match leaf.kind() {
            PlannedLeafKind::Compile(expectation) => {
                own_stream(files, expectation.stderr(), leaf.spec_id());
            }
            PlannedLeafKind::Run(run) => {
                if let ResolvedArgs::File(path) = run.args() {
                    own_file(files, path, leaf.spec_id());
                }
                own_bytes(files, run.stdin(), leaf.spec_id());
                for input in run.input_files() {
                    own_bytes(files, input.contents(), leaf.spec_id());
                }
                if let ResolvedWorkingDirectory::Fixture(path) = run.cwd() {
                    roots.push((path.clone(), leaf.spec_id().to_owned()));
                }
                own_stream(files, run.expectation().stdout(), leaf.spec_id());
                own_stream(files, run.expectation().stderr(), leaf.spec_id());
                for output in run.expectation().output_files() {
                    own_bytes(files, output.contents(), leaf.spec_id());
                }
            }
        }
    }
}

fn own_stream(
    files: &mut BTreeMap<PathBuf, BTreeSet<String>>,
    stream: &ResolvedStreamExpectation,
    owner: &str,
) {
    if let Some(source) = stream.expected() {
        own_bytes(files, source, owner);
    }
}

fn own_bytes(
    files: &mut BTreeMap<PathBuf, BTreeSet<String>>,
    source: &ResolvedByteSource,
    owner: &str,
) {
    if let ResolvedByteSource::File(path) = source {
        own_file(files, path, owner);
    }
}

fn own_file(files: &mut BTreeMap<PathBuf, BTreeSet<String>>, path: &Path, owner: &str) {
    files
        .entry(path.to_owned())
        .or_default()
        .insert(owner.to_owned());
}

fn collect_candidates(
    golden_root: &Path,
    directory: &Path,
    candidates: &mut Vec<PathBuf>,
) -> Result<(), PlanError> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| {
            PlanError::at_path(
                directory,
                format!("could not audit golden fixture directory: {error}"),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            PlanError::at_path(
                directory,
                format!("could not read golden fixture entry: {error}"),
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let relative = path
            .strip_prefix(golden_root)
            .expect("audited path should remain below the golden root");
        if relative
            .components()
            .next()
            .is_some_and(|component| component.as_os_str() == OsStr::new("oracles"))
        {
            continue;
        }
        let file_type = entry.file_type().map_err(|error| {
            PlanError::at_path(&path, format!("could not inspect golden fixture: {error}"))
        })?;
        if file_type.is_dir() {
            collect_candidates(golden_root, &path, candidates)?;
        } else if file_type.is_file() && is_fixture_candidate(&path) {
            candidates.push(path);
        }
    }
    Ok(())
}

fn is_fixture_candidate(path: &Path) -> bool {
    let name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
    name != "README.md" && name != "config.toml" && !name.ends_with(".golden.toml")
}

fn relative(plan: &TestPlan, path: &Path) -> String {
    path.strip_prefix(plan.golden_root())
        .expect("owned fixture should remain below golden root")
        .to_string_lossy()
        .replace('\\', "/")
}
