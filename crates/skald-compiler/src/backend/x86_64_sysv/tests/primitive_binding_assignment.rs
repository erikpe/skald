use super::*;

#[test]
fn primitive_binding_reassignment_executes_for_every_scalar_storage_class() {
    let mut output = assembly(concat!(
        "extern fn validate_f64(value: f64) -> bool;\n",
        "fn next(value: i64) -> i64 { value = value + 1; value = value + 1; return value; }\n",
        "fn main() -> i64 {\n",
        "  var signed: i64 = 0;\n",
        "  var unsigned: u64 = 0u;\n",
        "  var byte: u8 = 0u8;\n",
        "  var float: f64 = 0.0;\n",
        "  var flag: bool = false;\n",
        "  signed = next(6);\n",
        "  unsigned = 18446744073709551615u;\n",
        "  (byte) = 255u8;\n",
        "  float = 2.5;\n",
        "  flag = validate_f64(float);\n",
        "  var branch: i64 = 1;\n",
        "  if (flag) {\n",
        "    branch = 9;\n",
        "    { var branch: i64 = 2; branch = 3; }\n",
        "  }\n",
        "  if (signed == 8) {\n",
        "    if (unsigned == 18446744073709551615u) {\n",
        "      if (byte == 255u8) {\n",
        "        if (flag) { return branch + 82; }\n",
        "      }\n",
        "    }\n",
        "  }\n",
        "  return 1;\n",
        "}\n",
    ));
    output.push_str(concat!(
        "\n.text\n",
        ".globl validate_f64\n",
        ".type validate_f64, @function\n",
        "validate_f64:\n",
        "    movq rax, xmm0\n",
        "    mov rcx, 0x4004000000000000\n",
        "    cmp rax, rcx\n",
        "    sete al\n",
        "    movzx rax, al\n",
        "    ret\n",
        ".size validate_f64, .-validate_f64\n",
    ));

    assert_eq!(run_native_assembly(&output).code(), Some(91));
    assert!(output.contains("movzx rax, al"));
}
