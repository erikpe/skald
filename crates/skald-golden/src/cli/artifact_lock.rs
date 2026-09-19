use nix::{
    errno::Errno,
    fcntl::{Flock, FlockArg},
};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
};

const LOCK_FILE: &str = ".skald-golden.lock";

/// Exclusive process ownership of one golden artifact root.
///
/// The advisory lock is tied to the open file and therefore disappears when a
/// runner exits, including abnormal process termination. The file itself stays
/// behind so acquiring ownership never races with path deletion.
pub(super) struct ArtifactRootLock {
    _file: Flock<File>,
}

impl ArtifactRootLock {
    pub(super) fn acquire(artifact_root: &Path) -> Result<Self, String> {
        fs::create_dir_all(artifact_root).map_err(|error| {
            format!(
                "could not prepare artifact root {}: {error}",
                artifact_root.display()
            )
        })?;
        let path = artifact_root.join(LOCK_FILE);
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|error| format!("could not open artifact lock {}: {error}", path.display()))?;
        let mut file = match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
            Ok(file) => file,
            Err((_, Errno::EWOULDBLOCK)) => return Err(in_use(&path)),
            Err((_, error)) => {
                return Err(format!(
                    "could not lock artifact root {}: {error}",
                    artifact_root.display()
                ));
            }
        };
        file.set_len(0)
            .and_then(|()| writeln!(file, "process {}", std::process::id()))
            .and_then(|()| file.flush())
            .map_err(|error| {
                format!(
                    "could not record artifact owner {}: {error}",
                    path.display()
                )
            })?;
        Ok(Self { _file: file })
    }
}

fn in_use(path: &Path) -> String {
    let owner = fs::read_to_string(path)
        .ok()
        .map(|owner| owner.trim().to_owned())
        .filter(|owner| !owner.is_empty())
        .map(|owner| format!(" ({owner})"))
        .unwrap_or_default();
    format!(
        "artifact root {} is already owned by another golden invocation{owner}; wait for it to finish",
        path.parent().unwrap_or_else(|| Path::new(".")).display()
    )
}

#[cfg(test)]
mod tests {
    use super::ArtifactRootLock;
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
    };

    static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn ownership_is_exclusive_and_released_with_the_guard() {
        let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "skald-golden-artifact-lock-{}-{sequence}",
            std::process::id()
        ));
        let first = ArtifactRootLock::acquire(&root).unwrap();
        let error = ArtifactRootLock::acquire(&root).err().unwrap();
        assert!(error.contains("already owned"), "{error}");
        assert!(error.contains(&format!("process {}", std::process::id())));

        drop(first);
        ArtifactRootLock::acquire(&root).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
