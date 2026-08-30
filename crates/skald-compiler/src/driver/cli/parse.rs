//! Typed command-line option parsing without filesystem access.

use std::{ffi::OsString, path::PathBuf};

use crate::{
    backend::{RuntimeTracePolicy, DEFAULT_TARGET_NAME},
    module::ModulePath,
    passes::MirOptimizationProfile,
    reporting::ReportDetail,
};

use super::super::request::{
    ArtifactKind, ArtifactOptions, EntrySelector, MirOptimizationOptions, StandardLibrarySelection,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CompileOptions {
    pub entry: EntrySelector,
    pub module_roots: Vec<PathBuf>,
    pub standard_library: StandardLibrarySelection,
    pub artifact: ArtifactOptions,
    pub mir_optimization: MirOptimizationOptions,
    pub target: String,
    pub report_detail: ReportDetail,
    pub diagnostic_level: DiagnosticLevel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DiagnosticLevel {
    Warning,
    Error,
}

pub(super) enum Command {
    Help,
    Version,
    Compile(CompileOptions),
}

pub(super) fn parse_command<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();
    let _program_name = args.next();
    let mut positional_file = None;
    let mut logical_entry = None;
    let mut module_roots = Vec::new();
    let mut standard_library_root = None;
    let mut no_standard_library = false;
    let mut output = None;
    let mut output_kind = ArtifactKind::Executable;
    let mut emit_seen = false;
    let mut target = None;
    let mut omit_runtime_trace = false;
    let mut mir_optimization_profile = None;
    let mut disabled_mir_passes = Vec::new();
    let mut verbose = 0usize;
    let mut quiet = 0usize;
    let mut report_level = None;
    let mut diagnostic_level = None;

    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("-h" | "--help") => return Ok(Command::Help),
            Some("--version") => return Ok(Command::Version),
            Some("--entry") => {
                if logical_entry.is_some() {
                    return Err("entry option specified more than once".to_owned());
                }
                let value = utf8_option_value(&mut args, "--entry", "a module path")?;
                logical_entry =
                    Some(value.parse::<ModulePath>().map_err(|error| {
                        format!("invalid entry module path `{value}`: {error}")
                    })?);
            }
            Some("--module-root") => {
                module_roots.push(path_option_value(
                    &mut args,
                    "--module-root",
                    "a directory",
                )?);
            }
            Some("--stdlib-root") => {
                if standard_library_root.is_some() {
                    return Err("standard-library root specified more than once".to_owned());
                }
                standard_library_root = Some(path_option_value(
                    &mut args,
                    "--stdlib-root",
                    "a directory",
                )?);
            }
            Some("--no-stdlib") => {
                if no_standard_library {
                    return Err("no-stdlib option specified more than once".to_owned());
                }
                no_standard_library = true;
            }
            Some("-o" | "--output") => {
                if output.is_some() {
                    return Err("output option specified more than once".to_owned());
                }
                output = Some(path_option_value(&mut args, "-o", "a path")?);
            }
            Some("--emit") => {
                if emit_seen {
                    return Err("emit option specified more than once".to_owned());
                }
                emit_seen = true;
                let value = utf8_option_value(&mut args, "--emit", "`asm`")?;
                if value != "asm" {
                    return Err(format!(
                        "unsupported emission kind `{value}`; expected `asm`"
                    ));
                }
                output_kind = ArtifactKind::Assembly;
            }
            Some("--target") => {
                if target.is_some() {
                    return Err("target option specified more than once".to_owned());
                }
                target = Some(utf8_option_value(&mut args, "--target", "a target name")?);
            }
            Some("--omit-runtime-trace") => {
                if omit_runtime_trace {
                    return Err("omit-runtime-trace option specified more than once".to_owned());
                }
                omit_runtime_trace = true;
            }
            Some("--mir-optimization") => {
                if mir_optimization_profile.is_some() {
                    return Err("MIR optimization profile specified more than once".to_owned());
                }
                let value =
                    utf8_option_value(&mut args, "--mir-optimization", "`none` or `default`")?;
                mir_optimization_profile = Some(parse_mir_optimization_profile(&value)?);
            }
            Some("--disable-mir-pass") => {
                disabled_mir_passes.push(utf8_option_value(
                    &mut args,
                    "--disable-mir-pass",
                    "a registered MIR pass name",
                )?);
            }
            Some("--report-level") => {
                if report_level.is_some() {
                    return Err("report level specified more than once".to_owned());
                }
                let value = utf8_option_value(
                    &mut args,
                    "--report-level",
                    "`off`, `phases`, `details`, or `trace`",
                )?;
                report_level = Some(parse_report_detail(&value)?);
            }
            Some("--diagnostic-level") => {
                if diagnostic_level.is_some() {
                    return Err("diagnostic level specified more than once".to_owned());
                }
                let value =
                    utf8_option_value(&mut args, "--diagnostic-level", "`warning` or `error`")?;
                diagnostic_level = Some(parse_diagnostic_level(&value)?);
            }
            Some(value) if report_shorthand(value).is_some() => {
                let (more_verbose, more_quiet) =
                    report_shorthand(value).expect("guard recognized report shorthand");
                verbose = verbose.saturating_add(more_verbose);
                quiet = quiet.saturating_add(more_quiet);
            }
            Some(value) if value.starts_with('-') => {
                return Err(format!("unknown option `{value}`"));
            }
            _ if positional_file.is_some() => {
                return Err("more than one positional input file was provided".to_owned())
            }
            _ => positional_file = Some(PathBuf::from(argument)),
        }
    }

    let entry = EntrySelector::from_options(positional_file, logical_entry)
        .map_err(|error| error.to_string())?;
    let standard_library =
        StandardLibrarySelection::from_options(standard_library_root, no_standard_library)
            .map_err(|error| error.to_string())?;
    let report_detail = resolve_report_detail(verbose, quiet, report_level)?;
    let mut mir_optimization =
        MirOptimizationOptions::new(mir_optimization_profile.unwrap_or_default());
    for name in disabled_mir_passes {
        mir_optimization = mir_optimization.with_disabled_pass(name);
    }
    mir_optimization
        .resolve_schedule()
        .map_err(|error| error.to_string())?;
    Ok(Command::Compile(CompileOptions {
        entry,
        module_roots,
        standard_library,
        artifact: ArtifactOptions::new(output_kind, output).with_runtime_trace_policy(
            if omit_runtime_trace {
                RuntimeTracePolicy::Omitted
            } else {
                RuntimeTracePolicy::Enabled
            },
        ),
        mir_optimization,
        target: target.unwrap_or_else(|| DEFAULT_TARGET_NAME.to_owned()),
        report_detail,
        diagnostic_level: diagnostic_level.unwrap_or(DiagnosticLevel::Warning),
    }))
}

fn parse_mir_optimization_profile(value: &str) -> Result<MirOptimizationProfile, String> {
    match value {
        "none" => Ok(MirOptimizationProfile::None),
        "default" => Ok(MirOptimizationProfile::Default),
        _ => Err(format!(
            "invalid MIR optimization profile `{value}`; expected `none` or `default`"
        )),
    }
}

fn report_shorthand(argument: &str) -> Option<(usize, usize)> {
    let shorthand = argument.strip_prefix('-')?;
    if shorthand.is_empty() || !shorthand.bytes().all(|byte| matches!(byte, b'v' | b'q')) {
        return None;
    }
    Some((
        shorthand.bytes().filter(|byte| *byte == b'v').count(),
        shorthand.bytes().filter(|byte| *byte == b'q').count(),
    ))
}

fn resolve_report_detail(
    verbose: usize,
    quiet: usize,
    explicit: Option<ReportDetail>,
) -> Result<ReportDetail, String> {
    if explicit.is_some() && (verbose != 0 || quiet != 0) {
        return Err(
            "report shorthand `-v`/`-q` cannot be combined with `--report-level`".to_owned(),
        );
    }
    if let Some(explicit) = explicit {
        return Ok(explicit);
    }
    Ok(match verbose.saturating_sub(quiet) {
        0 => ReportDetail::Off,
        1 => ReportDetail::Phases,
        2 => ReportDetail::Details,
        _ => ReportDetail::Trace,
    })
}

fn parse_report_detail(value: &str) -> Result<ReportDetail, String> {
    match value {
        "off" => Ok(ReportDetail::Off),
        "phases" => Ok(ReportDetail::Phases),
        "details" => Ok(ReportDetail::Details),
        "trace" => Ok(ReportDetail::Trace),
        _ => Err(format!(
            "invalid report level `{value}`; expected `off`, `phases`, `details`, or `trace`"
        )),
    }
}

fn parse_diagnostic_level(value: &str) -> Result<DiagnosticLevel, String> {
    match value {
        "warning" => Ok(DiagnosticLevel::Warning),
        "error" => Ok(DiagnosticLevel::Error),
        _ => Err(format!(
            "invalid diagnostic level `{value}`; expected `warning` or `error`"
        )),
    }
}

fn path_option_value(
    args: &mut impl Iterator<Item = OsString>,
    option: &str,
    expected: &str,
) -> Result<PathBuf, String> {
    args.next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("expected {expected} after `{option}`"))
}

fn utf8_option_value(
    args: &mut impl Iterator<Item = OsString>,
    option: &str,
    expected: &str,
) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("expected {expected} after `{option}`"))?
        .into_string()
        .map_err(|_| format!("value after `{option}` must be valid UTF-8"))
}

#[cfg(test)]
mod tests;
