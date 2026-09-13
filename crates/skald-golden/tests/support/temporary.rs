use std::{
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_WORKSPACE: AtomicUsize = AtomicUsize::new(0);

/// Owns a unique temporary root and any declared sibling artifact paths.
pub(crate) struct TemporaryWorkspace {
    root: PathBuf,
    associated_paths: Vec<PathBuf>,
}

impl TemporaryWorkspace {
    pub(crate) fn new(label: &str, associated_extensions: &[&str]) -> Self {
        let temporary_root = absolute_temporary_root();
        let (root, associated_paths) = loop {
            let sequence = NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed);
            let candidate = temporary_root.join(format!(
                "skald-golden-{label}-{}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&candidate) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => panic!(
                    "could not create temporary workspace {}: {error}",
                    candidate.display()
                ),
            }
            let associated_paths = associated_extensions
                .iter()
                .map(|extension| candidate.with_extension(extension))
                .collect::<Vec<_>>();
            if associated_paths
                .iter()
                .any(|path| fs::symlink_metadata(path).is_ok())
            {
                remove_path(&candidate);
                continue;
            }
            break (candidate, associated_paths);
        };
        Self {
            root,
            associated_paths,
        }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn path(&self, relative: impl AsRef<Path>) -> PathBuf {
        let relative = relative.as_ref();
        assert_owned_relative_path(relative);
        self.root.join(relative)
    }

    #[allow(dead_code)] // Planning fixtures own child paths rather than siblings.
    pub(crate) fn associated_path(&self, extension: &str) -> PathBuf {
        let path = self.root.with_extension(extension);
        assert!(
            self.associated_paths.contains(&path),
            "associated temporary path was not declared: {}",
            path.display()
        );
        path
    }

    pub(crate) fn write(&self, relative: impl AsRef<Path>, contents: impl AsRef<[u8]>) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[allow(dead_code)] // Execution fixtures create parents through `write`.
    pub(crate) fn create_directory(&self, relative: impl AsRef<Path>) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[allow(dead_code)] // Only planning assertions need canonical fixture paths.
    pub(crate) fn canonical(&self, relative: impl AsRef<Path>) -> PathBuf {
        fs::canonicalize(self.path(relative)).unwrap()
    }
}

impl Drop for TemporaryWorkspace {
    fn drop(&mut self) {
        for path in self
            .associated_paths
            .iter()
            .rev()
            .chain(std::iter::once(&self.root))
        {
            remove_path(path);
        }
    }
}

fn absolute_temporary_root() -> PathBuf {
    let root = std::env::temp_dir();
    if root.is_absolute() {
        root
    } else {
        std::env::current_dir().unwrap().join(root)
    }
}

fn assert_owned_relative_path(path: &Path) {
    assert!(
        !path.as_os_str().is_empty()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_) | Component::CurDir)),
        "temporary workspace paths must be non-empty and relative: {}",
        path.display()
    );
}

fn remove_path(path: &Path) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_dir() {
        let _ = fs::remove_dir_all(path);
    } else {
        let _ = fs::remove_file(path);
    }
}
