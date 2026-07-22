use super::*;

pub(super) fn assert_system_assembler_accepts(output: &str) {
    let mut child = Command::new("cc")
        .args(["-x", "assembler", "-c", "-o", "/dev/null", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("native backend tests require the Linux `cc` toolchain");
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
    let (_executable, mut command) = build_native_assembly(output);
    command.status().unwrap()
}

pub(super) fn run_native_assembly_output(output: &str) -> std::process::Output {
    let (_executable, mut command) = build_native_assembly(output);
    command.output().unwrap()
}

fn build_native_assembly(output: &str) -> (TemporaryFile, Command) {
    let executable = TemporaryFile::new("native-executable").unwrap();
    // Backend execution tests deliberately avoid depending on a prebuilt C
    // runtime. Supply only the link guard; driver and golden tests exercise
    // the real archive boundary.
    let linkable_output = format!(
        ".text\n.globl {0}\n.type {0}, @function\n{0}:\n    ret\n.size {0}, .-{0}\n\n{output}",
        RUNTIME_ABI_MARKER_SYMBOL,
    );
    let mut child = Command::new("cc")
        .args(["-x", "assembler", "-o"])
        .arg(executable.path())
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("native backend tests require the Linux `cc` toolchain");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(linkable_output.as_bytes())
        .unwrap();
    let linked = child.wait_with_output().unwrap();
    assert!(
        linked.status.success(),
        "linker rejected generated output:\n{}\nassembly:\n{output}",
        String::from_utf8_lossy(&linked.stderr)
    );

    let command = Command::new(executable.path());
    (executable, command)
}
