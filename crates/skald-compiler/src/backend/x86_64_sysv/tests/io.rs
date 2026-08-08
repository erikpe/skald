use super::io_probes::{
    binary_read_probe, closed_descriptor_write_probe, invalid_write_probe,
    partial_binary_write_probe, read_failure_probe, unused_read_probe,
};
use super::*;

const RUNTIME_SYMBOLS: [&str; 5] = [
    "ska_rt_io_standard_handle",
    "ska_rt_io_open",
    "ska_rt_io_read",
    "ska_rt_io_write",
    "ska_rt_io_close",
];

#[test]
fn verified_io_selects_each_exact_runtime_symbol_and_assembles() {
    let output = emit_assembly(Target::X86_64SysV, &fixture_io_program()).unwrap();

    for symbol in RUNTIME_SYMBOLS {
        assert_eq!(
            output.matches(&format!("call {symbol}\n")).count(),
            1,
            "expected one call to {symbol}\n{output}"
        );
    }
    assert!(!output.contains("call .Lska.fn.std.io._io_"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn io_ranges_use_checked_offsets_and_raw_pointer_length_arguments() {
    let output = emit_assembly(Target::X86_64SysV, &fixture_io_program()).unwrap();

    // Open passes the full byte range in rdi/rsi. Read and write pass a
    // checked remaining range in rsi/rdx and never pass an array descriptor.
    assert!(output.contains("mov rsi, qword ptr [rax + 8]"));
    assert!(output.contains("lea rdi, [rax + 16]"));
    assert!(output.contains("mov rdx, qword ptr [rax + 8]"));
    assert!(output.contains("lea rsi, [rax + 16]"));
    assert!(output.contains("add rsi, r10"));
    assert!(output.contains("sub rdx, r10"));

    // Unsigned offsets equal to the byte length are valid. Larger offsets use
    // the array failure edge before an I/O call can be reached.
    assert!(output.contains("jb "));
    assert!(output.contains("je "));
    assert!(output.contains("mov rax, 0xffffffffffffffff"));

    // Null empty-array descriptors branch before either header load or LEA,
    // and materialize the runtime's permitted null/zero range.
    assert!(output.contains("io_"));
    assert!(output.contains("range_empty"));
    assert!(output.contains("mov rsi, 0\n"));
    assert!(output.contains("mov rdx, 0\n"));
}

#[test]
fn io_calls_preserve_alignment_result_homes_and_backing_anchor_lifetimes() {
    let output = emit_assembly(Target::X86_64SysV, &fixture_io_program()).unwrap();

    for symbol in RUNTIME_SYMBOLS {
        let call = output
            .find(&format!("call {symbol}\n"))
            .expect("runtime call exists");
        let following = &output[call..output.len().min(call + 120)];
        assert!(
            following.contains("mov qword ptr [rbp - "),
            "signed result is stored immediately after {symbol}\n{following}"
        );
    }
    assert!(!output.contains("sub rsp, 8\n"));
    assert!(!output.contains("add rsp, 8\n"));

    // The verified backing anchor remains a frame-resident owner until the
    // full-expression cleanup after each host call.
    assert!(output.matches("call ska_rt_io_open\n").count() == 1);
    assert!(output.matches("call ska_rt_io_read\n").count() == 1);
    assert!(output.matches("call ska_rt_io_write\n").count() == 1);
    assert!(output.contains("call .Lska_array_0_release"));
}

#[test]
fn nested_aliases_and_shared_arrays_select_their_backing_layouts() {
    let program = crate::mir::test_fixtures::io_program_with_additional_bodies(concat!(
        "public fn write_nested(handle: i64, ref sources: u8[][], offset: u64) -> i64 {\n",
        "  return _io_write(handle, sources[0], offset);\n",
        "}\n",
        "public fn write_shared(handle: i64, source: shared u8[], offset: u64) -> i64 {\n",
        "  return _io_write(handle, *source, offset);\n",
        "}\n",
    ));

    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assert_eq!(output.matches("call ska_rt_io_write\n").count(), 3);
    assert!(output.contains("lea rsi, [rax + 16]"));
    assert!(output.contains("mov rdx, qword ptr [rax + 16]"));
    assert!(output.contains("lea rsi, [rax + 24]"));
    assert!(output.contains("sub rdx, r10"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn canonical_write_all_loop_selects_verified_calls_and_failure_paths() {
    let program = fixture_standard_io_program(concat!(
        "import std::io;\n",
        "import std::str;\n",
        "fn main() -> i64 {\n",
        "  var text: std::str::Str = \"output\";\n",
        "  std::io::write_stdout(text);\n",
        "  std::io::write_stderr(text);\n",
        "  return 0;\n",
        "}\n",
    ));
    let write_all = program
        .definitions
        .iter()
        .find(|definition| {
            program
                .declarations
                .get(definition.function)
                .is_some_and(|declaration| declaration.name == "_write_all")
        })
        .expect("canonical standard library defines one write-all helper");
    let write_all_symbol = super::super::symbol::callable(&program, write_all.callable());
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    let write_all = function_assembly(&output, &write_all_symbol);

    assert_eq!(
        write_all
            .matches("call ska_rt_io_standard_handle\n")
            .count(),
        1
    );
    assert_eq!(write_all.matches("call ska_rt_io_write\n").count(), 1);
    assert!(write_all.matches("call ska_rt_panic\n").count() >= 3);
    assert!(output.contains("io: failed to write stdout"));
    assert!(output.contains("io: failed to write stderr"));
    assert!(output.contains("io: invalid runtime result"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn canonical_read_loops_select_open_read_close_in_library_order() {
    let program = fixture_standard_io_program(concat!(
        "import std::io;\n",
        "import std::str;\n",
        "fn main() -> i64 {\n",
        "  var path: std::str::Str = \"input.bin\";\n",
        "  var stdin: std::str::Str = std::io::read_stdin();\n",
        "  var file: std::str::Str = std::io::read_file(path);\n",
        "  return 0;\n",
        "}\n",
    ));
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();

    let read_all = function_named_assembly(&program, &output, "_read_all");
    assert_eq!(read_all.matches("call ska_rt_io_read\n").count(), 1);
    assert!(output.contains("io: failed to read stdin"));
    assert!(output.contains("io: failed to read file"));
    assert!(output.contains("io: invalid runtime result"));

    let read_stdin = function_named_assembly(&program, &output, "read_stdin");
    assert_eq!(
        read_stdin
            .matches("call ska_rt_io_standard_handle\n")
            .count(),
        1
    );
    let read_file = function_named_assembly(&program, &output, "read_file");
    let open = read_file.find("call ska_rt_io_open\n").unwrap();
    let close = read_file.find("call ska_rt_io_close\n").unwrap();
    assert!(open < close);
    assert_system_assembler_accepts(&output);
}

#[test]
fn public_reads_preserve_partial_binary_input_and_close_files_natively() {
    let program = fixture_standard_io_program(concat!(
        "import std::io;\n",
        "import std::str;\n",
        "extern fn validate() -> i64;\n",
        "fn main() -> i64 {\n",
        "  var path: std::str::Str = \"payload\";\n",
        "  var stdin: std::str::Str = std::io::read_stdin();\n",
        "  var file: std::str::Str = std::io::read_file(path);\n",
        "  std::io::write_stdout(stdin);\n",
        "  std::io::write_stderr(file);\n",
        "  return validate();\n",
        "}\n",
    ));
    let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    output.push_str(binary_read_probe());

    let result = run_native_assembly_output(&output);
    assert!(
        result.status.success(),
        "binary read probe failed with {:?}: {}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

#[test]
fn public_reads_select_read_close_and_invalid_result_failures_natively() {
    for (app, read_result, close_result, expected) in [
        (
            "var value: std::str::Str = std::io::read_stdin();",
            -5,
            0,
            "panic: io: failed to read stdin\n",
        ),
        (
            concat!(
                "var path: std::str::Str = \"payload\";\n",
                "  var value: std::str::Str = std::io::read_file(path);",
            ),
            -5,
            0,
            "panic: io: failed to read file\n",
        ),
        (
            concat!(
                "var path: std::str::Str = \"payload\";\n",
                "  var value: std::str::Str = std::io::read_file(path);",
            ),
            0,
            -5,
            "panic: io: failed to close file\n",
        ),
        (
            "var value: std::str::Str = std::io::read_stdin();",
            65,
            0,
            "panic: io: invalid runtime result\n",
        ),
    ] {
        let program = fixture_standard_io_program(&format!(
            concat!(
                "import std::io;\n",
                "import std::str;\n",
                "fn main() -> i64 {{\n",
                "  {app}\n",
                "  return 0;\n",
                "}}\n",
            ),
            app = app,
        ));
        let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
        output.push_str(&read_failure_probe(read_result, close_result));
        output.push_str(native_panic_reporter());

        let result = run_native_assembly_output(&output);
        assert!(
            !result.status.success(),
            "failure probe unexpectedly returned"
        );
        assert_eq!(result.stderr, expected.as_bytes());
    }
}

#[test]
fn read_capacity_overflow_selects_the_standard_library_failure_natively() {
    let program = fixture_standard_io_program_with_additional_bodies(
        concat!(
            "import std::io;\n",
            "fn main() -> i64 {\n",
            "  std::io::provoke_input_too_large();\n",
            "  return 0;\n",
            "}\n",
        ),
        concat!(
            "public fn provoke_input_too_large() -> unit {\n",
            "  var capacity: u64 = _next_read_capacity(4611686018427387904u);\n",
            "  return;\n",
            "}\n",
        ),
    );
    let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    output.push_str(&read_failure_probe(0, 0));
    output.push_str(native_panic_reporter());

    let result = run_native_assembly_output(&output);
    assert!(!result.status.success());
    assert_eq!(result.stderr, b"panic: io: input too large\n");
}

fn function_named_assembly<'a>(program: &MirProgram, output: &'a str, name: &str) -> &'a str {
    let definition = program
        .definitions
        .iter()
        .find(|definition| {
            program
                .declarations
                .get(definition.function)
                .is_some_and(|declaration| declaration.name == name)
        })
        .unwrap_or_else(|| panic!("canonical standard library defines `{name}`"));
    let symbol = super::super::symbol::callable(program, definition.callable());
    function_assembly(output, &symbol)
}

#[test]
fn public_writes_complete_forced_partial_binary_transfers_natively() {
    let program = fixture_standard_io_program(concat!(
        "import std::io;\n",
        "import std::str;\n",
        "extern fn validate() -> i64;\n",
        "fn main() -> i64 {\n",
        "  var empty: std::str::Str = \"\";\n",
        "  var binary: std::str::Str = \"\\0\\x80\\xff\\n\";\n",
        "  std::io::write_stdout(empty);\n",
        "  std::io::write_stderr(empty);\n",
        "  std::io::write_stdout(binary);\n",
        "  std::io::write_stderr(binary);\n",
        "  return validate();\n",
        "}\n",
    ));
    let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    output.push_str(partial_binary_write_probe());
    output.push_str(unused_read_probe());

    let result = run_native_assembly_output(&output);
    assert!(
        result.status.success(),
        "partial binary write probe failed with {:?}: {}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

#[test]
fn public_writes_reject_invalid_runtime_progress_natively() {
    for invalid_result in [0, 2] {
        let program = fixture_standard_io_program(concat!(
            "import std::io;\n",
            "import std::str;\n",
            "fn main() -> i64 {\n",
            "  var text: std::str::Str = \"x\";\n",
            "  std::io::write_stdout(text);\n",
            "  return 0;\n",
            "}\n",
        ));
        let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
        output.push_str(&invalid_write_probe(invalid_result));
        output.push_str(unused_read_probe());
        output.push_str(native_panic_reporter());

        let result = run_native_assembly_output(&output);
        assert!(!result.status.success(), "invalid result {invalid_result}");
        assert!(result.stdout.is_empty());
        assert_eq!(
            result.stderr, b"panic: io: invalid runtime result\n",
            "invalid result {invalid_result}"
        );
    }
}

#[test]
fn public_writes_select_stream_failures_for_closed_descriptors() {
    for (function, expected_stderr) in [
        (
            "write_stdout",
            b"panic: io: failed to write stdout\n".as_slice(),
        ),
        ("write_stderr", b"".as_slice()),
    ] {
        let program = fixture_standard_io_program(&format!(
            concat!(
                "import std::io;\n",
                "import std::str;\n",
                "fn main() -> i64 {{\n",
                "  var text: std::str::Str = \"failure\";\n",
                "  std::io::{function}(text);\n",
                "  return 0;\n",
                "}}\n",
            ),
            function = function,
        ));
        let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
        output.push_str(closed_descriptor_write_probe());
        output.push_str(unused_read_probe());
        output.push_str(native_panic_reporter());

        let result = run_native_assembly_output(&output);
        assert!(!result.status.success(), "{function} unexpectedly returned");
        assert_eq!(result.stderr, expected_stderr, "{function}");
    }
}
