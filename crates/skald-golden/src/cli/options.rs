use crate::{Determinism, ReportFormat, SelectionOptions};
use std::{ffi::OsString, num::NonZeroUsize, path::PathBuf, time::Duration};

pub(super) struct Options {
    pub(super) inspection: Option<Inspection>,
    pub(super) selection: SelectionOptions,
    pub(super) compiler_args: Vec<OsString>,
    pub(super) compiler: Option<PathBuf>,
    pub(super) determinism: Determinism,
    pub(super) jobs: Option<NonZeroUsize>,
    pub(super) fail_fast: bool,
    pub(super) timeout: Option<Duration>,
    pub(super) show_output: bool,
    pub(super) slowest: Option<NonZeroUsize>,
    pub(super) format: ReportFormat,
    pub(super) keep_all_artifacts: bool,
    pub(super) help: bool,
}

pub(super) enum Inspection {
    List,
    ListTests,
    Explain(String),
}

impl Options {
    pub(super) fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut arguments = arguments.into_iter();
        let _program = arguments.next();
        let mut inspection = None;
        let mut selection = SelectionOptions::default();
        let mut compiler_args = Vec::new();
        let mut compiler = None;
        let mut determinism = Determinism::Off;
        let mut jobs = None;
        let mut fail_fast = false;
        let mut timeout = None;
        let mut show_output = false;
        let mut slowest = None;
        let mut format = ReportFormat::Human;
        let mut keep_all_artifacts = false;
        let mut help = false;

        while let Some(argument) = arguments.next() {
            match argument.to_str() {
                Some("--help" | "-h") => help = true,
                Some("--list") => set_inspection(&mut inspection, Inspection::List)?,
                Some("--list-tests") => set_inspection(&mut inspection, Inspection::ListTests)?,
                Some("--explain") => set_inspection(
                    &mut inspection,
                    Inspection::Explain(utf8_value(&mut arguments, "--explain")?),
                )?,
                Some("--filter") => {
                    selection = selection.include(utf8_value(&mut arguments, "--filter")?)
                }
                Some("--exclude") => {
                    selection = selection.exclude(utf8_value(&mut arguments, "--exclude")?)
                }
                Some("--exact") => {
                    selection = selection.exact(utf8_value(&mut arguments, "--exact")?)
                }
                Some("--variant") => {
                    selection = selection.variant(utf8_value(&mut arguments, "--variant")?)
                }
                Some("--compiler-arg") => compiler_args.push(
                    arguments
                        .next()
                        .ok_or_else(|| "expected an argument after --compiler-arg".to_owned())?,
                ),
                Some("--compiler") => {
                    compiler = Some(PathBuf::from(os_value(&mut arguments, "--compiler")?));
                }
                Some("--determinism") => {
                    determinism = utf8_value(&mut arguments, "--determinism")?.parse()?;
                }
                Some("--jobs") => {
                    jobs = Some(positive_usize(
                        &utf8_value(&mut arguments, "--jobs")?,
                        "--jobs",
                    )?);
                }
                Some("--fail-fast") => fail_fast = true,
                Some("--timeout") => {
                    timeout = Some(positive_seconds(&utf8_value(&mut arguments, "--timeout")?)?);
                }
                Some("--show-output") => show_output = true,
                Some("--slowest") => {
                    slowest = Some(positive_usize(
                        &utf8_value(&mut arguments, "--slowest")?,
                        "--slowest",
                    )?);
                }
                Some("--format") => {
                    format = utf8_value(&mut arguments, "--format")?.parse()?;
                }
                Some("--keep-all-artifacts") => keep_all_artifacts = true,
                Some("--allow-empty") => selection = selection.allow_empty(true),
                Some(value) => return Err(format!("unknown option {value:?}")),
                None => return Err("runner options must be valid UTF-8".to_owned()),
            }
        }

        Ok(Self {
            inspection,
            selection,
            compiler_args,
            compiler,
            determinism,
            jobs,
            fail_fast,
            timeout,
            show_output,
            slowest,
            format,
            keep_all_artifacts,
            help,
        })
    }
}

fn positive_seconds(value: &str) -> Result<Duration, String> {
    let seconds = value
        .parse::<u64>()
        .ok()
        .filter(|seconds| *seconds > 0)
        .ok_or_else(|| "--timeout requires a positive integer number of seconds".to_owned())?;
    Ok(Duration::from_secs(seconds))
}

fn positive_usize(value: &str, option: &str) -> Result<NonZeroUsize, String> {
    value
        .parse::<usize>()
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| format!("{option} requires a positive integer"))
}

fn os_value(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<OsString, String> {
    arguments
        .next()
        .ok_or_else(|| format!("expected a value after {option}"))
}

fn set_inspection(current: &mut Option<Inspection>, next: Inspection) -> Result<(), String> {
    if current.is_some() {
        Err("choose only one of --list, --list-tests, or --explain".to_owned())
    } else {
        *current = Some(next);
        Ok(())
    }
}

fn utf8_value(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("expected a value after {option}"))?
        .into_string()
        .map_err(|_| format!("value after {option} must be valid UTF-8"))
}
