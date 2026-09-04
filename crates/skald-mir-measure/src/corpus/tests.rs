use super::load_corpus;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(manifest: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "skald-mir-measure-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("tests/golden")).unwrap();
        fs::create_dir_all(root.join("tests/measurements")).unwrap();
        fs::write(root.join("tests/golden/config.toml"), "schema = 1\n").unwrap();
        fs::write(root.join("entry.ska"), "fn main() -> i64 { return 0; }\n").unwrap();
        fs::write(root.join("second.ska"), "fn main() -> i64 { return 1; }\n").unwrap();
        fs::write(root.join("tests/measurements/corpus.toml"), manifest).unwrap();
        Self { root }
    }

    fn manifest(&self) -> &Path {
        Path::new("tests/measurements/corpus.toml")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn accepts_empty_and_partial_explicit_corpora() {
    let empty = Fixture::new("schema=1\nname='test'\nversion=1\n");
    let corpus = load_corpus(&empty.root, empty.manifest()).unwrap();
    assert!(corpus.workloads().is_empty());

    let partial = Fixture::new(
        "schema=1\nname='test'\nversion=1\n\
         [[workload]]\nid='focused/one'\ncategory='focused'\nentry='entry.ska'\n",
    );
    let corpus = load_corpus(&partial.root, partial.manifest()).unwrap();
    assert_eq!(corpus.workloads().len(), 1);
    assert_eq!(corpus.workloads()[0].id(), "focused/one");
    assert_eq!(
        corpus.workloads()[0].entry(),
        partial.root.join("entry.ska")
    );
}

#[test]
fn rejects_duplicate_ids_and_compilation_identities() {
    let duplicate_id = Fixture::new(
        "schema=1\nname='test'\nversion=1\n\
         [[workload]]\nid='same'\ncategory='a'\nentry='entry.ska'\n\
         [[workload]]\nid='same'\ncategory='b'\nentry='second.ska'\n",
    );
    assert!(load_corpus(&duplicate_id.root, duplicate_id.manifest())
        .unwrap_err()
        .to_string()
        .contains("duplicate workload ID"));

    let duplicate_identity = Fixture::new(
        "schema=1\nname='test'\nversion=1\n\
         [[workload]]\nid='first'\ncategory='a'\nentry='entry.ska'\n\
         [[workload]]\nid='second'\ncategory='b'\nentry='entry.ska'\n",
    );
    assert!(
        load_corpus(&duplicate_identity.root, duplicate_identity.manifest())
            .unwrap_err()
            .to_string()
            .contains("same canonical compilation identity")
    );
}

#[test]
fn rejects_lexical_and_canonical_path_escapes() {
    let lexical = Fixture::new(
        "schema=1\nname='test'\nversion=1\n\
         [[workload]]\nid='escape'\ncategory='a'\nentry='../entry.ska'\n",
    );
    assert!(load_corpus(&lexical.root, lexical.manifest())
        .unwrap_err()
        .to_string()
        .contains("contained repository-relative path"));

    let canonical = Fixture::new(
        "schema=1\nname='test'\nversion=1\n\
         [[workload]]\nid='escape'\ncategory='a'\nentry='escape.ska'\n",
    );
    std::os::unix::fs::symlink("/etc/hosts", canonical.root.join("escape.ska")).unwrap();
    assert!(load_corpus(&canonical.root, canonical.manifest())
        .unwrap_err()
        .to_string()
        .contains("escapes the repository"));
}

#[test]
fn loads_the_reviewed_repository_manifest_in_frozen_order() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let corpus = load_corpus(
        &root,
        Path::new("tests/measurements/local_mir_redundancy.toml"),
    )
    .unwrap();
    assert_eq!(corpus.name(), "local-final-mir-redundancy");
    assert_eq!(corpus.version(), 1);
    assert_eq!(corpus.workloads().len(), 16);
    assert_eq!(corpus.workloads()[0].id(), "focused/local-simplification");
    assert_eq!(corpus.workloads()[15].id(), "benchmark/while-u8");
}
