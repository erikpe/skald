//! Atomic publication of compiler artifacts.

use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);
const MAX_TEMPORARY_ATTEMPTS: usize = 1_024;

/// An unpublished artifact reserved beside its final destination.
///
/// The reserved path is removed on drop. Publication uses a same-directory
/// rename so a destination is replaced only by a complete artifact.
pub(super) struct PendingArtifact {
    destination: PathBuf,
    temporary: PathBuf,
}

impl PendingArtifact {
    pub(super) fn new(destination: &Path) -> io::Result<Self> {
        for _ in 0..MAX_TEMPORARY_ATTEMPTS {
            let temporary = temporary_path(destination);
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => {
                    drop(file);
                    return Ok(Self {
                        destination: destination.to_owned(),
                        temporary,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }

        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "could not reserve a unique temporary output for `{}`",
                destination.display()
            ),
        ))
    }

    pub(super) fn path(&self) -> &Path {
        &self.temporary
    }

    pub(super) fn write(&self, contents: &[u8]) -> io::Result<()> {
        fs::write(&self.temporary, contents)
    }

    pub(super) fn publish(mut self) -> io::Result<()> {
        fs::rename(&self.temporary, &self.destination)?;
        self.temporary.clear();
        Ok(())
    }
}

impl Drop for PendingArtifact {
    fn drop(&mut self) {
        if !self.temporary.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.temporary);
        }
    }
}

fn temporary_path(destination: &Path) -> PathBuf {
    let id = NEXT_TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
    let mut path: OsString = destination.as_os_str().to_owned();
    path.push(format!(".skac-{}-{id}.tmp", std::process::id()));
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TemporaryDirectory;

    #[test]
    fn destination_changes_only_when_a_complete_artifact_is_published() {
        let directory = TemporaryDirectory::new("artifact-publish").unwrap();
        let destination = directory.join("program.s");
        fs::write(&destination, "previous artifact").unwrap();

        let pending = PendingArtifact::new(&destination).unwrap();
        let temporary = pending.path().to_owned();
        pending.write(b"complete artifact").unwrap();

        assert_eq!(
            fs::read_to_string(&destination).unwrap(),
            "previous artifact"
        );
        assert_eq!(fs::read_to_string(&temporary).unwrap(), "complete artifact");

        pending.publish().unwrap();
        assert_eq!(
            fs::read_to_string(&destination).unwrap(),
            "complete artifact"
        );
        assert!(!temporary.exists());
    }

    #[test]
    fn dropping_an_unpublished_artifact_removes_its_temporary_file() {
        let directory = TemporaryDirectory::new("artifact-drop").unwrap();
        let destination = directory.join("program.s");
        fs::write(&destination, "previous artifact").unwrap();

        let pending = PendingArtifact::new(&destination).unwrap();
        let temporary = pending.path().to_owned();
        pending.write(b"incomplete artifact").unwrap();
        drop(pending);

        assert_eq!(
            fs::read_to_string(&destination).unwrap(),
            "previous artifact"
        );
        assert!(!temporary.exists());
    }

    #[test]
    fn publication_failure_preserves_the_destination_and_cleans_up() {
        let directory = TemporaryDirectory::new("artifact-failed-publish").unwrap();
        let destination = directory.join("existing-directory");
        fs::create_dir(&destination).unwrap();

        let pending = PendingArtifact::new(&destination).unwrap();
        let temporary = pending.path().to_owned();
        pending.write(b"artifact").unwrap();

        assert!(pending.publish().is_err());
        assert!(destination.is_dir());
        assert!(!temporary.exists());
    }
}
