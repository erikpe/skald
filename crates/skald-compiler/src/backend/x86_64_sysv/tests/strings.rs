use super::*;
use crate::{
    mir::{lower_hir, MirStaticAllocationOrigin},
    resolve::resolve_module_graph,
    test_support::{
        load_module_sources, load_module_sources_with_standard_library, lower_hir_to_final_mir,
    },
    typeck::type_check,
};

const VALID_STR: &str = concat!(
    "public class Str {\n",
    "  private _storage: shared u8[];\n",
    "  private _start: i64;\n",
    "  private _length: u64;\n",
    "  init() {\n",
    "    self._storage = new u8[]();\n",
    "    self._start = 0;\n",
    "    self._length = 0u;\n",
    "  }\n",
    "}\n",
);
fn string_program(app: &str) -> MirProgram {
    string_program_with_item(app, VALID_STR)
}

fn string_program_with_item(app: &str, string_item: &str) -> MirProgram {
    let (_workspace, graph) =
        load_module_sources("app", &[("app.ska", app), ("std/str.ska", string_item)]);
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "resolution failed: {:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(
        checked.diagnostics.is_empty(),
        "type checking failed: {:?}",
        checked.diagnostics
    );
    lower_hir(&checked.hir.expect("valid string source must produce HIR"))
}

fn panic_program(app: &str) -> MirProgram {
    canonical_string_program(app)
}

fn canonical_string_program(app: &str) -> MirProgram {
    let (_workspace, graph) = load_module_sources_with_standard_library("app", &[("app.ska", app)]);
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    lower_hir_to_final_mir(&checked.hir.expect("valid panic source must produce HIR"))
}

fn string_assembly(app: &str) -> String {
    emit_assembly(Target::X86_64SysV, &string_program(app)).unwrap()
}

#[test]
fn emits_pooled_aligned_immutable_literal_backings_with_exact_bytes() {
    let source = concat!(
        "from std::str import Str;\n",
        "fn empty() -> Str { return \"\"; }\n",
        "fn ascii() -> Str { return \"abc\"; }\n",
        "fn duplicate() -> Str { return \"abc\"; }\n",
        "fn binary() -> Str { return \"a\\0\\x80\\xff\"; }\n",
        "fn escaped() -> Str { return \"\\\"\\\\\\n\\r\\t\\x01\\x7f\"; }\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let output = string_assembly(source);
    assert_eq!(output, string_assembly(source));

    assert_eq!(output.matches(".type .Lska_literal_").count(), 4);
    assert!(output.contains(concat!(
        ".section .data.rel.ro.local,\"aw\",@progbits\n",
        ".p2align 3\n",
        ".type .Lska_literal_0_backing, @object\n",
        ".Lska_literal_0_backing:\n",
        "    .quad 0xffffffffffffffff\n",
        "    .quad .Lska_array_0_shared_metadata\n",
        "    .quad 0\n",
        ".size .Lska_literal_0_backing, .-.Lska_literal_0_backing\n",
    )));
    assert!(output.contains("    .quad 3\n    .ascii \"abc\"\n"));
    assert!(output.contains("    .quad 4\n    .ascii \"a\\000\\200\\377\"\n"));
    assert!(output.contains("    .quad 7\n    .ascii \"\\\"\\\\\\n\\r\\t\\001\\177\"\n"));
    let ascii = function_assembly(&output, ".Lska.fn.app.ascii.f1");
    assert!(ascii.contains("lea rax, [rip + .Lska_literal_1_backing]"));
    assert!(ascii.contains("mov qword ptr [rdx], rax"));
    assert!(ascii.contains("mov qword ptr [rdx + 8], rax"));
    assert!(ascii.contains("mov qword ptr [rdx + 16], rax"));
    assert!(!ascii.contains("call .Lska_array_0_copy_element"));
    for (function, name) in ["empty", "ascii", "duplicate", "binary", "escaped"]
        .into_iter()
        .enumerate()
    {
        assert!(
            !function_assembly(&output, &format!(".Lska.fn.app.{name}.f{function}"))
                .contains("call ska_rt_alloc"),
            "literal materialization must not allocate"
        );
    }
    assert_system_assembler_accepts(&output);
}

#[test]
fn repeated_literals_copy_assign_pass_and_return_with_immortal_backing() {
    let output = string_assembly(concat!(
        "from std::str import Str;\n",
        "fn identity(value: Str) -> Str { return value; }\n",
        "fn consume(value: Str) -> unit {}\n",
        "fn main() -> i64 {\n",
        "  var first: Str = \"repeat\";\n",
        "  var second: Str = first;\n",
        "  second = \"repeat\";\n",
        "  consume(identity(first));\n",
        "  consume(\"repeat\");\n",
        "  return 0;\n",
        "}\n",
    ));

    assert_eq!(output.matches(".type .Lska_literal_").count(), 1);
    assert!(output.contains("mov r11, 0xfffffffffffffffe"));
    let output = format!(
        "{output}\n{}",
        concat!(
            ".text\n",
            ".globl ska_rt_alloc\n",
            ".type ska_rt_alloc, @function\n",
            "ska_rt_alloc:\n",
            "    ud2\n",
            ".size ska_rt_alloc, .-ska_rt_alloc\n",
            ".globl ska_rt_free\n",
            ".type ska_rt_free, @function\n",
            "ska_rt_free:\n",
            "    ud2\n",
            ".size ska_rt_free, .-ska_rt_free\n",
        )
    );
    assert_eq!(run_native_assembly(&output).code(), Some(0));
}

#[test]
fn backend_rejects_unverified_static_provenance() {
    let mut program = string_program(concat!(
        "from std::str import Str;\n",
        "fn main() -> i64 { var value: Str = \"invalid\"; return 0; }\n",
    ));
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let static_owner = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::SharedStatic(static_owner) => Some(static_owner),
            _ => None,
        })
        .unwrap();
    static_owner.origin = MirStaticAllocationOrigin::Unspecified;

    let error = emit_assembly(Target::X86_64SysV, &program).unwrap_err();
    assert!(error
        .message()
        .contains("static shared owner must have immortal provenance"));
}

#[test]
fn string_emission_uses_the_conversion_free_runtime_abi() {
    let header = include_str!("../../../../../../runtime/include/skald_runtime.h");
    assert!(header.contains("#define SKALD_RUNTIME_ABI_VERSION UINT64_C(9)"));
    assert!(header.contains("#define SKALD_RUNTIME_ABI_MARKER ska_rt_abi_v9"));
    assert!(!header.contains("string"));
    assert!(!header.contains("println"));
}

#[test]
fn panic_extracts_the_exact_descriptor_slice_and_uses_the_common_reporter() {
    let program = panic_program(concat!(
        "from std::error import panic;\n",
        "fn main() -> i64 { panic(\"failure\"); }\n",
    ));
    verify_mir(&program).unwrap();
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    let main = function_assembly(&output, ".Lska.fn.app.main.f0");
    // One call is the explicit panic and one is the generic retain-overflow
    // edge used while copying its string argument.
    assert_eq!(main.matches("call ska_rt_panic").count(), 2);
    assert!(main.contains("lea rdi, [rdi + 24]"));
    assert!(main.contains("add rdi, rax"));
    assert!(main.contains("mov rsi, qword ptr [rdx + 16]"));
    assert!(!main.contains("call ska_rt_free"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn private_initializer_dynamic_strings_reclaim_their_last_backing_owner() {
    let program = canonical_string_program(concat!(
        "from std::str import Str;\n",
        "extern fn report() -> i64;\n",
        "fn main() -> i64 {\n",
        "  if (true) {\n",
        "    var bytes: u8[] = u8[](3u);\n",
        "    bytes[0] = 1u8;\n",
        "    var value: Str = Str.from_bytes(bytes);\n",
        "    var slice: Str = value.slice(0, 1);\n",
        "    var observed: u8 = slice.byte(-1);\n",
        "  }\n",
        "  return report();\n",
        "}\n",
    ));
    let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    output.push_str(concat!(
        "\n.bss\n",
        ".p2align 3\n",
        ".Lstring_allocations: .quad 0\n",
        ".Lstring_frees: .quad 0\n",
        "\n.text\n",
        ".globl ska_rt_alloc\n",
        ".type ska_rt_alloc, @function\n",
        "ska_rt_alloc:\n",
        "    inc qword ptr [rip + .Lstring_allocations]\n",
        "    jmp malloc@PLT\n",
        ".size ska_rt_alloc, .-ska_rt_alloc\n",
        ".globl ska_rt_free\n",
        ".type ska_rt_free, @function\n",
        "ska_rt_free:\n",
        "    inc qword ptr [rip + .Lstring_frees]\n",
        "    jmp free@PLT\n",
        ".size ska_rt_free, .-ska_rt_free\n",
        ".globl report\n",
        ".type report, @function\n",
        "report:\n",
        "    mov rax, 1\n",
        "    mov rcx, qword ptr [rip + .Lstring_allocations]\n",
        // The three conversion tables and Str's empty backing remain live
        // until program shutdown; the two dynamic string allocations have
        // already been reclaimed here.
        "    cmp rcx, 6\n",
        "    jne .Lstring_report_done\n",
        "    cmp qword ptr [rip + .Lstring_frees], 2\n",
        "    jne .Lstring_report_done\n",
        "    xor rax, rax\n",
        ".Lstring_report_done:\n",
        "    ret\n",
        ".size report, .-report\n",
    ));

    assert_eq!(run_native_assembly(&output).code(), Some(0));
}

#[test]
fn default_strings_share_one_static_empty_backing() {
    let program = canonical_string_program(concat!(
        "from std::str import Str;\n",
        "extern fn report() -> i64;\n",
        "fn main() -> i64 {\n",
        "  var index: i64 = 0;\n",
        "  while (index < 32) {\n",
        "    var value: Str = Str();\n",
        "    if (value.len() != 0u) { return 2; }\n",
        "    index = index + 1;\n",
        "  }\n",
        "  return report();\n",
        "}\n",
    ));
    let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    output.push_str(concat!(
        "\n.bss\n",
        ".p2align 3\n",
        ".Ldefault_string_allocations: .quad 0\n",
        ".Ldefault_string_frees: .quad 0\n",
        "\n.text\n",
        ".globl ska_rt_alloc\n",
        ".type ska_rt_alloc, @function\n",
        "ska_rt_alloc:\n",
        "    inc qword ptr [rip + .Ldefault_string_allocations]\n",
        "    jmp malloc@PLT\n",
        ".size ska_rt_alloc, .-ska_rt_alloc\n",
        ".globl ska_rt_free\n",
        ".type ska_rt_free, @function\n",
        "ska_rt_free:\n",
        "    inc qword ptr [rip + .Ldefault_string_frees]\n",
        "    jmp free@PLT\n",
        ".size ska_rt_free, .-ska_rt_free\n",
        ".globl report\n",
        ".type report, @function\n",
        "report:\n",
        "    mov rax, 1\n",
        // The three conversion tables and one empty string backing are the
        // only allocations before ordinary static shutdown.
        "    cmp qword ptr [rip + .Ldefault_string_allocations], 4\n",
        "    jne .Ldefault_string_report_done\n",
        "    cmp qword ptr [rip + .Ldefault_string_frees], 0\n",
        "    jne .Ldefault_string_report_done\n",
        "    xor rax, rax\n",
        ".Ldefault_string_report_done:\n",
        "    ret\n",
        ".size report, .-report\n",
    ));

    assert_eq!(run_native_assembly(&output).code(), Some(0));
}
