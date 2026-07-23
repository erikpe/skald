use super::*;

#[test]
fn selects_every_supported_arithmetic_operation_and_storage_copy() {
    let output = assembly(concat!(
        "fn helper(a: i64) -> i64 { return -a; }\n",
        "fn main() -> i64 { ",
        "var x: i64 = 9; return helper(x * 3 - 4 + 2); }",
    ));

    assert!(output.contains("neg rax"));
    assert!(output.contains("imul rax, rcx"));
    assert!(output.contains("sub rax, rcx"));
    assert!(output.contains("add rax, rcx"));
    assert!(output.contains("call .Lska_fn_0"));
    assert!(output.contains("mov qword ptr [rbp - 8], rax"));
}
