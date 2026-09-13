use std::{fs, path::Path};

use crate::support::TemporaryDirectory;

pub(crate) struct ModuleFixture {
    directory: TemporaryDirectory,
}

impl ModuleFixture {
    pub(crate) fn new(label: &str, variant: usize) -> Self {
        Self {
            directory: TemporaryDirectory::new(&format!("{label}-{variant}"))
                .expect("module fixture directory must be creatable"),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        self.directory.path()
    }
}

pub(crate) fn write_source(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[cfg(unix)]
pub(crate) fn link_directory(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(windows)]
pub(crate) fn link_directory(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_dir(target, link).unwrap();
}
