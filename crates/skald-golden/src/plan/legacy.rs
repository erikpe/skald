//! Compatibility translation from sidecar fixtures into the shared plan.

use super::{
    builder::insert_unique,
    identity::{artifact_name, slash_path},
    model::{
        PlannedBuild, PlannedLeaf, PlannedLeafKind, PlannedRun, PlannedSpec, PlannedTest,
        ResolvedArgs, ResolvedByteSource, ResolvedCompileExpectation, ResolvedRunExpectation,
        ResolvedStreamExpectation, ResolvedWorkingDirectory, TestPlan,
    },
    PlanError,
};
use crate::{
    decode_arguments,
    discovery::{LegacyCase, LegacyKind},
    ExitExpectation, MatchMode,
};
use std::{
    collections::{BTreeMap, HashMap},
    ffi::OsString,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

pub(super) struct IdentityMaps<'a> {
    pub(super) specs: &'a mut HashMap<String, PathBuf>,
    pub(super) tests: &'a mut HashMap<String, PathBuf>,
    pub(super) builds: &'a mut HashMap<String, PathBuf>,
    pub(super) leaves: &'a mut HashMap<String, PathBuf>,
    pub(super) artifacts: &'a mut HashMap<String, PathBuf>,
}

pub(super) fn append(
    plan: &mut TestPlan,
    cases: Vec<LegacyCase>,
    command_line_compiler_args: &[OsString],
    identities: IdentityMaps<'_>,
) -> Result<(), PlanError> {
    for case in cases {
        let owner = &case.expectation_stem;
        let canonical_id = canonical_id(&case.relative_stem)?;
        insert_unique(identities.specs, &canonical_id, owner, "spec ID")?;
        insert_unique(identities.tests, &canonical_id, owner, "test ID")?;
        let build_id = format!("{canonical_id}::default");
        insert_unique(identities.builds, &build_id, owner, "build ID")?;
        let artifact = artifact_name(&build_id);
        insert_unique(
            identities.artifacts,
            &artifact,
            owner,
            "artifact directory name",
        )?;
        let leaf_suffix = match case.kind {
            LegacyKind::Run => "<run>",
            LegacyKind::CompileFail => "<compile>",
        };
        let leaf_id = format!("{build_id}::{leaf_suffix}");
        insert_unique(identities.leaves, &leaf_id, owner, "leaf ID")?;

        let relative_path = slash_path(&case.relative_stem)?;
        let (source, source_relative) = if case
            .expectation_stem
            .extension()
            .is_some_and(|value| value == "ska")
        {
            (
                Some(case.expectation_stem.clone()),
                Some(relative_path.clone()),
            )
        } else {
            (None, None)
        };
        plan.specs.push(PlannedSpec {
            id: canonical_id.clone(),
            path: case.expectation_stem.clone(),
            relative_path: relative_path.clone(),
        });
        plan.tests.push(PlannedTest {
            id: canonical_id.clone(),
            spec_id: canonical_id.clone(),
            name: canonical_id.clone(),
            source,
            source_relative: source_relative.clone(),
            build_ids: vec![build_id.clone()],
        });

        let base_args = case.compiler_args.clone();
        let command_line_args = command_line_compiler_args.to_vec();
        let compiler_args = base_args
            .iter()
            .chain(&command_line_args)
            .cloned()
            .collect();
        plan.builds.push(PlannedBuild {
            id: build_id.clone(),
            test_id: canonical_id.clone(),
            variant: "default".to_owned(),
            compiler_args,
            base_args,
            variant_args: Vec::new(),
            command_line_args,
            compiler_working_directory: Some(case.compiler_working_directory.clone()),
            artifact_directory: plan.artifact_root.join(artifact),
            timeout_seconds: None,
            serial: false,
            resources: Vec::new(),
            leaf_ids: vec![leaf_id.clone()],
        });

        let kind = match case.kind {
            LegacyKind::Run => PlannedLeafKind::Run(Box::new(load_run(&case)?)),
            LegacyKind::CompileFail => PlannedLeafKind::Compile(load_compile_fail(&case)?),
        };
        plan.leaves.push(PlannedLeaf {
            id: leaf_id,
            spec_id: canonical_id.clone(),
            spec_relative_path: relative_path,
            test_id: canonical_id,
            build_id,
            variant: "default".to_owned(),
            source_relative,
            kind,
        });
    }
    Ok(())
}

fn canonical_id(relative_stem: &Path) -> Result<String, PlanError> {
    slash_path(&relative_stem.with_extension(""))
}

fn load_run(case: &LegacyCase) -> Result<PlannedRun, PlanError> {
    let exit = load_exit(&case.expectation_stem)?;
    let args = optional_file(&case.expectation_stem, "argv")?
        .map_or(ResolvedArgs::Utf8(Vec::new()), ResolvedArgs::File);
    decode_arguments(&args).map_err(|error| {
        PlanError::at_path(
            case.expectation_stem.with_extension("argv"),
            error.to_string(),
        )
    })?;
    Ok(PlannedRun {
        name: "run".to_owned(),
        args,
        stdin: optional_bytes(&case.expectation_stem, "stdin")?,
        input_files: Vec::new(),
        cwd: ResolvedWorkingDirectory::Fixture(case.compiler_working_directory.clone()),
        env: BTreeMap::new(),
        timeout_seconds: None,
        serial: false,
        // A single-file case can reach anywhere below its repository cwd, so
        // directory-specific locks would not isolate it from a multi-file
        // case. One migration-only lock is the conservative compatible rule.
        resources: vec!["legacy-working-directory".to_owned()],
        expectation: ResolvedRunExpectation {
            exit,
            stdout: exact_optional_stream(&case.expectation_stem, "stdout")?,
            stderr: exact_optional_stream(&case.expectation_stem, "stderr")?,
            output_files: Vec::new(),
        },
    })
}

fn load_compile_fail(case: &LegacyCase) -> Result<ResolvedCompileExpectation, PlanError> {
    let path = case.expectation_stem.with_extension("stderr");
    require_file(&path, "legacy compile-fail stderr sidecar")?;
    Ok(ResolvedCompileExpectation {
        stderr: exact_file(path),
        stderr_prefix_to_strip: case.diagnostic_path_prefix.clone(),
    })
}

fn load_exit(stem: &Path) -> Result<ExitExpectation, PlanError> {
    let path = stem.with_extension("exit");
    let text = fs::read_to_string(&path).map_err(|error| {
        PlanError::at_path(
            &path,
            format!("could not read legacy exit sidecar: {error}"),
        )
    })?;
    let value = text.trim();
    if value == "failure" {
        return Ok(ExitExpectation::Failure);
    }
    let code = value.parse::<i32>().map_err(|error| {
        PlanError::at_path(&path, format!("invalid legacy exit status: {error}"))
    })?;
    if !(0..=255).contains(&code) {
        return Err(PlanError::at_path(
            &path,
            format!("legacy exit status {code} is outside 0..=255"),
        ));
    }
    Ok(ExitExpectation::Code(code))
}

fn exact_optional_stream(
    stem: &Path,
    extension: &str,
) -> Result<ResolvedStreamExpectation, PlanError> {
    Ok(ResolvedStreamExpectation::Match {
        mode: MatchMode::Exact,
        expected: optional_bytes(stem, extension)?,
    })
}

fn exact_file(path: PathBuf) -> ResolvedStreamExpectation {
    ResolvedStreamExpectation::Match {
        mode: MatchMode::Exact,
        expected: ResolvedByteSource::File(path),
    }
}

fn optional_bytes(stem: &Path, extension: &str) -> Result<ResolvedByteSource, PlanError> {
    Ok(optional_file(stem, extension)?.map_or_else(
        || ResolvedByteSource::Inline(String::new()),
        ResolvedByteSource::File,
    ))
}

fn optional_file(stem: &Path, extension: &str) -> Result<Option<PathBuf>, PlanError> {
    let path = stem.with_extension(extension);
    match fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() => Ok(Some(path)),
        Ok(_) => Err(PlanError::at_path(&path, "legacy sidecar is not a file")),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(PlanError::at_path(
            &path,
            format!("could not inspect legacy sidecar: {error}"),
        )),
    }
}

fn require_file(path: &Path, description: &str) -> Result<(), PlanError> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(PlanError::at_path(
            path,
            format!("{description} is not a file"),
        )),
        Err(error) => Err(PlanError::at_path(
            path,
            format!("could not read {description}: {error}"),
        )),
    }
}
