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
