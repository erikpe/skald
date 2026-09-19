//! Fallible fragment storage. Text is disposable storage, never proof.
use crate::backend::plan::LirCallableId;
use std::{
    collections::BTreeMap,
    fs, io,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

pub(in crate::backend) trait FragmentStore {
    fn write(&mut self, key: LirCallableId, text: &str) -> io::Result<()>;
    fn read(&self, key: LirCallableId) -> io::Result<String>;
    fn remove(&mut self, key: LirCallableId) -> io::Result<()>;
}

#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct TempFragmentStore {
    root: PathBuf,
    files: BTreeMap<LirCallableId, PathBuf>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl TempFragmentStore {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::backend) fn create() -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "skald-native-fragments-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root)?;
        Ok(Self {
            root,
            files: BTreeMap::new(),
        })
    }

    fn path(&self, key: LirCallableId) -> io::Result<PathBuf> {
        self.files
            .get(&key)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing fragment"))
    }
}

impl FragmentStore for TempFragmentStore {
    fn write(&mut self, key: LirCallableId, text: &str) -> io::Result<()> {
        if self.files.contains_key(&key) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "duplicate fragment",
            ));
        }
        // The ordinal is based on current insertion count only for storage. Publication
        // order comes from typed keys and is independent of file names.
        let path = self.root.join(format!("fragment-{}.s", self.files.len()));
        fs::write(&path, text)?;
        self.files.insert(key, path);
        Ok(())
    }
    fn read(&self, key: LirCallableId) -> io::Result<String> {
        fs::read_to_string(self.path(key)?)
    }
    fn remove(&mut self, key: LirCallableId) -> io::Result<()> {
        if !self.files.contains_key(&key) {
            return Ok(());
        }
        fs::remove_file(self.path(key)?)?;
        self.files.remove(&key);
        Ok(())
    }
}

impl Drop for TempFragmentStore {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
