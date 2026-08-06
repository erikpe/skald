use crate::PlanError;
use std::{ffi::OsStr, fs, path::Path};

pub(super) fn collect_spec_paths(
    directory: &Path,
    paths: &mut Vec<std::path::PathBuf>,
) -> Result<(), PlanError> {
    let entries = fs::read_dir(directory).map_err(|error| {
        PlanError::at_path(directory, format!("could not read directory: {error}"))
    })?;
    let mut entries = entries.collect::<Result<Vec<_>, _>>().map_err(|error| {
        PlanError::at_path(
            directory,
            format!("could not read directory entry: {error}"),
        )
    })?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let file_type = entry.file_type().map_err(|error| {
            PlanError::at_path(entry.path(), format!("could not inspect entry: {error}"))
        })?;
        if file_type.is_dir() {
            collect_spec_paths(&entry.path(), paths)?;
        } else if file_type.is_file() && is_spec_path(&entry.path()) {
            paths.push(entry.path());
        }
    }
    Ok(())
}

fn is_spec_path(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.ends_with(".golden.toml"))
}
