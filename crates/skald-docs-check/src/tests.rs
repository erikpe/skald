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
fn a_shorter_marker_does_not_close_a_longer_fence() {
    let fixture = Fixture::new();
    fixture.write(
        "README.md",
        "````markdown\n[hidden](first.md)\n```\n[still hidden](second.md)\n````\n[visible](missing.md)\n",
    );

    let diagnostics = fixture.check();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].to_string(),
        "README.md:6: missing file `missing.md`"
    );
}

#[test]
fn trailing_text_prevents_a_fence_from_closing() {
    let fixture = Fixture::new();
    fixture.write(
        "README.md",
        "~~~markdown\n[hidden](first.md)\n~~~ trailing text\n[still hidden](second.md)\n~~~\n[visible](missing.md)\n",
    );

    let diagnostics = fixture.check();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].to_string(),
        "README.md:6: missing file `missing.md`"
    );
}

#[test]
fn recognizes_nested_and_escaped_link_labels() {
    let fixture = Fixture::new();
    fixture.write(
        "README.md",
        "\\[literal](ignored.md)\n[outer [inner]](nested.md)\n[escaped \\] label](escaped.md)\n",
    );

    let diagnostics = fixture.check();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics[0].to_string(),
        "README.md:2: missing file `nested.md`"
    );
    assert_eq!(
        diagnostics[1].to_string(),
        "README.md:3: missing file `escaped.md`"
    );
}

#[test]
fn unescapes_destination_punctuation_and_ignores_link_titles() {
    let fixture = Fixture::new();
    fixture.write(
        "README.md",
        "[escaped](guide\\(one\\).md \"title\")\n[angle](<guide two.md> \"title\")\n",
    );
    fixture.write("guide(one).md", "# Escaped\n");
    fixture.write("guide two.md", "# Angle\n");

    assert_eq!(fixture.check(), []);
}

#[test]
fn resolves_forward_full_collapsed_and_shortcut_references() {
    let fixture = Fixture::new();
    fixture.write(
        "README.md",
        "[Forward][  GUIDE label ]\n[Collapsed][]\n[Shortcut]\n\n[guide   LABEL]: guide.md\n[GUIDE LABEL]: ignored-duplicate.md\n[collapsed]: collapsed.md\n[shortcut]: shortcut.md\n",
    );
    fixture.write("guide.md", "# Guide\n");
    fixture.write("collapsed.md", "# Collapsed\n");
    fixture.write("shortcut.md", "# Shortcut\n");

    assert_eq!(fixture.check(), []);
}

#[test]
fn reports_undefined_explicit_reference_links() {
    let fixture = Fixture::new();
    fixture.write(
        "README.md",
        "[full][missing full]\n[collapsed][]\n[ordinary brackets]\n",
    );

    let diagnostics = fixture.check();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics[0].to_string(),
        "README.md:1: undefined reference link `[missing full]`"
    );
    assert_eq!(
        diagnostics[1].to_string(),
        "README.md:2: undefined reference link `[collapsed]`"
    );
}

#[test]
fn validates_reference_destinations_at_the_use_site() {
    let fixture = Fixture::new();
    fixture.write(
        "README.md",
        "See [the guide][guide].\n\n[guide]: missing.md\n",
    );

    let diagnostics = fixture.check();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].to_string(),
        "README.md:1: missing file `missing.md`"
    );
}

#[test]
fn assigns_stable_suffixes_to_duplicate_heading_anchors() {
    let fixture = Fixture::new();
    fixture.write(
        "README.md",
        "[first](guide.md#repeated)\n[second](guide.md#repeated-1)\n[third](guide.md#repeated-2)\n",
    );
    fixture.write("guide.md", "# Repeated\n\n## Repeated\n\n### Repeated\n");

    assert_eq!(fixture.check(), []);
}
