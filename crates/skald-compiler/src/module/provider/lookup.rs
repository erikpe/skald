use std::{
    ffi::{OsStr, OsString},
    fs::{self, File},
    path::{Path, PathBuf},
};

use super::super::ModulePath;
use super::model::{
    CandidateLookupError, CandidateLookupErrorKind, CandidateResolution, ModuleCandidate,
    NormalizedProvider, ProviderSet,
};

pub(super) fn resolve_candidates(
    providers: &ProviderSet,
    module_path: &ModulePath,
) -> Result<CandidateResolution, Vec<CandidateLookupError>> {
    let root_relative_path = source_relative_path(module_path);
    let mut candidates = Vec::new();
    let mut errors = Vec::new();
    let mut case_errors = Vec::new();

    for provider in providers.providers() {
        match probe_provider(provider, module_path, &root_relative_path) {
            Ok(Some(candidate)) => candidates.push(candidate),
            Ok(None) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    CandidateLookupErrorKind::CaseMismatch
                        | CandidateLookupErrorKind::CaseCollision
                ) =>
            {
                case_errors.push(error);
            }
            Err(error) => errors.push(error),
        }
    }

    if !errors.is_empty() {
        if candidates.is_empty() {
            errors.extend(case_errors);
            errors.sort_by_key(CandidateLookupError::provider_id);
        }
        return Err(errors);
    }
    if candidates.is_empty() && !case_errors.is_empty() {
        return Err(case_errors);
    }
    Ok(match candidates.len() {
        0 => CandidateResolution::Missing {
            module_path: module_path.clone(),
        },
        1 => CandidateResolution::Unique(
            candidates
                .pop()
                .expect("one candidate was counted before selection"),
        ),
        _ => CandidateResolution::Ambiguous {
            module_path: module_path.clone(),
            candidates,
        },
    })
}

fn source_relative_path(module_path: &ModulePath) -> PathBuf {
    let mut components = module_path.components().peekable();
    let mut path = PathBuf::new();
    while let Some(component) = components.next() {
        if components.peek().is_some() {
            path.push(component);
        } else {
            path.push(format!("{component}.ska"));
        }
    }
    path
}

fn probe_provider(
    provider: &NormalizedProvider,
    module_path: &ModulePath,
    root_relative_path: &Path,
) -> Result<Option<ModuleCandidate>, CandidateLookupError> {
    let mut current = provider.canonical_root().to_owned();
    for expected in root_relative_path.components() {
        let expected = expected.as_os_str();
        let entries = read_directory(provider, module_path, &current)?;
        match select_directory_component(expected, entries) {
            DirectoryComponentSelection::Exact(path) => current = path,
            DirectoryComponentSelection::Missing => return Ok(None),
            DirectoryComponentSelection::CaseMismatch(folded_matches) => {
                return Err(CandidateLookupError::new(
                    CandidateLookupErrorKind::CaseMismatch,
                    module_path.clone(),
                    provider.id(),
                    current.join(expected),
                    folded_matches,
                ));
            }
            DirectoryComponentSelection::CaseCollision(folded_matches) => {
                return Err(CandidateLookupError::new(
                    CandidateLookupErrorKind::CaseCollision,
                    module_path.clone(),
                    provider.id(),
                    current.join(expected),
                    folded_matches,
                ));
            }
        }
    }

    let canonical_io_path = match fs::canonicalize(&current) {
        Ok(path) => path,
        Err(error) => {
            let kind = if fs::symlink_metadata(&current)
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
            {
                CandidateLookupErrorKind::SymlinkResolution(error.kind())
            } else {
                CandidateLookupErrorKind::Canonicalization(error.kind())
            };
            return Err(CandidateLookupError::new(
                kind,
                module_path.clone(),
                provider.id(),
                current,
                Vec::new(),
            ));
        }
    };
    let metadata = fs::metadata(&canonical_io_path).map_err(|error| {
        CandidateLookupError::new(
            CandidateLookupErrorKind::Canonicalization(error.kind()),
            module_path.clone(),
            provider.id(),
            current.clone(),
            Vec::new(),
        )
    })?;
    if !metadata.is_file() {
        return Err(CandidateLookupError::new(
            CandidateLookupErrorKind::NonRegularFile,
            module_path.clone(),
            provider.id(),
            current,
            Vec::new(),
        ));
    }
    File::open(&canonical_io_path).map_err(|error| {
        CandidateLookupError::new(
            CandidateLookupErrorKind::UnreadableFile(error.kind()),
            module_path.clone(),
            provider.id(),
            current.clone(),
            Vec::new(),
        )
    })?;

    Ok(Some(ModuleCandidate::new(
        module_path.clone(),
        provider.id(),
        provider.package_id(),
        root_relative_path.to_owned(),
        provider.display_root().join(root_relative_path),
        canonical_io_path,
    )))
}

fn read_directory(
    provider: &NormalizedProvider,
    module_path: &ModulePath,
    directory: &Path,
) -> Result<Vec<(OsString, PathBuf)>, CandidateLookupError> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            let kind = match fs::metadata(directory) {
                Ok(metadata) if !metadata.is_dir() => {
                    CandidateLookupErrorKind::NonDirectoryComponent
                }
                _ if fs::symlink_metadata(directory)
                    .is_ok_and(|metadata| metadata.file_type().is_symlink()) =>
                {
                    CandidateLookupErrorKind::SymlinkResolution(error.kind())
                }
                _ => CandidateLookupErrorKind::UnreadableDirectory(error.kind()),
            };
            return Err(CandidateLookupError::new(
                kind,
                module_path.clone(),
                provider.id(),
                directory.to_owned(),
                Vec::new(),
            ));
        }
    };

    let mut names = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => names.push((entry.file_name(), entry.path())),
            Err(error) => {
                return Err(CandidateLookupError::new(
                    CandidateLookupErrorKind::UnreadableDirectory(error.kind()),
                    module_path.clone(),
                    provider.id(),
                    directory.to_owned(),
                    Vec::new(),
                ));
            }
        }
    }
    Ok(names)
}

fn ascii_case_equal(left: &OsStr, right: &OsStr) -> bool {
    match (left.to_str(), right.to_str()) {
        (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
        _ => false,
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum DirectoryComponentSelection {
    Missing,
    Exact(PathBuf),
    CaseMismatch(Vec<PathBuf>),
    CaseCollision(Vec<PathBuf>),
}

pub(super) fn select_directory_component(
    expected: &OsStr,
    mut entries: Vec<(OsString, PathBuf)>,
) -> DirectoryComponentSelection {
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let mut exact = None;
    let mut folded_matches = Vec::new();
    for (name, path) in entries {
        if name == expected {
            exact = Some(path.clone());
        }
        if ascii_case_equal(&name, expected) {
            folded_matches.push(path);
        }
    }
    match exact {
        Some(path) => DirectoryComponentSelection::Exact(path),
        None if folded_matches.is_empty() => DirectoryComponentSelection::Missing,
        None if folded_matches.len() == 1 => {
            DirectoryComponentSelection::CaseMismatch(folded_matches)
        }
        None => DirectoryComponentSelection::CaseCollision(folded_matches),
    }
}
