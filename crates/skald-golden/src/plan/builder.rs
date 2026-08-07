use super::{
    identity::{artifact_name, slash_path, spec_id},
    model::{
        os_strings, PlannedBuild, PlannedLeaf, PlannedLeafKind, PlannedRun, PlannedSpec,
        PlannedTest, ResolvedArgs, ResolvedCompileExpectation, ResolvedInputFile,
        ResolvedOutputFile, ResolvedRunExpectation, ResolvedStreamExpectation,
        ResolvedWorkingDirectory, TestPlan,
    },
    paths::FixturePaths,
    PlanError,
};
use crate::{
    discovery::DiscoveredSuite, ArgSource, MatchMode, Run, StreamExpectation, Test, TestKind,
    WorkingDirectory,
};
use std::{
    collections::HashMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

pub(super) fn build(
    discovered: DiscoveredSuite,
    artifact_root: &Path,
    command_line_compiler_args: &[OsString],
) -> Result<TestPlan, PlanError> {
    let artifact_root = absolute_path(artifact_root)?;
    let mut plan = TestPlan {
        golden_root: discovered.root.clone(),
        artifact_root,
        specs: Vec::new(),
        tests: Vec::new(),
        builds: Vec::new(),
        leaves: Vec::new(),
    };
    let mut spec_ids = HashMap::new();
    let mut test_ids = HashMap::new();
    let mut build_ids = HashMap::new();
    let mut leaf_ids = HashMap::new();
    let mut artifact_names = HashMap::new();

    for discovered_spec in discovered.specs {
        let spec_id = spec_id(&discovered_spec.relative_path)?;
        insert_unique(&mut spec_ids, &spec_id, &discovered_spec.path, "spec ID")?;
        let spec_relative_path = slash_path(&discovered_spec.relative_path)?;
        plan.specs.push(PlannedSpec {
            id: spec_id.clone(),
            path: discovered_spec.path.clone(),
            relative_path: spec_relative_path.clone(),
        });

        let paths = FixturePaths::new(&discovered.root, &discovered_spec.path);
        for (test_index, test) in discovered_spec.spec.tests().iter().enumerate() {
            let test_field = format!("test[{test_index}]");
            let test_id = format!("{spec_id}::{}", test.name());
            insert_unique(&mut test_ids, &test_id, &discovered_spec.path, "test ID")?;
            let (source, source_relative) = resolve_source(&paths, test, &test_field)?;
            let mut planned_test = PlannedTest {
                id: test_id.clone(),
                spec_id: spec_id.clone(),
                name: test.name().to_owned(),
                source,
                source_relative: source_relative.clone(),
                build_ids: Vec::new(),
            };

            for (variant_index, variant_name) in test.variants().iter().enumerate() {
                let variant = discovered
                    .config
                    .variants()
                    .get(variant_name)
                    .ok_or_else(|| {
                        PlanError::at_field(
                            &discovered_spec.path,
                            format!("{test_field}.variants[{variant_index}]"),
                            format!("unknown repository variant {variant_name:?}"),
                        )
                    })?;
                let build_id = format!("{test_id}::{variant_name}");
                insert_unique(&mut build_ids, &build_id, &discovered_spec.path, "build ID")?;
                let artifact_name = artifact_name(&build_id);
                insert_unique(
                    &mut artifact_names,
                    &artifact_name,
                    &discovered_spec.path,
                    "artifact directory name",
                )?;

                let mut base_args = Vec::new();
                if let Some(source) = &planned_test.source {
                    base_args.push(source.as_os_str().to_owned());
                }
                base_args.extend(paths.compiler_args(
                    &os_strings(test.compiler_args()),
                    &format!("{test_field}.compiler_args"),
                )?);
                let variant_args = paths.compiler_args(
                    &os_strings(variant.compiler_args()),
                    &format!("variant.{variant_name}.compiler_args"),
                )?;
                let command_line_args = paths
                    .compiler_args(command_line_compiler_args, "command_line.compiler_args")?;
                let compiler_args = base_args
                    .iter()
                    .chain(&variant_args)
                    .chain(&command_line_args)
                    .cloned()
                    .collect();

                let mut build = PlannedBuild {
                    id: build_id.clone(),
                    test_id: test_id.clone(),
                    variant: variant_name.clone(),
                    compiler_args,
                    base_args,
                    variant_args,
                    command_line_args,
                    compiler_working_directory: None,
                    artifact_directory: plan.artifact_root.join(artifact_name),
                    timeout_seconds: test.timeout_seconds(),
                    serial: test.serial(),
                    resources: test.resources().to_vec(),
                    leaf_ids: Vec::new(),
                };

                match test.kind() {
                    TestKind::Run(run_test) => {
                        for (run_index, run) in run_test.runs().iter().enumerate() {
                            let leaf_id = format!("{build_id}::{}", run.name());
                            insert_unique(
                                &mut leaf_ids,
                                &leaf_id,
                                &discovered_spec.path,
                                "leaf ID",
                            )?;
                            build.leaf_ids.push(leaf_id.clone());
                            plan.leaves.push(PlannedLeaf {
                                id: leaf_id,
                                spec_id: spec_id.clone(),
                                spec_relative_path: spec_relative_path.clone(),
                                test_id: test_id.clone(),
                                build_id: build_id.clone(),
                                variant: variant_name.clone(),
                                source_relative: source_relative.clone(),
                                kind: PlannedLeafKind::Run(Box::new(resolve_run(
                                    &paths,
                                    run,
                                    &format!("{test_field}.run[{run_index}]"),
                                )?)),
                            });
                        }
                    }
                    TestKind::CompileFail(compile) => {
                        let leaf_id = format!("{build_id}::<compile>");
                        insert_unique(&mut leaf_ids, &leaf_id, &discovered_spec.path, "leaf ID")?;
                        build.leaf_ids.push(leaf_id.clone());
                        plan.leaves.push(PlannedLeaf {
                            id: leaf_id,
                            spec_id: spec_id.clone(),
                            spec_relative_path: spec_relative_path.clone(),
                            test_id: test_id.clone(),
                            build_id: build_id.clone(),
                            variant: variant_name.clone(),
                            source_relative: source_relative.clone(),
                            kind: PlannedLeafKind::Compile(ResolvedCompileExpectation {
                                stderr: resolve_stream(
                                    &paths,
                                    compile.expectation().stderr(),
                                    &format!("{test_field}.expect.stderr"),
                                    true,
                                )?,
                                stderr_prefix_to_strip: None,
                            }),
                        });
                    }
                }
                build.leaf_ids.sort();
                planned_test.build_ids.push(build_id);
                plan.builds.push(build);
            }
            planned_test.build_ids.sort();
            plan.tests.push(planned_test);
        }
    }

    super::legacy::append(
        &mut plan,
        discovered.legacy_cases,
        command_line_compiler_args,
        super::legacy::IdentityMaps {
            specs: &mut spec_ids,
            tests: &mut test_ids,
            builds: &mut build_ids,
            leaves: &mut leaf_ids,
            artifacts: &mut artifact_names,
        },
    )?;

    plan.specs.sort_by(|left, right| left.id.cmp(&right.id));
    plan.tests.sort_by(|left, right| left.id.cmp(&right.id));
    plan.builds.sort_by(|left, right| left.id.cmp(&right.id));
    plan.leaves.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(plan)
}

fn resolve_source(
    paths: &FixturePaths<'_>,
    test: &Test,
    field: &str,
) -> Result<(Option<PathBuf>, Option<String>), PlanError> {
    let Some(source) = test.source() else {
        return Ok((None, None));
    };
    let source = paths.file(source, &format!("{field}.source"))?;
    let relative = paths.relative(&source)?;
    Ok((Some(source), Some(relative)))
}

fn resolve_run(paths: &FixturePaths<'_>, run: &Run, field: &str) -> Result<PlannedRun, PlanError> {
    let args = match run.args() {
        ArgSource::Utf8(arguments) => ResolvedArgs::Utf8(arguments.clone()),
        ArgSource::File(file) => {
            ResolvedArgs::File(paths.file(file, &format!("{field}.argv_file"))?)
        }
    };
    let stdin = paths.byte_source(run.stdin(), &format!("{field}.stdin"))?;
    let input_files = run
        .input_files()
        .iter()
        .enumerate()
        .map(|(index, file)| {
            Ok(ResolvedInputFile {
                name: file.name().to_owned(),
                contents: paths.byte_source(
                    file.contents(),
                    &format!("{field}.input_files[{index}].contents"),
                )?,
            })
        })
        .collect::<Result<Vec<_>, PlanError>>()?;
    let cwd = match run.cwd() {
        WorkingDirectory::Private => ResolvedWorkingDirectory::Private,
        WorkingDirectory::Fixture(directory) => ResolvedWorkingDirectory::Fixture(
            paths.directory(directory, &format!("{field}.cwd.fixture"))?,
        ),
    };
    let expectation = run.expectation();
    let output_files = expectation
        .output_files()
        .iter()
        .enumerate()
        .map(|(index, file)| {
            Ok(ResolvedOutputFile {
                name: file.name().to_owned(),
                contents: paths.byte_source(
                    file.contents(),
                    &format!("{field}.expect.output_files[{index}].contents"),
                )?,
            })
        })
        .collect::<Result<Vec<_>, PlanError>>()?;

    Ok(PlannedRun {
        name: run.name().to_owned(),
        args,
        stdin,
        input_files,
        cwd,
        env: run.env().clone(),
        timeout_seconds: run.timeout_seconds(),
        serial: run.serial(),
        resources: run.resources().to_vec(),
        expectation: ResolvedRunExpectation {
            exit: expectation.exit(),
            stdout: resolve_stream(
                paths,
                expectation.stdout(),
                &format!("{field}.expect.stdout"),
                false,
            )?,
            stderr: resolve_stream(
                paths,
                expectation.stderr(),
                &format!("{field}.expect.stderr"),
                false,
            )?,
            output_files,
        },
    })
}

fn resolve_stream(
    paths: &FixturePaths<'_>,
    expectation: &StreamExpectation,
    field: &str,
    require_nonempty: bool,
) -> Result<ResolvedStreamExpectation, PlanError> {
    match expectation {
        StreamExpectation::Ignore => Ok(ResolvedStreamExpectation::Ignore),
        StreamExpectation::Match { mode, expected } => {
            let expected = paths.byte_source(expected, field)?;
            if (require_nonempty || *mode != MatchMode::Exact)
                && matches!(&expected, super::model::ResolvedByteSource::File(path) if file_is_empty(path)?)
            {
                return Err(PlanError::at_field(
                    paths.spec_path(),
                    field,
                    "expected byte file must not be empty for this match policy",
                ));
            }
            Ok(ResolvedStreamExpectation::Match {
                mode: *mode,
                expected,
            })
        }
    }
}

fn file_is_empty(path: &Path) -> Result<bool, PlanError> {
    fs::metadata(path)
        .map(|metadata| metadata.len() == 0)
        .map_err(|error| {
            PlanError::at_path(path, format!("could not inspect expected data: {error}"))
        })
}

pub(super) fn insert_unique(
    seen: &mut HashMap<String, PathBuf>,
    id: &str,
    owner: &Path,
    kind: &str,
) -> Result<(), PlanError> {
    if let Some(previous) = seen.insert(id.to_owned(), owner.to_owned()) {
        Err(PlanError::at_path(
            owner,
            format!(
                "duplicate {kind} {id:?}; it is already owned by {}",
                previous.display()
            ),
        ))
    } else {
        Ok(())
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf, PlanError> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(path))
            .map_err(|error| {
                PlanError::message(format!("could not resolve artifact root: {error}"))
            })
    }
}
