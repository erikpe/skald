use std::path::Path;

#[allow(dead_code)] // Direct process tests need only the native fake binary.
pub(crate) fn fake_compiler() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_skald-golden-fake-compiler"))
}

#[allow(dead_code)] // Direct process tests need only the native fake binary.
pub(crate) fn fake_linker() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_skald-golden-fake-linker"))
}

pub(crate) fn fake_process() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_skald-golden-fake-process"))
}
