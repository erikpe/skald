use super::coordinator::{execute_with, TaskResult};
use crate::{
    build_plan, select, CompilerConfig, ExecutionOptions, ProcessCommand, RuntimePreparation,
    SchedulerOptions, SelectionOptions, SequentialOptions,
};
use skald_compiler::driver::Toolchain;
use std::{
    fs,
    num::NonZeroUsize,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

#[test]
fn worker_panics_become_internal_failures_with_active_and_pending_ids() {
    let root = temporary_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("config.toml"), "schema=1\n").unwrap();
    fs::write(root.join("a.ska"), "invalid\n").unwrap();
    fs::write(root.join("b.ska"), "invalid\n").unwrap();
    fs::write(
        root.join("panic.golden.toml"),
        r#"schema=1
[[test]]
name="a"
mode="compile-fail"
source="a.ska"
expect={stderr={inline="expected"}}
[[test]]
name="b"
mode="compile-fail"
source="b.ska"
expect={stderr={inline="expected"}}
"#,
    )
    .unwrap();
    let plan = build_plan(&root, root.join("artifacts"), &[]).unwrap();
    let selected = select(&plan, &SelectionOptions::default()).unwrap();
    let options = SequentialOptions::new(
        CompilerConfig::new("unused-compiler", &root),
        RuntimePreparation::new(ProcessCommand::new("unused-runtime", &root), "unused.a"),
        Toolchain::new("unused-linker", "unused.a"),
        ExecutionOptions::new(root.join("temporary")),
    );
    let executor = |_, _: &SequentialOptions| -> TaskResult { panic!("injected worker defect") };

    let execution = execute_with(
        &selected,
        &options,
        SchedulerOptions::new(NonZeroUsize::MIN),
        &executor,
    );

    let failure = execution.scheduler_failure().unwrap();
    assert!(failure.message().contains("injected worker defect"));
    assert_eq!(failure.active_nodes().len(), 1);
    assert_eq!(failure.pending_nodes().len(), 1);
    assert!(execution
        .leaves()
        .iter()
        .all(|leaf| !leaf.status().passed()));
    fs::remove_dir_all(root).unwrap();
}

fn temporary_root() -> PathBuf {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "skald-golden-scheduler-unit-{}-{sequence}",
        std::process::id()
    ))
}
