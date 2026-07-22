use super::check_repository;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "skald-docs-check-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("fixture directory should be creatable");
        Self { root }
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().expect("fixture path should have a parent"))
            .expect("fixture parent should be creatable");
        fs::write(path, contents).expect("fixture should be writable");
    }

    fn check(&self) -> Vec<super::Diagnostic> {
        check_repository(&self.root).expect("fixture should be checkable")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).expect("fixture should be removable");
    }
}

#[test]
fn accepts_valid_relative_files_and_local_anchors() {
    let fixture = Fixture::new();
    fixture.write("README.md", "See [guide](docs/guide.md#details).\n");
    fixture.write("docs/README.md", "# Docs\n\n- [Guide](guide.md)\n");
    fixture.write("docs/guide.md", "# Guide\n\n## Details\n");

    assert_eq!(fixture.check(), []);
}

#[test]
fn reports_a_missing_file() {
    let fixture = Fixture::new();
    fixture.write("README.md", "See [missing](docs/missing.md).\n");

    let diagnostics = fixture.check();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message().contains("missing file"));
}

#[test]
fn reports_a_missing_anchor() {
    let fixture = Fixture::new();
    fixture.write("README.md", "See [guide](guide.md#missing).\n");
    fixture.write("guide.md", "# Guide\n\n## Present\n");

    let diagnostics = fixture.check();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message().contains("missing anchor"));
}

#[test]
fn decodes_encoded_paths() {
    let fixture = Fixture::new();
    fixture.write(
        "README.md",
        "See [guide](<guide%20one.md#encoded-heading>).\n",
    );
    fixture.write("guide one.md", "# Encoded heading\n");

    assert_eq!(fixture.check(), []);
}

#[test]
fn requires_archive_documents_to_be_linked_from_the_archive_index() {
    let fixture = Fixture::new();
    fixture.write("docs/archive/README.md", "# Archive\n");
    fixture.write("docs/archive/PLAN.md", "# Plan\n");

    let diagnostics = fixture.check();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message()
        .contains("missing required index entry"));

    fixture.write("docs/archive/README.md", "# Archive\n\n- [Plan](PLAN.md)\n");
    assert_eq!(fixture.check(), []);
}

#[test]
fn ignores_links_inside_code() {
    let fixture = Fixture::new();
    fixture.write(
        "README.md",
        "`[inline](missing.md)`\n\n```markdown\n[fenced](missing.md)\n```\n",
    );

    assert_eq!(fixture.check(), []);
}

#[test]
fn validates_reference_link_destinations() {
    let fixture = Fixture::new();
    fixture.write(
        "README.md",
        "See [the guide][guide].\n\n[guide]: missing.md\n",
    );

    let diagnostics = fixture.check();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message().contains("missing file"));
}
