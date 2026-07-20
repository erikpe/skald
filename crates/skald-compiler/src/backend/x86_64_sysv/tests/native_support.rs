use super::*;

pub(super) fn assert_system_assembler_accepts(output: &str) {
    let mut child = Command::new("cc")
        .args(["-x", "assembler", "-c", "-o", "/dev/null", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the M0 Linux toolchain prerequisite requires `cc`");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(output.as_bytes())
        .unwrap();
    let result = child.wait_with_output().unwrap();

    assert!(
        result.status.success(),
        "assembler rejected generated output:\n{}\nassembly:\n{output}",
        String::from_utf8_lossy(&result.stderr)
    );
}

pub(super) fn run_native_assembly(output: &str) -> std::process::ExitStatus {
    let executable = TemporaryFile::new("native-executable").unwrap();
    let mut child = Command::new("cc")
        .args(["-x", "assembler", "-o"])
        .arg(executable.path())
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the M0 Linux toolchain prerequisite requires `cc`");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(output.as_bytes())
        .unwrap();
    let linked = child.wait_with_output().unwrap();
    assert!(
        linked.status.success(),
        "linker rejected generated output:\n{}\nassembly:\n{output}",
        String::from_utf8_lossy(&linked.stderr)
    );

    Command::new(executable.path()).status().unwrap()
}
