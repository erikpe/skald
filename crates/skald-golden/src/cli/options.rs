use crate::{Determinism, SelectionOptions};
use std::{ffi::OsString, num::NonZeroUsize, path::PathBuf};

pub(super) struct Options {
    pub(super) inspection: Option<Inspection>,
    pub(super) selection: SelectionOptions,
    pub(super) compiler_args: Vec<OsString>,
    pub(super) compiler: Option<PathBuf>,
    pub(super) determinism: Determinism,
    pub(super) jobs: Option<NonZeroUsize>,
    pub(super) fail_fast: bool,
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
            help,
        })
    }
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
