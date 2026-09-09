//! Source-level guards for compiler phase dependency direction.

use std::{fs, path::Path};

#[test]
fn production_resolution_does_not_depend_on_type_checking() {
    let resolve = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/resolve");
    let mut violations = Vec::new();
    visit_rust_sources(&resolve, &mut |path| {
        let relative = path.strip_prefix(&resolve).unwrap();
        if relative
            .components()
            .any(|part| part.as_os_str() == "tests")
            || path
                .file_stem()
                .is_some_and(|name| name.to_string_lossy().ends_with("_tests"))
        {
            return;
        }
        let source = fs::read_to_string(path).unwrap();
        if source.contains("crate::typeck") || source.contains("typeck::") {
            violations.push(relative.display().to_string());
        }
    });
    assert!(
        violations.is_empty(),
        "resolution must depend only on its own or earlier/neutral phases: {violations:?}"
    );
}

fn visit_rust_sources(directory: &Path, visit: &mut impl FnMut(&Path)) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            visit_rust_sources(&path, visit);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            visit(&path);
        }
    }
}
