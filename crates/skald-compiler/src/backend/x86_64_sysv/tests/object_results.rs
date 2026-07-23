use super::*;

#[test]
fn lowers_function_and_method_object_results_through_hidden_return_storage() {
    let output = assembly(concat!(
        "class Value {\n",
        "  tag: u8;\n",
        "  field: i64;\n",
        "  init(field: i64, tag: u8) { self.tag = tag; self.field = field; }\n",
        "  init(ref other: Value) {\n",
        "    self.tag = other.tag;\n",
        "    self.field = other.field + 100;\n",
        "  }\n",
        "  fn duplicate(a: i64, b: i64, c: i64, d: i64, e: i64) -> Value {\n",
        "    return self;\n",
        "  }\n",
        "}\n",
        "fn produce(ref source: Value, a: i64, b: i64, c: i64, d: i64, e: i64, f: i64) -> Value {\n",
        "  var local: Value = source;\n",
        "  return local;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var source: Value = Value(10, 7u8);\n",
        "  var first: Value = produce(source, 1, 2, 3, 4, 5, 6);\n",
        "  var second: Value = first.duplicate(1, 2, 3, 4, 5);\n",
        "  return second.field;\n",
        "}\n",
    ));

    // 10 -> 110 in produce's local -> 210 in its caller result -> 310 in
    // the method caller result. The low byte is the process exit status.
    assert_eq!(run_native_assembly(&output).code(), Some(54));
    assert!(!output.contains("memcpy"));
}

#[test]
fn return_destination_precedes_receiver_and_explicit_arguments() {
    let layout = super::super::abi::CallLayout::classify_internal_call(
        &[
            MirParameter::value(MirType::I64),
            MirParameter::value(MirType::I64),
        ],
        true,
        true,
    )
    .unwrap();

    assert_eq!(
        layout.return_destination(),
        Some(super::super::abi::ArgumentLocation::IntegerRegister(
            super::super::machine::Register::Rdi,
        ))
    );
    assert_eq!(
        layout.receiver(),
        Some(super::super::abi::ArgumentLocation::IntegerRegister(
            super::super::machine::Register::Rsi,
        ))
    );
    assert_eq!(
        layout.locations(),
        [
            super::super::abi::ArgumentLocation::IntegerRegister(
                super::super::machine::Register::R8,
            ),
            super::super::abi::ArgumentLocation::IntegerRegister(
                super::super::machine::Register::R9,
            ),
        ]
    );
}
