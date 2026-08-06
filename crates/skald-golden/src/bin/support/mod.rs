use std::{
    env, fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

const ACTIVE_ENV: &str = "SKALD_FAKE_ACTIVE";
const PEAK_ENV: &str = "SKALD_FAKE_PEAK";
const DELAY_ENV: &str = "SKALD_FAKE_DELAY_MS";

pub(super) struct ActivityGuard {
    active: PathBuf,
}

impl ActivityGuard {
    pub(super) fn from_environment() -> Result<Option<Self>, String> {
        let (active, peak) = match (env::var_os(ACTIVE_ENV), env::var_os(PEAK_ENV)) {
            (Some(active), Some(peak)) => (active, peak),
            (None, None) => return Ok(None),
            _ => return Err(format!("{ACTIVE_ENV} and {PEAK_ENV} must be set together")),
        };
        let active = PathBuf::from(active);
        let peak = PathBuf::from(peak);
        update(&active, &peak, 1)?;
        let guard = Self { active };
        if let Some(delay) = env::var_os(DELAY_ENV) {
            let milliseconds = delay
                .to_str()
                .ok_or_else(|| format!("{DELAY_ENV} must be UTF-8"))?
                .parse::<u64>()
                .map_err(display)?;
            thread::sleep(Duration::from_millis(milliseconds));
        }
        Ok(Some(guard))
    }
}

impl Drop for ActivityGuard {
    fn drop(&mut self) {
        let lock = lock_path(&self.active);
        let Ok(_lock) = FileLock::acquire(&lock) else {
            return;
        };
        let current = read_count(&self.active).unwrap_or(1);
        let _ = fs::write(&self.active, current.saturating_sub(1).to_string());
    }
}

fn update(active: &Path, peak: &Path, change: u64) -> Result<(), String> {
    let lock = lock_path(active);
    let _lock = FileLock::acquire(&lock)?;
    let current = read_count(active).unwrap_or(0) + change;
    let previous_peak = read_count(peak).unwrap_or(0);
    fs::write(active, current.to_string()).map_err(display)?;
    fs::write(peak, previous_peak.max(current).to_string()).map_err(display)
}

fn read_count(path: &Path) -> Option<u64> {
    fs::read_to_string(path).ok()?.parse().ok()
}

fn lock_path(active: &Path) -> PathBuf {
    let mut value = active.as_os_str().to_owned();
    value.push(".lock");
    PathBuf::from(value)
}

struct FileLock(PathBuf);

impl FileLock {
    fn acquire(path: &Path) -> Result<Self, String> {
        for _ in 0..10_000 {
            match fs::create_dir(path) {
                Ok(()) => return Ok(Self(path.to_path_buf())),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    thread::yield_now();
                }
                Err(error) => return Err(display(error)),
            }
        }
        Err(format!(
            "could not acquire fake activity lock {}",
            path.display()
        ))
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
