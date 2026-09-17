//! Internal text schema shared by phase-owned renderers, never authority serialization.
use super::plan::{CallableBinding, PlanView};
use std::fmt::{self, Write};

pub(super) fn header(
    out: &mut dyn Write,
    stage: &str,
    status: &str,
    owner: CallableBinding<'_>,
) -> fmt::Result {
    writeln!(
        out,
        "skald-lir schema=1 stage={stage} status={status} callable={:?}",
        owner.key()
    )?;
    declarations(out, owner.context())?;
    match owner.signature() {
        Ok(signature) => writeln!(out, "callable-signature {signature:?}"),
        Err(reason) => writeln!(out, "callable-signature <unresolved> {reason:?}"),
    }
}

pub(super) fn declarations(out: &mut dyn Write, view: PlanView<'_>) -> fmt::Result {
    writeln!(out, "profile {:?}", view.profile())?;
    writeln!(
        out,
        "policy trace={:?} artifacts={:?}",
        view.runtime_trace(),
        view.artifact_policy()
    )?;
    for (id, fact) in view.layouts().enumerate() {
        writeln!(out, "layout {id} {fact:?}")?;
    }
    for (id, fact) in view.signatures().enumerate() {
        writeln!(out, "signature {id} {fact:?}")?;
    }
    for fact in view.callables() {
        writeln!(out, "callable {fact:?}")?;
    }
    for fact in view.artifacts() {
        writeln!(out, "artifact {fact:?}")?;
    }
    for fact in view.dispatch() {
        writeln!(out, "dispatch {fact:?}")?;
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn test_child_dumps(test: &str) -> Vec<String> {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture"])
        .env("SKALD_LIR_DUMP_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let dumps: Vec<_> = text
        .split("LIR-DUMP-BEGIN\n")
        .skip(1)
        .map(|piece| piece.split_once("LIR-DUMP-END").unwrap().0.to_owned())
        .collect();
    assert!(!dumps.is_empty(), "child did not emit a model dump");
    dumps
}
