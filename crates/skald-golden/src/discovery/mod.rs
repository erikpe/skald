//! Deterministic spec discovery and repository-root ownership.

mod walk;

use crate::{parse_config, parse_spec, PlanError, RepositoryConfig, Spec};
use std::{fs, path::PathBuf};

pub(crate) struct DiscoveredSuite {
    pub(crate) root: PathBuf,
    pub(crate) config: RepositoryConfig,
    pub(crate) specs: Vec<DiscoveredSpec>,
}

pub(crate) struct DiscoveredSpec {
    pub(crate) path: PathBuf,
    pub(crate) relative_path: PathBuf,
    pub(crate) spec: Spec,
}

pub(crate) fn discover(root: impl Into<PathBuf>) -> Result<DiscoveredSuite, PlanError> {
    let requested_root = root.into();
    let root = fs::canonicalize(&requested_root).map_err(|error| {
        PlanError::at_path(
            &requested_root,
            format!("could not canonicalize golden root: {error}"),
        )
    })?;
    if !root.is_dir() {
        return Err(PlanError::at_path(&root, "golden root is not a directory"));
    }

    let config_path = root.join("config.toml");
    let config_contents = fs::read_to_string(&config_path).map_err(|error| {
        PlanError::at_path(
            &config_path,
            format!("could not read repository variant configuration: {error}"),
        )
    })?;
    let config = parse_config(&config_path, &config_contents).map_err(PlanError::from_spec)?;

    let mut paths = Vec::new();
    walk::collect_spec_paths(&root, &mut paths)?;
    paths.sort();

    let mut specs = Vec::with_capacity(paths.len());
    for path in paths {
        let canonical = fs::canonicalize(&path).map_err(|error| {
            PlanError::at_path(&path, format!("could not canonicalize spec: {error}"))
        })?;
        if !canonical.starts_with(&root) {
            return Err(PlanError::at_path(
                &path,
                "discovered spec resolves outside the golden root",
            ));
        }
        let relative_path = canonical
            .strip_prefix(&root)
            .expect("contained canonical spec should be below canonical root")
            .to_owned();
        let contents = fs::read_to_string(&canonical).map_err(|error| {
            PlanError::at_path(&canonical, format!("could not read spec as UTF-8: {error}"))
        })?;
        let spec = parse_spec(&canonical, &contents).map_err(PlanError::from_spec)?;
        specs.push(DiscoveredSpec {
            path: canonical,
            relative_path,
            spec,
        });
    }

    Ok(DiscoveredSuite {
        root,
        config,
        specs,
    })
}
