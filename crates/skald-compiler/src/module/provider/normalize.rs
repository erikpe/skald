use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

use crate::identity::{PackageId, ProviderId};

use super::model::{
    NormalizedProvider, NormalizedRootSpelling, ProviderNormalizationError,
    ProviderNormalizationErrorKind, ProviderRootConfiguration, ProviderSet,
};

/// Normalizes and coalesces all configured filesystem roots.
///
/// The result is ordered by canonical root rather than configuration order.
/// Provider and package identities therefore remain stable under option
/// permutation.
pub fn normalize_provider_roots(
    working_directory: &Path,
    configurations: &[ProviderRootConfiguration],
) -> Result<ProviderSet, Vec<ProviderNormalizationError>> {
    if !working_directory.is_absolute() {
        return Err(vec![ProviderNormalizationError::working_directory(
            working_directory.to_owned(),
        )]);
    }

    let mut configurations = configurations.to_vec();
    configurations.sort();
    let mut groups = BTreeMap::<PathBuf, Vec<NormalizedRootSpelling>>::new();
    let mut errors = Vec::new();

    for configuration in configurations {
        let absolute_path = if configuration.path().is_absolute() {
            configuration.path().to_owned()
        } else {
            working_directory.join(configuration.path())
        };
        let lexical_path = lexical_normalize(&absolute_path);
        let canonical_root = match fs::canonicalize(&absolute_path) {
            Ok(path) => path,
            Err(error) => {
                errors.push(ProviderNormalizationError::root(
                    ProviderNormalizationErrorKind::Canonicalization(error.kind()),
                    configuration,
                    absolute_path,
                ));
                continue;
            }
        };
        match fs::metadata(&canonical_root) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                errors.push(ProviderNormalizationError::root(
                    ProviderNormalizationErrorKind::NotDirectory,
                    configuration,
                    absolute_path,
                ));
                continue;
            }
            Err(error) => {
                errors.push(ProviderNormalizationError::root(
                    ProviderNormalizationErrorKind::Canonicalization(error.kind()),
                    configuration,
                    absolute_path,
                ));
                continue;
            }
        }
        groups
            .entry(canonical_root)
            .or_default()
            .push(NormalizedRootSpelling::new(
                configuration,
                absolute_path,
                lexical_path,
            ));
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    let providers = groups
        .into_iter()
        .enumerate()
        .map(|(index, (canonical_root, mut spellings))| {
            spellings.sort();
            let display_root = spellings
                .iter()
                .map(NormalizedRootSpelling::absolute_path)
                .min()
                .expect("a normalized provider has at least one spelling")
                .to_owned();
            NormalizedProvider::new(
                ProviderId::new(index),
                PackageId::new(index),
                canonical_root,
                display_root,
                spellings,
            )
        })
        .collect();
    Ok(ProviderSet::new(providers))
}

/// Removes ordinary `.` and `..` components without following symlinks.
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}
