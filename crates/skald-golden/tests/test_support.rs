#[path = "support/temporary.rs"]
mod temporary;

use std::{
    collections::BTreeSet,
    fs,
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};
use temporary::TemporaryWorkspace;

#[test]
fn temporary_workspaces_are_unique_during_parallel_creation() {
    let active_paths = Arc::new(Mutex::new(BTreeSet::new()));
    let paths = (0..32)
        .map(|_| {
            let active_paths = Arc::clone(&active_paths);
            thread::spawn(move || {
                let workspace = TemporaryWorkspace::new("parallel", &[]);
                let path = workspace.root().to_owned();
                assert!(active_paths.lock().unwrap().insert(path.clone()));
                path
            })
        })
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(paths.iter().collect::<BTreeSet<_>>().len(), paths.len());
    assert!(paths.iter().all(|path| !path.exists()));
}

#[test]
fn temporary_workspace_cleans_roots_and_associated_paths_on_drop_and_unwind() {
    let (workspace, dropped) = populated_workspace("drop");
    drop(workspace);
    assert!(dropped.iter().all(|path| !path.exists()));

    let mut unwound = Vec::new();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let (_workspace, paths) = populated_workspace("unwind");
        unwound = paths;
        panic!("exercise unwind cleanup");
    }));
    assert!(result.is_err());
    assert!(unwound.iter().all(|path| !path.exists()));
}

#[test]
fn temporary_workspace_writes_owned_trees_and_canonical_paths() {
    let workspace = TemporaryWorkspace::new("paths", &["artifacts"]);
    workspace.write("nested/data.bin", b"payload");
    let directory = workspace.create_directory("empty/directory");

    assert!(workspace.root().is_absolute());
    assert_eq!(
        fs::read(workspace.path("nested/data.bin")).unwrap(),
        b"payload"
    );
    assert!(directory.is_dir());
    assert_eq!(
        workspace.canonical("nested/data.bin"),
        fs::canonicalize(workspace.path("nested/data.bin")).unwrap()
    );
    assert_eq!(
        workspace.associated_path("artifacts"),
        workspace.root().with_extension("artifacts")
    );
    assert!(catch_unwind(|| workspace.path("../outside")).is_err());
    assert!(catch_unwind(|| workspace.associated_path("undeclared")).is_err());
}

fn populated_workspace(label: &str) -> (TemporaryWorkspace, Vec<PathBuf>) {
    let workspace = TemporaryWorkspace::new(label, &["artifacts", "runtime.a"]);
    workspace.write("nested/source.ska", "fn main() -> i64 { return 0; }\n");
    let artifacts = workspace.associated_path("artifacts");
    let runtime = workspace.associated_path("runtime.a");
    fs::create_dir_all(&artifacts).unwrap();
    fs::write(&runtime, "runtime").unwrap();
    let paths = vec![workspace.root().to_owned(), artifacts, runtime];
    (workspace, paths)
}
