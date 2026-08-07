use super::{
    error::SpecError,
    model::{
        ArgSource, ByteSource, CompileExpectation, CompileFailTest, ExitExpectation, InputFile,
        MatchMode, OutputFileExpectation, RepositoryConfig, Run, RunExpectation, RunTest,
        SchemaVersion, Spec, StreamExpectation, Test, TestKind, Variant, WorkingDirectory,
    },
    raw::{
        RawByteSource, RawCompileExpectation, RawConfig, RawExitExpectation, RawExitName,
        RawInputFile, RawMatchMode, RawOutputFileExpectation, RawRun, RawRunExpectation, RawSpec,
        RawStreamExpectation, RawStreamMatcher, RawTest, RawVariant,
    },
};
use crate::{StreamMatcher, StreamMatcherSet};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

pub(super) fn validate_spec(path: &Path, raw: RawSpec) -> Result<Spec, SpecError> {
    let schema = validate_spec_schema(path, raw.schema)?;
    if raw.test.is_empty() {
        return Err(error(path, "test", "must contain at least one test"));
    }

    let mut names = HashSet::new();
    let mut tests = Vec::with_capacity(raw.test.len());
    for (index, test) in raw.test.into_iter().enumerate() {
        let field = format!("test[{index}]");
        require_unique_name(path, &field, &test.name, &mut names)?;
        tests.push(validate_test(path, &field, schema, test)?);
    }

    Ok(Spec { schema, tests })
}

pub(super) fn validate_config(path: &Path, raw: RawConfig) -> Result<RepositoryConfig, SpecError> {
    let schema = validate_config_schema(path, raw.schema)?;
    let mut variants = BTreeMap::new();

    for (name, variant) in raw.variant {
        require_name(path, &format!("variant.{name}"), &name)?;
        validate_strings_without_nul(
            path,
            &format!("variant.{name}.compiler_args"),
            &variant.compiler_args,
        )?;
        variants.insert(name, validate_variant(variant));
    }

    variants.entry("default".to_owned()).or_insert(Variant {
        compiler_args: Vec::new(),
    });

    Ok(RepositoryConfig { schema, variants })
}

fn validate_spec_schema(path: &Path, schema: u64) -> Result<SchemaVersion, SpecError> {
    match schema {
        1 => Ok(SchemaVersion::V1),
        2 => Ok(SchemaVersion::V2),
        version => Err(error(
            path,
            "schema",
            format!("unsupported schema version {version}; expected 1 or 2"),
        )),
    }
}

fn validate_config_schema(path: &Path, schema: u64) -> Result<SchemaVersion, SpecError> {
    match schema {
        1 => Ok(SchemaVersion::V1),
        version => Err(error(
            path,
            "schema",
            format!("unsupported configuration schema version {version}; expected 1"),
        )),
    }
}

fn validate_variant(raw: RawVariant) -> Variant {
    Variant {
        compiler_args: raw.compiler_args,
    }
}

fn validate_test(
    path: &Path,
    field: &str,
    schema: SchemaVersion,
    raw: RawTest,
) -> Result<Test, SpecError> {
    require_name(path, field, &raw.name)?;
    validate_strings_without_nul(path, &format!("{field}.compiler_args"), &raw.compiler_args)?;
    validate_entry_selection(path, field, raw.source.as_ref(), &raw.compiler_args)?;
    validate_timeout(path, &format!("{field}.timeout"), raw.timeout)?;

    let variants = match raw.variants {
        Some(variants) => {
            require_nonempty_collection(path, &format!("{field}.variants"), &variants)?;
            validate_unique_strings(path, &format!("{field}.variants"), &variants)?;
            for (index, variant) in variants.iter().enumerate() {
                require_name(path, &format!("{field}.variants[{index}]"), variant)?;
            }
            variants
        }
        None => vec!["default".to_owned()],
    };
    let resources = validate_resources(path, field, raw.resources)?;

    let kind = match raw.mode {
        super::raw::RawMode::Run => {
            if raw.expect.is_some() {
                return Err(error(
                    path,
                    format!("{field}.expect"),
                    "is only valid for compile-fail tests; put expectations on each run",
                ));
            }
            if raw.run.is_empty() {
                return Err(error(
                    path,
                    format!("{field}.run"),
                    "run tests must contain at least one named run",
                ));
            }

            let mut names = HashSet::new();
            let mut runs = Vec::with_capacity(raw.run.len());
            for (index, run) in raw.run.into_iter().enumerate() {
                let run_field = format!("{field}.run[{index}]");
                require_unique_name(path, &run_field, &run.name, &mut names)?;
                runs.push(validate_run(path, &run_field, schema, run)?);
            }
            TestKind::Run(RunTest { runs })
        }
        super::raw::RawMode::CompileFail => {
            if !raw.run.is_empty() {
                return Err(error(
                    path,
                    format!("{field}.run"),
                    "is not valid for compile-fail tests",
                ));
            }
            let expectation = raw.expect.ok_or_else(|| {
                error(
                    path,
                    format!("{field}.expect"),
                    "compile-fail tests require a stderr expectation",
                )
            })?;
            TestKind::CompileFail(CompileFailTest {
                expectation: validate_compile_expectation(path, field, schema, expectation)?,
            })
        }
    };

    Ok(Test {
        name: raw.name,
        source: raw.source,
        compiler_args: raw.compiler_args,
        variants,
        timeout_seconds: raw.timeout,
        serial: raw.serial,
        resources,
        kind,
    })
}

fn validate_entry_selection(
    path: &Path,
    field: &str,
    source: Option<&PathBuf>,
    compiler_args: &[String],
) -> Result<(), SpecError> {
    let logical_entry = compiler_args
        .first()
        .is_some_and(|argument| argument == "--entry");
    match (source, logical_entry) {
        (Some(source), false) if !source.as_os_str().is_empty() => Ok(()),
        (None, true) if compiler_args.get(1).is_some_and(|entry| !entry.is_empty()) => Ok(()),
        (Some(_), true) => Err(error(
            path,
            format!("{field}.source"),
            "cannot be combined with compiler_args beginning with --entry",
        )),
        (Some(_), false) => Err(error(path, format!("{field}.source"), "must not be empty")),
        (None, true) => Err(error(
            path,
            format!("{field}.compiler_args[1]"),
            "--entry requires a nonempty logical module name",
        )),
        (None, false) => Err(error(
            path,
            format!("{field}.source"),
            "exactly one entry form is required: source or compiler_args beginning with --entry",
        )),
    }
}

fn validate_run(
    path: &Path,
    field: &str,
    schema: SchemaVersion,
    raw: RawRun,
) -> Result<Run, SpecError> {
    require_name(path, field, &raw.name)?;
    validate_timeout(path, &format!("{field}.timeout"), raw.timeout)?;

    let args = match (raw.args, raw.argv_file) {
        (Some(_), Some(_)) => {
            return Err(error(
                path,
                format!("{field}.args"),
                "args and argv_file are mutually exclusive",
            ));
        }
        (Some(args), None) => {
            validate_strings_without_nul(path, &format!("{field}.args"), &args)?;
            ArgSource::Utf8(args)
        }
        (None, Some(file)) => {
            require_path(path, &format!("{field}.argv_file"), &file)?;
            ArgSource::File(file)
        }
        (None, None) => ArgSource::Utf8(Vec::new()),
    };

    let stdin = match raw.stdin {
        Some(source) => validate_byte_source(path, &format!("{field}.stdin"), source)?,
        None => ByteSource::Inline(String::new()),
    };
    let input_files = validate_input_files(path, field, raw.input_files)?;
    let cwd = match raw.cwd {
        Some(cwd) => {
            require_path(path, &format!("{field}.cwd.fixture"), &cwd.fixture)?;
            WorkingDirectory::Fixture(cwd.fixture)
        }
        None => WorkingDirectory::Private,
    };
    validate_environment(path, field, &raw.env)?;
    let resources = validate_resources(path, field, raw.resources)?;
    let expectation = validate_run_expectation(path, field, schema, raw.expect)?;

    Ok(Run {
        name: raw.name,
        args,
        stdin,
        input_files,
        cwd,
        env: raw.env,
        timeout_seconds: raw.timeout,
        serial: raw.serial,
        resources,
        expectation,
    })
}

fn validate_input_files(
    path: &Path,
    field: &str,
    raw: Option<Vec<RawInputFile>>,
) -> Result<Vec<InputFile>, SpecError> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    require_nonempty_collection(path, &format!("{field}.input_files"), &raw)?;

    let mut names = HashSet::new();
    raw.into_iter()
        .enumerate()
        .map(|(index, file)| {
            let item_field = format!("{field}.input_files[{index}]");
            require_unique_safe_name(path, &item_field, &file.name, &mut names)?;
            Ok(InputFile {
                name: file.name,
                contents: validate_byte_source(
                    path,
                    &format!("{item_field}.contents"),
                    file.contents,
                )?,
            })
        })
        .collect()
}

fn validate_run_expectation(
    path: &Path,
    field: &str,
    schema: SchemaVersion,
    raw: Option<RawRunExpectation>,
) -> Result<RunExpectation, SpecError> {
    let Some(raw) = raw else {
        return Ok(RunExpectation {
            exit: ExitExpectation::Code(0),
            stdout: StreamExpectation::exact_empty(),
            stderr: StreamExpectation::exact_empty(),
            output_files: Vec::new(),
        });
    };

    let exit = match raw.exit {
        Some(RawExitExpectation::Code(code)) => ExitExpectation::Code(code),
        Some(RawExitExpectation::Name(RawExitName::Failure)) => ExitExpectation::Failure,
        None => ExitExpectation::Code(0),
    };
    let stdout = match raw.stdout {
        Some(expectation) => validate_stream_expectation(
            path,
            &format!("{field}.expect.stdout"),
            schema,
            expectation,
        )?,
        None => StreamExpectation::exact_empty(),
    };
    let stderr = match raw.stderr {
        Some(expectation) => validate_stream_expectation(
            path,
            &format!("{field}.expect.stderr"),
            schema,
            expectation,
        )?,
        None => StreamExpectation::exact_empty(),
    };
    let output_files = validate_output_files(path, field, raw.output_files)?;

    Ok(RunExpectation {
        exit,
        stdout,
        stderr,
        output_files,
    })
}

fn validate_output_files(
    path: &Path,
    field: &str,
    raw: Option<Vec<RawOutputFileExpectation>>,
) -> Result<Vec<OutputFileExpectation>, SpecError> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    require_nonempty_collection(path, &format!("{field}.expect.output_files"), &raw)?;

    let mut names = HashSet::new();
    raw.into_iter()
        .enumerate()
        .map(|(index, file)| {
            let item_field = format!("{field}.expect.output_files[{index}]");
            require_unique_safe_name(path, &item_field, &file.name, &mut names)?;
            Ok(OutputFileExpectation {
                name: file.name,
                contents: validate_byte_source(
                    path,
                    &format!("{item_field}.contents"),
                    file.contents,
                )?,
            })
        })
        .collect()
}

fn validate_compile_expectation(
    path: &Path,
    field: &str,
    schema: SchemaVersion,
    raw: RawCompileExpectation,
) -> Result<CompileExpectation, SpecError> {
    if schema == SchemaVersion::V1 && raw.stdout.is_some() {
        return Err(error(
            path,
            format!("{field}.expect.stdout"),
            "compile-fail stdout expectations require schema version 2",
        ));
    }
    let stdout = match raw.stdout {
        Some(expectation) => validate_stream_expectation(
            path,
            &format!("{field}.expect.stdout"),
            schema,
            expectation,
        )?,
        None => StreamExpectation::exact_empty(),
    };
    let stderr = raw.stderr.ok_or_else(|| {
        error(
            path,
            format!("{field}.expect.stderr"),
            "compile-fail tests require a nonempty stderr expectation",
        )
    })?;
    if stderr.matches.is_none() && stderr.inline.as_deref() == Some("") {
        return Err(error(
            path,
            format!("{field}.expect.stderr.inline"),
            "compile-fail stderr must not be empty",
        ));
    }
    let stderr =
        validate_stream_expectation(path, &format!("{field}.expect.stderr"), schema, stderr)?;
    match &stderr {
        StreamExpectation::Ignore => Err(error(
            path,
            format!("{field}.expect.stderr.ignore"),
            "compile-fail stderr cannot be ignored",
        )),
        StreamExpectation::Match(matchers) => {
            for (index, matcher) in matchers.matchers().iter().enumerate() {
                if matches!(matcher.expected(), ByteSource::Inline(expected) if expected.is_empty())
                {
                    return Err(error(
                        path,
                        format!("{field}.expect.stderr.matches[{index}].inline"),
                        "compile-fail stderr matchers must not be empty",
                    ));
                }
            }
            Ok(CompileExpectation { stdout, stderr })
        }
    }
}

fn validate_stream_expectation(
    path: &Path,
    field: &str,
    schema: SchemaVersion,
    raw: RawStreamExpectation,
) -> Result<StreamExpectation, SpecError> {
    let singular_present =
        raw.mode.is_some() || raw.inline.is_some() || raw.file.is_some() || raw.ignore.is_some();
    if let Some(matches) = raw.matches {
        if schema == SchemaVersion::V1 {
            return Err(error(
                path,
                format!("{field}.matches"),
                "matcher lists require schema version 2",
            ));
        }
        if singular_present {
            return Err(error(
                path,
                field,
                "matches cannot coexist with match, inline, file, or ignore",
            ));
        }
        require_nonempty_collection(path, &format!("{field}.matches"), &matches)?;
        let mut names = HashSet::new();
        let matchers = matches
            .into_iter()
            .enumerate()
            .map(|(index, matcher)| {
                validate_stream_matcher(
                    path,
                    &format!("{field}.matches[{index}]"),
                    matcher,
                    &mut names,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(StreamExpectation::Match(
            StreamMatcherSet::try_from(matchers).expect("validated matcher lists must be nonempty"),
        ));
    }

    if raw.ignore == Some(false) {
        return Err(error(
            path,
            format!("{field}.ignore"),
            "must be true when present",
        ));
    }

    let source_count = usize::from(raw.inline.is_some()) + usize::from(raw.file.is_some());
    if raw.ignore == Some(true) {
        if source_count != 0 || raw.mode.is_some() {
            return Err(error(
                path,
                field,
                "ignore cannot coexist with match, inline, or file",
            ));
        }
        return Ok(StreamExpectation::Ignore);
    }
    if source_count != 1 {
        return Err(error(
            path,
            field,
            "exactly one of inline, file, or ignore = true is required",
        ));
    }

    let mode = match raw.mode {
        Some(RawMatchMode::Exact) | None => MatchMode::Exact,
        Some(RawMatchMode::StartsWith) => MatchMode::StartsWith,
        Some(RawMatchMode::Contains) => MatchMode::Contains,
    };
    let expected = validate_byte_source_fields(path, field, raw.inline, raw.file)?;
    if mode != MatchMode::Exact
        && matches!(&expected, ByteSource::Inline(contents) if contents.is_empty())
    {
        return Err(error(
            path,
            format!("{field}.inline"),
            "partial matchers must not be empty",
        ));
    }

    Ok(StreamExpectation::Match(StreamMatcherSet::one(
        StreamMatcher::new(mode, expected),
    )))
}

fn validate_stream_matcher(
    path: &Path,
    field: &str,
    raw: RawStreamMatcher,
    names: &mut HashSet<String>,
) -> Result<StreamMatcher<ByteSource>, SpecError> {
    if let Some(name) = &raw.name {
        require_name(path, &format!("{field}.name"), name)?;
        if !names.insert(name.clone()) {
            return Err(error(
                path,
                format!("{field}.name"),
                format!("duplicate name {name:?}"),
            ));
        }
    }
    let mode = validate_match_mode(raw.mode);
    let expected = validate_byte_source_fields(path, field, raw.inline, raw.file)?;
    validate_partial_matcher(path, field, mode, &expected)?;
    Ok(match raw.name {
        Some(name) => StreamMatcher::named(name, mode, expected),
        None => StreamMatcher::new(mode, expected),
    })
}

fn validate_match_mode(raw: Option<RawMatchMode>) -> MatchMode {
    match raw {
        Some(RawMatchMode::Exact) | None => MatchMode::Exact,
        Some(RawMatchMode::StartsWith) => MatchMode::StartsWith,
        Some(RawMatchMode::Contains) => MatchMode::Contains,
    }
}

fn validate_partial_matcher(
    path: &Path,
    field: &str,
    mode: MatchMode,
    expected: &ByteSource,
) -> Result<(), SpecError> {
    if mode != MatchMode::Exact
        && matches!(expected, ByteSource::Inline(contents) if contents.is_empty())
    {
        Err(error(
            path,
            format!("{field}.inline"),
            "partial matchers must not be empty",
        ))
    } else {
        Ok(())
    }
}

fn validate_byte_source(
    path: &Path,
    field: &str,
    raw: RawByteSource,
) -> Result<ByteSource, SpecError> {
    validate_byte_source_fields(path, field, raw.inline, raw.file)
}

fn validate_byte_source_fields(
    path: &Path,
    field: &str,
    inline: Option<String>,
    file: Option<PathBuf>,
) -> Result<ByteSource, SpecError> {
    match (inline, file) {
        (Some(contents), None) => Ok(ByteSource::Inline(contents)),
        (None, Some(file)) => {
            require_path(path, &format!("{field}.file"), &file)?;
            Ok(ByteSource::File(file))
        }
        (Some(_), Some(_)) => Err(error(path, field, "inline and file are mutually exclusive")),
        (None, None) => Err(error(
            path,
            field,
            "exactly one of inline or file is required",
        )),
    }
}

fn validate_resources(
    path: &Path,
    field: &str,
    resources: Option<Vec<String>>,
) -> Result<Vec<String>, SpecError> {
    let Some(resources) = resources else {
        return Ok(Vec::new());
    };
    require_nonempty_collection(path, &format!("{field}.resources"), &resources)?;
    validate_unique_strings(path, &format!("{field}.resources"), &resources)?;
    for (index, resource) in resources.iter().enumerate() {
        require_name(path, &format!("{field}.resources[{index}]"), resource)?;
        if resource.contains('\0') {
            return Err(error(
                path,
                format!("{field}.resources[{index}]"),
                "resource names must not contain NUL",
            ));
        }
    }
    Ok(resources)
}

fn validate_environment(
    path: &Path,
    field: &str,
    environment: &BTreeMap<String, String>,
) -> Result<(), SpecError> {
    for (name, value) in environment {
        let env_field = format!("{field}.env.{name}");
        if name.is_empty() || name.contains('=') || name.contains('\0') {
            return Err(error(
                path,
                env_field,
                "environment names must be nonempty and contain neither '=' nor NUL",
            ));
        }
        if value.contains('\0') {
            return Err(error(
                path,
                env_field,
                "environment values must not contain NUL",
            ));
        }
    }
    Ok(())
}

fn validate_timeout(path: &Path, field: &str, timeout: Option<u64>) -> Result<(), SpecError> {
    if timeout == Some(0) {
        Err(error(path, field, "must be greater than zero seconds"))
    } else {
        Ok(())
    }
}

fn validate_strings_without_nul(
    path: &Path,
    field: &str,
    values: &[String],
) -> Result<(), SpecError> {
    if let Some(index) = values.iter().position(|value| value.contains('\0')) {
        Err(error(
            path,
            format!("{field}[{index}]"),
            "must not contain NUL",
        ))
    } else {
        Ok(())
    }
}

fn validate_unique_strings(path: &Path, field: &str, values: &[String]) -> Result<(), SpecError> {
    let mut seen = HashSet::new();
    for (index, value) in values.iter().enumerate() {
        if !seen.insert(value) {
            return Err(error(
                path,
                format!("{field}[{index}]"),
                format!("duplicate name {value:?}"),
            ));
        }
    }
    Ok(())
}

fn require_unique_name(
    path: &Path,
    field: &str,
    name: &str,
    names: &mut HashSet<String>,
) -> Result<(), SpecError> {
    require_name(path, &format!("{field}.name"), name)?;
    if names.insert(name.to_owned()) {
        Ok(())
    } else {
        Err(error(
            path,
            format!("{field}.name"),
            format!("duplicate name {name:?}"),
        ))
    }
}

fn require_unique_safe_name(
    path: &Path,
    field: &str,
    name: &str,
    names: &mut HashSet<String>,
) -> Result<(), SpecError> {
    require_safe_name(path, &format!("{field}.name"), name)?;
    if names.insert(name.to_owned()) {
        Ok(())
    } else {
        Err(error(
            path,
            format!("{field}.name"),
            format!("duplicate name {name:?}"),
        ))
    }
}

fn require_name(path: &Path, field: &str, name: &str) -> Result<(), SpecError> {
    if name.is_empty() {
        Err(error(path, field, "name must not be empty"))
    } else {
        Ok(())
    }
}

fn require_safe_name(path: &Path, field: &str, name: &str) -> Result<(), SpecError> {
    let safe = !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'));
    if safe {
        Ok(())
    } else {
        Err(error(
            path,
            field,
            "must use only ASCII letters, digits, '.', '-', or '_' and be one safe path component",
        ))
    }
}

fn require_path(path: &Path, field: &str, value: &Path) -> Result<(), SpecError> {
    if value.as_os_str().is_empty() {
        Err(error(path, field, "path must not be empty"))
    } else {
        Ok(())
    }
}

fn require_nonempty_collection<T>(path: &Path, field: &str, values: &[T]) -> Result<(), SpecError> {
    if values.is_empty() {
        Err(error(path, field, "must not be empty when present"))
    } else {
        Ok(())
    }
}

fn error(path: &Path, field: impl Into<String>, message: impl Into<String>) -> SpecError {
    SpecError::new(path, field, message)
}
