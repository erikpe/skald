use super::*;

#[test]
fn lowers_owned_parameters_for_initializers_methods_and_functions() {
    let output = assembly(concat!(
        "class Value {\n",
        "  field: i64;\n",
        "  init(field: i64) { self.field = field; }\n",
        "  copy(ref other: Value) { self.field = other.field + 5; }\n",
        "}\n",
        "class Holder {\n",
        "  marker: i64;\n",
        "  init(value: Value) { self.marker = value.field; }\n",
        "  fn combine(value: Value) -> i64 { return self.marker + value.field; }\n",
        "}\n",
        "fn forward(value: Value) -> i64 {\n",
        "  var holder: Holder = Holder(value);\n",
        "  return holder.combine(value);\n",
        "}\n",
        "fn main() -> i64 { var source: Value = Value(2); return forward(source); }\n",
    ));

    // Copies occur at each of the function, initializer, and method call
    // boundaries: 2 -> 7 -> 12/12, producing 24.
    assert_eq!(run_native_assembly(&output).code(), Some(24));
    assert!(!output.contains("memcpy"));
}

#[test]
fn class_value_parameters_use_integer_address_locations() {
    let class = MirParameter::value(MirType::Class(ClassId::new(0)));
    let parameters = [
        MirParameter::value(MirType::I64),
        MirParameter::value(MirType::F64),
        class,
        MirParameter::read_only_alias(MirType::Class(ClassId::new(0))),
    ];
    let layout = super::super::abi::CallLayout::classify(&parameters).unwrap();

    assert_eq!(
        layout.locations(),
        [
            super::super::abi::ArgumentLocation::IntegerRegister(
                super::super::machine::Register::Rdi,
            ),
            super::super::abi::ArgumentLocation::SseRegister(
                super::super::machine::XmmRegister::Xmm0,
            ),
            super::super::abi::ArgumentLocation::IntegerRegister(
                super::super::machine::Register::Rsi,
            ),
            super::super::abi::ArgumentLocation::IntegerRegister(
                super::super::machine::Register::Rdx,
            ),
        ]
    );
}
