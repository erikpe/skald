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

fn read_failure_probe(read_result: i64, close_result: i64) -> String {
    format!(
        concat!(
            "\n.text\n",
            ".globl ska_rt_io_standard_handle\n",
            "ska_rt_io_standard_handle:\n",
            "    mov rax, rdi\n",
            "    ret\n",
            ".globl ska_rt_io_open\n",
            "ska_rt_io_open:\n",
            "    mov rax, 10\n",
            "    ret\n",
            ".globl ska_rt_io_read\n",
            "ska_rt_io_read:\n",
            "    mov rax, {read_result}\n",
            "    ret\n",
            ".globl ska_rt_io_write\n",
            "ska_rt_io_write:\n",
            "    mov rax, rdx\n",
            "    ret\n",
            ".globl ska_rt_io_close\n",
            "ska_rt_io_close:\n",
            "    mov rax, {close_result}\n",
            "    ret\n",
            ".globl ska_rt_alloc\n",
            "ska_rt_alloc:\n",
            "    jmp malloc@PLT\n",
            ".globl ska_rt_free\n",
            "ska_rt_free:\n",
            "    jmp free@PLT\n",
        ),
        read_result = read_result,
        close_result = close_result,
    )
}

fn binary_read_probe() -> &'static str {
    concat!(
        "\n.bss\n",
        ".p2align 3\n",
        ".Lread_stdin_count: .quad 0\n",
        ".Lread_file_count: .quad 0\n",
        ".Lread_stdout_seen: .quad 0\n",
        ".Lread_stderr_seen: .quad 0\n",
        ".Lread_close_seen: .quad 0\n",
        "\n.text\n",
        ".globl ska_rt_io_standard_handle\n",
        "ska_rt_io_standard_handle:\n",
        "    mov rax, rdi\n",
        "    ret\n",
        ".globl ska_rt_io_open\n",
        "ska_rt_io_open:\n",
        "    mov rax, 10\n",
        "    ret\n",
        ".globl ska_rt_io_read\n",
        "ska_rt_io_read:\n",
        "    cmp rdx, 2\n",
        "    jb .Lread_failure\n",
        "    cmp rdi, 0\n",
        "    je .Lread_from_stdin\n",
        "    cmp rdi, 10\n",
        "    jne .Lread_failure\n",
        "    lea r8, [rip + .Lread_file_count]\n",
        "    mov r9d, 0x00008101\n",
        "    mov r10d, 0x000000fe\n",
        "    jmp .Lread_chunk\n",
        ".Lread_from_stdin:\n",
        "    lea r8, [rip + .Lread_stdin_count]\n",
        "    mov r9d, 0x00008000\n",
        "    mov r10d, 0x00000aff\n",
        ".Lread_chunk:\n",
        "    mov rcx, qword ptr [r8]\n",
        "    cmp rcx, 2\n",
        "    je .Lread_eof\n",
        "    ja .Lread_failure\n",
        "    mov eax, r9d\n",
        "    test rcx, rcx\n",
        "    cmovne eax, r10d\n",
        "    mov word ptr [rsi], ax\n",
        "    add qword ptr [r8], 1\n",
        "    mov rax, 2\n",
        "    ret\n",
        ".Lread_eof:\n",
        "    xor eax, eax\n",
        "    ret\n",
        ".globl ska_rt_io_write\n",
        "ska_rt_io_write:\n",
        "    cmp rdx, 4\n",
        "    jne .Lread_failure\n",
        "    cmp rdi, 1\n",
        "    je .Lread_write_stdout\n",
        "    cmp rdi, 2\n",
        "    jne .Lread_failure\n",
        "    cmp dword ptr [rsi], 0x00fe8101\n",
        "    jne .Lread_failure\n",
        "    mov qword ptr [rip + .Lread_stderr_seen], 1\n",
        "    mov rax, 4\n",
        "    ret\n",
        ".Lread_write_stdout:\n",
        "    cmp dword ptr [rsi], 0x0aff8000\n",
        "    jne .Lread_failure\n",
        "    mov qword ptr [rip + .Lread_stdout_seen], 1\n",
        "    mov rax, 4\n",
        "    ret\n",
        ".globl ska_rt_io_close\n",
        "ska_rt_io_close:\n",
        "    cmp rdi, 10\n",
        "    jne .Lread_failure\n",
        "    cmp qword ptr [rip + .Lread_file_count], 2\n",
        "    jne .Lread_failure\n",
        "    mov qword ptr [rip + .Lread_close_seen], 1\n",
        "    xor eax, eax\n",
        "    ret\n",
        ".Lread_failure:\n",
        "    ud2\n",
        ".globl validate\n",
        "validate:\n",
        "    cmp qword ptr [rip + .Lread_stdin_count], 2\n",
        "    jne .Lread_invalid\n",
        "    cmp qword ptr [rip + .Lread_stdout_seen], 1\n",
        "    jne .Lread_invalid\n",
        "    cmp qword ptr [rip + .Lread_stderr_seen], 1\n",
        "    jne .Lread_invalid\n",
        "    cmp qword ptr [rip + .Lread_close_seen], 1\n",
        "    jne .Lread_invalid\n",
        "    xor eax, eax\n",
        "    ret\n",
        ".Lread_invalid:\n",
        "    mov rax, 1\n",
        "    ret\n",
        ".globl ska_rt_alloc\n",
        "ska_rt_alloc:\n",
        "    jmp malloc@PLT\n",
        ".globl ska_rt_free\n",
        "ska_rt_free:\n",
        "    jmp free@PLT\n",
    )
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

fn unused_read_probe() -> &'static str {
    concat!(
        "\n.text\n",
        ".globl ska_rt_io_open\n",
        "ska_rt_io_open:\n",
        "    mov rax, -1\n",
        "    ret\n",
        ".globl ska_rt_io_read\n",
        "ska_rt_io_read:\n",
        "    mov rax, -1\n",
        "    ret\n",
        ".globl ska_rt_io_close\n",
        "ska_rt_io_close:\n",
        "    mov rax, -1\n",
        "    ret\n",
    )
}

fn invalid_write_probe(result: i64) -> String {
    format!(
        concat!(
            "\n.text\n",
            ".globl ska_rt_io_standard_handle\n",
            ".type ska_rt_io_standard_handle, @function\n",
            "ska_rt_io_standard_handle:\n",
            "    mov rax, rdi\n",
            "    ret\n",
            ".size ska_rt_io_standard_handle, .-ska_rt_io_standard_handle\n",
            ".globl ska_rt_io_write\n",
            ".type ska_rt_io_write, @function\n",
            "ska_rt_io_write:\n",
            "    mov rax, {result}\n",
            "    ret\n",
            ".size ska_rt_io_write, .-ska_rt_io_write\n",
            ".globl ska_rt_alloc\n",
            ".type ska_rt_alloc, @function\n",
            "ska_rt_alloc:\n",
            "    jmp malloc@PLT\n",
            ".size ska_rt_alloc, .-ska_rt_alloc\n",
            ".globl ska_rt_free\n",
            ".type ska_rt_free, @function\n",
            "ska_rt_free:\n",
            "    jmp free@PLT\n",
            ".size ska_rt_free, .-ska_rt_free\n",
        ),
        result = result,
    )
}

fn partial_binary_write_probe() -> &'static str {
    concat!(
        "\n.section .rodata\n",
        ".Lio_expected: .byte 0, 128, 255, 10\n",
        "\n.bss\n",
        ".p2align 3\n",
        ".Lio_stdout_count: .quad 0\n",
        ".Lio_stderr_count: .quad 0\n",
        "\n.text\n",
        ".globl ska_rt_io_standard_handle\n",
        ".type ska_rt_io_standard_handle, @function\n",
        "ska_rt_io_standard_handle:\n",
        "    mov rax, rdi\n",
        "    ret\n",
        ".size ska_rt_io_standard_handle, .-ska_rt_io_standard_handle\n",
        ".globl ska_rt_io_write\n",
        ".type ska_rt_io_write, @function\n",
        "ska_rt_io_write:\n",
        "    cmp rdi, 1\n",
        "    je .Lio_write_stdout\n",
        "    cmp rdi, 2\n",
        "    jne .Lio_write_failure\n",
        "    lea r9, [rip + .Lio_stderr_count]\n",
        "    jmp .Lio_write_check\n",
        ".Lio_write_stdout:\n",
        "    lea r9, [rip + .Lio_stdout_count]\n",
        ".Lio_write_check:\n",
        "    mov rcx, qword ptr [r9]\n",
        "    cmp rcx, 4\n",
        "    jae .Lio_write_failure\n",
        "    mov rax, 4\n",
        "    sub rax, rcx\n",
        "    cmp rdx, rax\n",
        "    jne .Lio_write_failure\n",
        "    lea r8, [rip + .Lio_expected]\n",
        "    movzx eax, byte ptr [r8 + rcx]\n",
        "    cmp byte ptr [rsi], al\n",
        "    jne .Lio_write_failure\n",
        "    add qword ptr [r9], 1\n",
        "    mov rax, 1\n",
        "    ret\n",
        ".Lio_write_failure:\n",
        "    ud2\n",
        ".size ska_rt_io_write, .-ska_rt_io_write\n",
        ".globl validate\n",
        ".type validate, @function\n",
        "validate:\n",
        "    cmp qword ptr [rip + .Lio_stdout_count], 4\n",
        "    jne .Lio_validate_failure\n",
        "    cmp qword ptr [rip + .Lio_stderr_count], 4\n",
        "    jne .Lio_validate_failure\n",
        "    mov rax, 0\n",
        "    ret\n",
        ".Lio_validate_failure:\n",
        "    mov rax, 1\n",
        "    ret\n",
        ".size validate, .-validate\n",
        ".globl ska_rt_alloc\n",
        ".type ska_rt_alloc, @function\n",
        "ska_rt_alloc:\n",
        "    jmp malloc@PLT\n",
        ".size ska_rt_alloc, .-ska_rt_alloc\n",
        ".globl ska_rt_free\n",
        ".type ska_rt_free, @function\n",
        "ska_rt_free:\n",
        "    jmp free@PLT\n",
        ".size ska_rt_free, .-ska_rt_free\n",
    )
}

fn closed_descriptor_write_probe() -> &'static str {
    concat!(
        "\n.text\n",
        ".globl ska_rt_io_standard_handle\n",
        ".type ska_rt_io_standard_handle, @function\n",
        "ska_rt_io_standard_handle:\n",
        "    mov rax, rdi\n",
        "    ret\n",
        ".size ska_rt_io_standard_handle, .-ska_rt_io_standard_handle\n",
        ".globl ska_rt_io_write\n",
        ".type ska_rt_io_write, @function\n",
        "ska_rt_io_write:\n",
        "    mov r8, rdi\n",
        "    mov r9, rsi\n",
        "    mov r10, rdx\n",
        "    mov rax, 3\n",
        "    syscall\n",
        "    mov rax, 1\n",
        "    mov rdi, r8\n",
        "    mov rsi, r9\n",
        "    mov rdx, r10\n",
        "    syscall\n",
        "    ret\n",
        ".size ska_rt_io_write, .-ska_rt_io_write\n",
        ".globl ska_rt_alloc\n",
        ".type ska_rt_alloc, @function\n",
        "ska_rt_alloc:\n",
        "    jmp malloc@PLT\n",
        ".size ska_rt_alloc, .-ska_rt_alloc\n",
        ".globl ska_rt_free\n",
        ".type ska_rt_free, @function\n",
        "ska_rt_free:\n",
        "    jmp free@PLT\n",
        ".size ska_rt_free, .-ska_rt_free\n",
    )
}
