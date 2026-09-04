//! Best-effort repository revision context for generated evidence.

use std::{path::Path, process::Command};

use crate::model::CompilerIdentity;

pub(super) fn inspect(repository_root: &Path) -> CompilerIdentity {
    let revision = git(repository_root, &["rev-parse", "HEAD"])
        .filter(|output| !output.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    let dirty = git(repository_root, &["status", "--porcelain=v1"])
        .is_some_and(|output| !output.is_empty());
    CompilerIdentity { revision, dirty }
}

fn git(repository_root: &Path, arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repository_root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
