use crate::ProcessEnvironment;

const INHERITED_NAMES: &[&str] = &["PATH", "CC", "SKALD_RUNTIME_ARCHIVE", "SKALD_STDLIB_ROOT"];

/// Snapshots only environment values required to locate repository toolchains.
pub fn allowlisted_environment() -> ProcessEnvironment {
    let mut environment = ProcessEnvironment::new();
    for name in INHERITED_NAMES {
        if let Some(value) = std::env::var_os(name) {
            environment.insert(*name, value);
        }
    }
    environment
}
