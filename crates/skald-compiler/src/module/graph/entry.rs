use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs::{self, File},
    path::{Component, Path, PathBuf},
};

use crate::{
    driver::EntrySelector,
    identity::{PackageId, ProviderId},
    lexical_policy::is_source_identifier,
};

use super::super::{
    provider::lexical_normalize, CandidateLookupError, CandidateResolution, ModuleCandidate,
    ModulePath, ProviderSet,
};

pub(super) struct SelectedEntry {
    pub candidate: ModuleCandidate,
    pub singleton: Option<ModuleCandidate>,
}

pub(super) struct LoaderProviders<'providers> {
    roots: &'providers ProviderSet,
    singleton: Option<ModuleCandidate>,
}

impl<'providers> LoaderProviders<'providers> {
    pub fn new(roots: &'providers ProviderSet, singleton: Option<ModuleCandidate>) -> Self {
        Self { roots, singleton }
    }

    pub fn resolve(
        &self,
        module_path: &ModulePath,
    ) -> Result<ModuleCandidate, ModuleResolutionError> {
        let root_resolution = self
            .roots
            .resolve(module_path)
            .map_err(ModuleResolutionError::Lookup)?;
        let singleton = self
            .singleton
            .as_ref()
            .filter(|candidate| candidate.module_path() == module_path)
            .cloned();

        match (root_resolution, singleton) {
            (CandidateResolution::Missing { .. }, None) => {
                Err(ModuleResolutionError::Missing(module_path.clone()))
            }
            (CandidateResolution::Missing { .. }, Some(candidate))
            | (CandidateResolution::Unique(candidate), None) => Ok(candidate),
            (CandidateResolution::Unique(root), Some(singleton)) => {
                Err(ModuleResolutionError::Ambiguous(vec![root, singleton]))
            }
            (CandidateResolution::Ambiguous { mut candidates, .. }, singleton) => {
                candidates.extend(singleton);
                Err(ModuleResolutionError::Ambiguous(candidates))
            }
        }
    }
}

#[derive(Debug)]
pub(super) enum ModuleResolutionError {
    Missing(ModulePath),
    Ambiguous(Vec<ModuleCandidate>),
    Lookup(Vec<CandidateLookupError>),
}

#[derive(Debug)]
pub(super) enum EntryError {
    Invalid {
        path: PathBuf,
        reason: String,
    },
    AmbiguousIdentity {
        path: PathBuf,
        identities: Vec<(ProviderId, ModulePath)>,
    },
    Resolution(ModuleResolutionError),
}

pub(super) fn select_entry(
    entry: &EntrySelector,
    working_directory: &Path,
    providers: &ProviderSet,
) -> Result<SelectedEntry, EntryError> {
    match entry {
        EntrySelector::Module(module_path) => {
            let candidate = LoaderProviders::new(providers, None)
                .resolve(module_path)
                .map_err(EntryError::Resolution)?;
            Ok(SelectedEntry {
                candidate,
                singleton: None,
            })
        }
        EntrySelector::File(path) => select_file_entry(path, working_directory, providers),
    }
}

fn select_file_entry(
    configured_path: &Path,
    working_directory: &Path,
    providers: &ProviderSet,
) -> Result<SelectedEntry, EntryError> {
    if !working_directory.is_absolute() {
        return Err(EntryError::Invalid {
            path: configured_path.to_owned(),
            reason: "the captured working directory is not absolute".into(),
        });
    }
    let absolute = if configured_path.is_absolute() {
        configured_path.to_owned()
    } else {
        working_directory.join(configured_path)
    };
    let lexical_path = lexical_normalize(&absolute);
    let display_source_path = positional_display_path(configured_path, working_directory);
    validate_source_suffix_and_stem(&lexical_path)?;

    let canonical_io_path =
        fs::canonicalize(&lexical_path).map_err(|error| EntryError::Invalid {
            path: lexical_path.clone(),
            reason: format!("the entry cannot be resolved: {:?}", error.kind()),
        })?;
    let metadata = fs::metadata(&canonical_io_path).map_err(|error| EntryError::Invalid {
        path: lexical_path.clone(),
        reason: format!("the entry cannot be inspected: {:?}", error.kind()),
    })?;
    if !metadata.is_file() {
        return Err(EntryError::Invalid {
            path: lexical_path,
            reason: "the entry does not resolve to a regular file".into(),
        });
    }
    File::open(&canonical_io_path).map_err(|error| EntryError::Invalid {
        path: lexical_path.clone(),
        reason: format!("the entry is not readable: {:?}", error.kind()),
    })?;

    let identities = rooted_identities(&lexical_path, providers)?;
    match identities.as_slice() {
        [] => {
            let module_path = module_path_from_file_name(&lexical_path)?;
            let trace_source_path = display_source_path.clone();
            let singleton = ModuleCandidate::new(
                module_path.clone(),
                ProviderId::new(providers.providers().len()),
                PackageId::new(providers.providers().len()),
                lexical_path
                    .file_name()
                    .expect("validated entry path has a file name")
                    .into(),
                display_source_path,
                canonical_io_path,
            )
            .with_trace_source_path(trace_source_path);
            let candidate = LoaderProviders::new(providers, Some(singleton.clone()))
                .resolve(&module_path)
                .map_err(EntryError::Resolution)?;
            Ok(SelectedEntry {
                candidate,
                singleton: Some(singleton),
            })
        }
        [(provider_id, module_path)] => {
            let candidate = LoaderProviders::new(providers, None)
                .resolve(module_path)
                .map_err(EntryError::Resolution)?
                .with_display_source_path(display_source_path);
            if candidate.provider_id() != *provider_id
                || candidate.canonical_io_path() != canonical_io_path
            {
                return Err(EntryError::Invalid {
                    path: lexical_path,
                    reason: "the rooted entry does not match its provider mapping".into(),
                });
            }
            Ok(SelectedEntry {
                candidate,
                singleton: None,
            })
        }
        _ => Err(EntryError::AmbiguousIdentity {
            path: lexical_path,
            identities,
        }),
    }
}

fn positional_display_path(configured_path: &Path, working_directory: &Path) -> PathBuf {
    if !configured_path.is_absolute() {
        return configured_path.to_owned();
    }
    configured_path
        .strip_prefix(working_directory)
        .map(Path::to_owned)
        .unwrap_or_else(|_| configured_path.to_owned())
}

fn validate_source_suffix_and_stem(path: &Path) -> Result<(), EntryError> {
    if path.extension() != Some(OsStr::new("ska")) {
        return Err(EntryError::Invalid {
            path: path.to_owned(),
            reason: "the entry must have the exact `.ska` suffix".into(),
        });
    }
    module_path_from_file_name(path).map(|_| ())
}

fn module_path_from_file_name(path: &Path) -> Result<ModulePath, EntryError> {
    let Some(stem) = path.file_stem().and_then(OsStr::to_str) else {
        return Err(EntryError::Invalid {
            path: path.to_owned(),
            reason: "the entry must have a UTF-8 file stem".into(),
        });
    };
    if !is_source_identifier(stem) {
        return Err(EntryError::Invalid {
            path: path.to_owned(),
            reason: format!("entry stem `{stem}` is not a Skald identifier"),
        });
    }
    ModulePath::from_components([stem]).map_err(|error| EntryError::Invalid {
        path: path.to_owned(),
        reason: error.to_string(),
    })
}

fn rooted_identities(
    lexical_entry: &Path,
    providers: &ProviderSet,
) -> Result<Vec<(ProviderId, ModulePath)>, EntryError> {
    let mut identities = BTreeSet::new();
    for provider in providers.providers() {
        let bases = std::iter::once(provider.canonical_root()).chain(
            provider
                .spellings()
                .iter()
                .map(|spelling| spelling.lexical_path()),
        );
        for base in bases {
            let Ok(relative) = lexical_entry.strip_prefix(base) else {
                continue;
            };
            let module_path = module_path_from_relative_file(relative, lexical_entry)?;
            identities.insert((provider.id(), module_path));
        }
    }
    Ok(identities.into_iter().collect())
}

fn module_path_from_relative_file(relative: &Path, entry: &Path) -> Result<ModulePath, EntryError> {
    let mut components = relative.components().collect::<Vec<_>>();
    let Some(Component::Normal(file_name)) = components.pop() else {
        return Err(EntryError::Invalid {
            path: entry.to_owned(),
            reason: "the rooted entry must name a source file below the root".into(),
        });
    };
    let file_path = Path::new(file_name);
    if file_path.extension() != Some(OsStr::new("ska")) {
        return Err(EntryError::Invalid {
            path: entry.to_owned(),
            reason: "the entry must have the exact `.ska` suffix".into(),
        });
    }
    let Some(stem) = file_path.file_stem().and_then(OsStr::to_str) else {
        return Err(EntryError::Invalid {
            path: entry.to_owned(),
            reason: "the entry must have a UTF-8 file stem".into(),
        });
    };
    let mut logical_components = Vec::with_capacity(components.len() + 1);
    for component in components {
        let Component::Normal(component) = component else {
            return Err(EntryError::Invalid {
                path: entry.to_owned(),
                reason: "the rooted entry contains an invalid path component".into(),
            });
        };
        let Some(component) = component.to_str() else {
            return Err(EntryError::Invalid {
                path: entry.to_owned(),
                reason: "the rooted entry contains a non-UTF-8 path component".into(),
            });
        };
        logical_components.push(component.to_owned());
    }
    logical_components.push(stem.to_owned());
    ModulePath::from_components(logical_components).map_err(|error| EntryError::Invalid {
        path: entry.to_owned(),
        reason: error.to_string(),
    })
}
