use super::{model::ResolvedByteSource, PlanError};
use crate::ByteSource;
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Component, Path, PathBuf},
};

pub(super) struct FixturePaths<'a> {
    root: &'a Path,
    spec_path: &'a Path,
    spec_directory: &'a Path,
}

impl<'a> FixturePaths<'a> {
    pub(super) fn new(root: &'a Path, spec_path: &'a Path) -> Self {
        Self {
            root,
            spec_path,
            spec_directory: spec_path
                .parent()
                .expect("discovered spec should have a parent directory"),
        }
    }

    pub(super) fn spec_path(&self) -> &Path {
        self.spec_path
    }

    pub(super) fn file(&self, reference: &Path, field: &str) -> Result<PathBuf, PlanError> {
        self.canonical(reference, field, ExpectedKind::File)
    }

    pub(super) fn directory(&self, reference: &Path, field: &str) -> Result<PathBuf, PlanError> {
        self.canonical(reference, field, ExpectedKind::Directory)
    }

    pub(super) fn byte_source(
        &self,
        source: &ByteSource,
        field: &str,
    ) -> Result<ResolvedByteSource, PlanError> {
        match source {
            ByteSource::Inline(contents) => Ok(ResolvedByteSource::Inline(contents.clone())),
            ByteSource::File(file) => Ok(ResolvedByteSource::File(self.file(file, field)?)),
        }
    }

    pub(super) fn compiler_args(
        &self,
        arguments: &[OsString],
        field: &str,
    ) -> Result<Vec<OsString>, PlanError> {
        let mut resolved = Vec::with_capacity(arguments.len());
        let mut index = 0;
        while index < arguments.len() {
            let argument = &arguments[index];
            resolved.push(argument.clone());
            if is_fixture_directory_option(argument) {
                let value_index = index + 1;
                let value = arguments.get(value_index).ok_or_else(|| {
                    PlanError::at_field(
                        self.spec_path,
                        format!("{field}[{index}]"),
                        format!(
                            "{} requires a directory argument",
                            argument.to_string_lossy()
                        ),
                    )
                })?;
                let directory =
                    self.directory(Path::new(value), &format!("{field}[{value_index}]"))?;
                resolved.push(directory.into_os_string());
                index += 2;
            } else {
                index += 1;
            }
        }
        Ok(resolved)
    }

    pub(super) fn relative(&self, canonical: &Path) -> Result<String, PlanError> {
        let relative = canonical
            .strip_prefix(self.root)
            .expect("resolved fixture path should remain below golden root");
        super::identity::slash_path(relative)
    }

    fn canonical(
        &self,
        reference: &Path,
        field: &str,
        expected: ExpectedKind,
    ) -> Result<PathBuf, PlanError> {
        let joined = lexical_fixture_path(
            self.root,
            self.spec_directory,
            self.spec_path,
            field,
            reference,
        )?;
        let canonical = fs::canonicalize(&joined).map_err(|error| {
            PlanError::at_field(
                self.spec_path,
                field,
                format!("could not canonicalize {}: {error}", joined.display()),
            )
        })?;
        if !canonical.starts_with(self.root) {
            return Err(PlanError::at_field(
                self.spec_path,
                field,
                format!(
                    "fixture resolves outside golden root: {}",
                    canonical.display()
                ),
            ));
        }
        let metadata = fs::metadata(&canonical).map_err(|error| {
            PlanError::at_field(
                self.spec_path,
                field,
                format!("could not inspect fixture: {error}"),
            )
        })?;
        let correct_kind = match expected {
            ExpectedKind::File => metadata.is_file(),
            ExpectedKind::Directory => metadata.is_dir(),
        };
        if !correct_kind {
            return Err(PlanError::at_field(
                self.spec_path,
                field,
                format!(
                    "expected {}, found another file type",
                    expected.description()
                ),
            ));
        }
        Ok(canonical)
    }
}

#[derive(Clone, Copy)]
enum ExpectedKind {
    File,
    Directory,
}

impl ExpectedKind {
    fn description(self) -> &'static str {
        match self {
            Self::File => "a file",
            Self::Directory => "a directory",
        }
    }
}

fn lexical_fixture_path(
    root: &Path,
    spec_directory: &Path,
    spec_path: &Path,
    field: &str,
    reference: &Path,
) -> Result<PathBuf, PlanError> {
    if reference.as_os_str().is_empty() || reference.is_absolute() {
        return Err(PlanError::at_field(
            spec_path,
            field,
            "fixture paths must be nonempty and relative to the spec directory",
        ));
    }
    let mut relative = spec_directory
        .strip_prefix(root)
        .expect("spec directory should remain below golden root")
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for component in reference.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => relative.push(value.to_owned()),
            Component::ParentDir if relative.pop().is_some() => {}
            Component::ParentDir => {
                return Err(PlanError::at_field(
                    spec_path,
                    field,
                    "fixture path lexically escapes the golden root",
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(PlanError::at_field(
                    spec_path,
                    field,
                    "fixture paths must be relative to the spec directory",
                ));
            }
        }
    }
    Ok(relative
        .into_iter()
        .fold(root.to_owned(), |path, part| path.join(part)))
}

fn is_fixture_directory_option(argument: &OsStr) -> bool {
    matches!(argument.to_str(), Some("--module-root" | "--stdlib-root"))
}
