//! Real-process coverage for the host-linker's concurrent pipe handling.

use std::{
    os::unix::fs::PermissionsExt,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use super::*;

const FULL_DUPLEX_HELPER: &str = "SKALD_LINK_FULL_DUPLEX_HELPER";
const HELPER_TEST: &str = "driver::tests::toolchain::process::full_duplex_link_helper";
const PIPE_BYTES: usize = 1024 * 1024;

#[test]
fn full_linker_pipes_complete_under_an_external_watchdog() {
    let mut child = Command::new(std::env::current_exe().expect("test executable must exist"))
        .args(["--exact", HELPER_TEST, "--nocapture"])
        .env(FULL_DUPLEX_HELPER, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("full-duplex linker helper must start");

    let started = Instant::now();
    loop {
        if child
            .try_wait()
            .expect("helper status must be readable")
            .is_some()
        {
            break;
        }
        if started.elapsed() >= Duration::from_secs(5) {
            child.kill().expect("deadlocked helper must be killable");
            let _ = child.wait();
            panic!("linker pipe helper exceeded its five-second watchdog");
        }
        thread::sleep(Duration::from_millis(10));
    }

    let output = child
        .wait_with_output()
        .expect("completed helper output must be readable");
    assert!(
        output.status.success(),
        "full-duplex linker helper failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn full_duplex_link_helper() {
    if std::env::var_os(FULL_DUPLEX_HELPER).is_none() {
        return;
    }

    let directory = TemporaryDirectory::new("driver-full-linker-pipes").unwrap();
    let linker = fake_linker(
        &directory,
        "full-duplex-linker.sh",
        concat!(
            "dd if=/dev/zero bs=1024 count=1024 1>&2 2>/dev/null\n",
            "dd if=/dev/zero bs=1024 count=1024 2>/dev/null\n",
            "cat >/dev/null\n",
            "printf 'linked executable' >\"$output\"\n",
        ),
    );
    let runtime = runtime_placeholder(&directory);
    let output = directory.join("program");

    Toolchain::new(linker, runtime)
        .link_assembly(&"x".repeat(PIPE_BYTES), &output)
        .unwrap();

    assert_eq!(fs::read_to_string(output).unwrap(), "linked executable");
    assert!(temporary_artifacts(directory.path()).is_empty());
}

#[test]
fn failed_status_precedes_a_broken_stdin_pipe_and_keeps_stderr() {
    let directory = TemporaryDirectory::new("driver-rejected-linker-input").unwrap();
    let linker = fake_linker(
        &directory,
        "rejecting-linker.sh",
        "exec 0<&-\nprintf 'rejected input' >&2\nexit 7\n",
    );
    let runtime = runtime_placeholder(&directory);

    let error = Toolchain::new(linker.clone(), runtime)
        .link_assembly(&"x".repeat(PIPE_BYTES), &directory.join("program"))
        .unwrap_err();

    let ToolchainError::Failed {
        tool,
        exit_code,
        details,
    } = error
    else {
        panic!("expected failed linker status, got {error:?}");
    };
    assert_eq!(tool, linker);
    assert_eq!(exit_code, Some(7));
    assert_eq!(details, "rejected input");
    assert!(temporary_artifacts(directory.path()).is_empty());
}

#[test]
fn successful_early_stdin_closure_is_a_write_error() {
    let directory = TemporaryDirectory::new("driver-successful-closed-stdin").unwrap();
    let output = directory.join("program");
    fs::write(&output, "previous executable").unwrap();
    let linker = fake_linker(
        &directory,
        "closed-stdin-linker.sh",
        "exec 0<&-\nsleep 0.05\nprintf 'unpublishable' >\"$output\"\n",
    );
    let runtime = runtime_placeholder(&directory);

    let error = Toolchain::new(linker, runtime)
        .link_assembly(&"x".repeat(PIPE_BYTES * 16), &output)
        .unwrap_err();

    let ToolchainError::WriteAssembly { source, .. } = error else {
        panic!("expected an assembly write error, got {error:?}");
    };
    assert_eq!(source.kind(), std::io::ErrorKind::BrokenPipe);
    assert_eq!(fs::read_to_string(output).unwrap(), "previous executable");
    assert!(temporary_artifacts(directory.path()).is_empty());
}

#[test]
fn publication_failure_cleans_the_completed_temporary_output() {
    let directory = TemporaryDirectory::new("driver-link-publication-failure").unwrap();
    let runtime = runtime_placeholder(&directory);
    let output = directory.join("existing-directory");
    fs::create_dir(&output).unwrap();

    let error = Toolchain::new("unused", runtime)
        .link_assembly_with("assembly", &output, |invocation| {
            let pending = invocation
                .arguments()
                .last()
                .expect("link invocation must end with the output path");
            fs::write(pending, "linked executable").unwrap();
            Ok(LinkObservation::new(Some(0), Vec::new(), Vec::new()))
        })
        .unwrap_err();

    assert!(matches!(error, ToolchainError::Publish { .. }));
    assert!(output.is_dir());
    assert!(temporary_artifacts(directory.path()).is_empty());
}

fn runtime_placeholder(directory: &TemporaryDirectory) -> PathBuf {
    let runtime = directory.join("runtime.a");
    fs::write(&runtime, "runtime").unwrap();
    runtime
}

fn fake_linker(directory: &TemporaryDirectory, name: &str, body: &str) -> OsString {
    let linker = directory.join(name);
    let script = format!(
        concat!(
            "#!/bin/sh\n",
            "output=\n",
            "while [ \"$#\" -gt 0 ]; do\n",
            "  if [ \"$1\" = \"-o\" ]; then output=$2; shift 2; else shift; fi\n",
            "done\n",
            "{body}",
        ),
        body = body,
    );
    fs::write(&linker, script).unwrap();
    let mut permissions = fs::metadata(&linker).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&linker, permissions).unwrap();
    linker.into_os_string()
}
