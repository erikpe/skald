//! Typed command-line option parsing without filesystem access.

use std::{ffi::OsString, path::PathBuf};

use crate::{backend::DEFAULT_TARGET_NAME, module::ModulePath};

use super::super::request::{
    ArtifactKind, ArtifactOptions, EntrySelector, StandardLibrarySelection,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CompileOptions {
    pub entry: EntrySelector,
    pub module_roots: Vec<PathBuf>,
    pub standard_library: StandardLibrarySelection,
    pub artifact: ArtifactOptions,
    pub target: String,
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
    Ok(Command::Compile(CompileOptions {
        entry,
        module_roots,
        standard_library,
        artifact: ArtifactOptions::new(output_kind, output),
        target: target.unwrap_or_else(|| DEFAULT_TARGET_NAME.to_owned()),
    }))
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
