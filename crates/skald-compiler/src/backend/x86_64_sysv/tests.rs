use std::{
    io::Write,
    process::{Command, Stdio},
};

use crate::{
    backend::{emit_assembly, Target},
    hir::HirProgram,
    lexer::lex,
    mir::{lower_hir, verify_mir, BlockId, MirBasicBlock, MirProgram},
    resolve::resolve,
    source::SourceDatabase,
    syntax::parse,
    typeck::type_check,
};

fn lower_text(text: &str) -> MirProgram {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("backend-test.ska", text);
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let resolved = resolve(&parsed.ast);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir: HirProgram = checked.hir.unwrap();
    lower_hir(&hir)
}

fn assembly(text: &str) -> String {
    emit_assembly(Target::X86_64SysV, &lower_text(text)).unwrap()
}

#[test]
fn emits_a_deterministic_minimal_function() {
    let source = "fn main() -> i64 { return 42; }";
    let expected = concat!(
        ".text\n",
        ".p2align 4\n",
        ".type .Lska_fn_0, @function\n",
        ".Lska_fn_0:\n",
        "    pushq %rbp\n",
        "    movq %rsp, %rbp\n",
        "    subq $16, %rsp\n",
        "    movabsq $42, %rax\n",
        "    movq %rax, -8(%rbp)\n",
        "    movq -8(%rbp), %rax\n",
        "    leave\n",
        "    ret\n",
        ".size .Lska_fn_0, .-.Lska_fn_0\n",
        "\n",
        ".p2align 4\n",
        ".globl main\n",
        ".type main, @function\n",
        "main:\n",
        "    pushq %rbp\n",
        "    movq %rsp, %rbp\n",
        "    call .Lska_fn_0\n",
        "    leave\n",
        "    ret\n",
        ".size main, .-main\n",
        "\n",
        ".section .note.GNU-stack,\"\",@progbits\n",
    );

    assert_eq!(assembly(source), expected);
    assert_eq!(assembly(source), assembly(source));
}

#[test]
fn selects_every_first_slice_arithmetic_operation_and_storage_copy() {
    let output = assembly(concat!(
        "fn helper(a: i64) -> i64 { return -a; }\n",
        "fn main() -> i64 { ",
        "var x: i64 = 9; return helper(x * 3 - 4 + 2); }",
    ));

    assert!(output.contains("negq %rax"));
    assert!(output.contains("imulq %rcx, %rax"));
    assert!(output.contains("subq %rcx, %rax"));
    assert!(output.contains("addq %rcx, %rax"));
    assert!(output.contains("call .Lska_fn_0"));
    assert!(output.contains("movq %rax, -8(%rbp)"));
}

#[test]
fn unit_calls_and_returns_do_not_move_a_fictitious_result() {
    let output = assembly(concat!(
        "fn notify(value: i64) -> unit {}\n",
        "fn main() -> i64 { notify(42); return 7; }\n",
    ));

    assert!(output.contains("call .Lska_fn_0\n    movabsq $7, %rax"));
    assert!(!output.contains("call .Lska_fn_0\n    movq %rax,"));
    assert!(output.contains(
        ".Lska_fn_0:\n    pushq %rbp\n    movq %rsp, %rbp\n    subq $16, %rsp\n    movq %rdi, -8(%rbp)\n    leave\n    ret"
    ));
}

#[test]
fn lowers_register_and_stack_arguments_at_the_abi_boundary() {
    let output = assembly(concat!(
        "fn seventh(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, ",
        "g: i64) -> i64 { return g; }\n",
        "fn main() -> i64 { return seventh(1, 2, 3, 4, 5, 6, 7); }",
    ));

    for spill in [
        "movq %rdi, -8(%rbp)",
        "movq %rsi, -16(%rbp)",
        "movq %rdx, -24(%rbp)",
        "movq %rcx, -32(%rbp)",
        "movq %r8, -40(%rbp)",
        "movq %r9, -48(%rbp)",
    ] {
        assert!(output.contains(spill), "missing `{spill}` in:\n{output}");
    }
    assert!(output.contains("movq 16(%rbp), %rax"));
    assert!(output.contains("subq $16, %rsp"));
    assert!(output.contains("movq %rax, (%rsp)"));
    assert!(output.contains("call .Lska_fn_0\n    addq $16, %rsp"));
}

#[test]
fn uses_no_unpreserved_callee_saved_scratch_registers() {
    let output = assembly("fn main() -> i64 { return (2 + 3) * 4; }");

    for register in ["%rbx", "%r12", "%r13", "%r14", "%r15"] {
        assert!(!output.contains(register));
    }
    assert!(output.contains("pushq %rbp"));
    assert!(output.contains("leave"));
}

#[test]
fn emits_a_c_compatible_entry_boundary() {
    let output = assembly("fn helper() -> i64 { return 1; } fn main() -> i64 { return 2; }");

    assert!(output.contains(".globl main\n.type main, @function\nmain:"));
    assert!(output.contains("main:\n    pushq %rbp\n    movq %rsp, %rbp\n    call .Lska_fn_1"));
    assert!(!output.contains(".globl .Lska_fn_"));
}

#[test]
fn external_calls_use_the_declared_symbol_without_emitting_a_body() {
    let mir = lower_text(concat!(
        // Deliberately resembles an old internal symbol. The leading dot on
        // target-private symbols keeps the two namespaces disjoint.
        "extern fn ska_fn_1(value: i64) -> i64;\n",
        "fn main() -> i64 { return ska_fn_1(9); }\n",
    ));

    let output = emit_assembly(Target::X86_64SysV, &mir).unwrap();

    assert!(output.contains("call ska_fn_1"));
    assert!(!output.contains("\nska_fn_1:\n"));
    assert!(output.contains(".Lska_fn_1:"));
}

#[test]
fn lowers_boolean_values_through_internal_and_external_abi_boundaries() {
    let output = assembly(concat!(
        "extern fn external_flag(value: bool) -> bool;\n",
        "fn identity(value: bool) -> bool { return value; }\n",
        "fn main() -> i64 { var flag: bool = identity(true); var external: bool = external_flag(flag); return 0; }\n",
    ));

    assert!(output.contains("movabsq $1, %rax"));
    assert!(output.contains("call .Lska_fn_1"));
    assert!(output.contains("call external_flag\n    movzbq %al, %rax"));
    assert!(output.contains("movq %rdi, -8(%rbp)"));
}

#[test]
fn rejects_verified_mir_outside_the_initial_target_shape() {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let original = &function.body.blocks[0];
    function.body.blocks.push(MirBasicBlock {
        id: BlockId::new(function.function, 1),
        instructions: Vec::new(),
        terminator: original.terminator.clone(),
        span: original.span,
    });
    assert!(verify_mir(&mir).is_ok());

    let error = emit_assembly(Target::X86_64SysV, &mir).unwrap_err();
    assert_eq!(error.function(), Some(mir.entry_function));
    assert_eq!(
        error.message(),
        "the initial backend supports exactly one basic block, found 2"
    );
}

#[test]
fn generated_text_is_accepted_by_the_system_assembler() {
    let output = assembly(concat!(
        "fn calculate(a: i64, b: i64) -> i64 { return -a * b + 3; }\n",
        "fn main() -> i64 { return calculate(6, 7); }",
    ));
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
