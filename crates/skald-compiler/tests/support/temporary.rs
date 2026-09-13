use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_RESOURCE_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) struct TemporaryFile {
    path: PathBuf,
}

impl TemporaryFile {
    pub(crate) fn new(label: &str) -> io::Result<Self> {
        let path = unique_temporary_path(label);
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        Ok(Self { path })
    }

    // Some integration binaries use this file only through process capture.
    #[allow(dead_code)]
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn open_for_output(&self) -> io::Result<File> {
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.path)
    }

    #[allow(dead_code)]
    pub(crate) fn read(&self) -> io::Result<Vec<u8>> {
        fs::read(&self.path)
    }

    pub(super) fn read_prefix(&self, limit: usize) -> io::Result<(Vec<u8>, u64)> {
        let mut file = File::open(&self.path)?;
        let observed_length = file.metadata()?.len();
        let mut retained =
            Vec::with_capacity(usize::try_from(observed_length.min(limit as u64)).unwrap_or(limit));
        file.by_ref()
            .take(limit as u64)
            .read_to_end(&mut retained)?;
        Ok((retained, observed_length))
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

// Directory fixtures are needed by integration suites that do not all compile
// into the same Cargo test binary.
#[allow(dead_code)]
pub(crate) struct TemporaryDirectory {
    path: PathBuf,
}

#[allow(dead_code)]
impl TemporaryDirectory {
    pub(crate) fn new(label: &str) -> io::Result<Self> {
        let path = unique_temporary_path(label);
        fs::create_dir(&path)?;
        Ok(Self { path })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn unique_temporary_path(label: &str) -> PathBuf {
    let label = sanitized_label(label);
    let process = std::process::id();
    let sequence = NEXT_RESOURCE_ID.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir().join(format!(
        "skald-integration-{label}-{process}-{timestamp}-{sequence}"
    ))
}

fn sanitized_label(label: &str) -> String {
    let label: String = label
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect();
    if label.is_empty() {
        "resource".to_owned()
    } else {
        label
    }
}
