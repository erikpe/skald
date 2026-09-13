use super::super::source::{
    single_source_full_phase_dump, single_source_type_error_dump, StandardLibraryInput,
};

pub(crate) fn integer_operation_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/primitives/integer_string_range_guards.ska"
        )),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

pub(crate) fn integer_bitwise_and_shift_phase_dump() -> String {
    single_source_full_phase_dump(
        concat!(
            "class Bits { value: u8; count: u64; ",
            "init(value: u8, count: u64) { self.value = value; self.count = count; } }\n",
            "class Trace { value: u64; init(value: u64) { self.value = value; } ",
            "fn read() -> u64 { return self.value; } destroy {} }\n",
            "fn make(value: u64) -> shared Trace { return new Trace(value); }\n",
            "fn mix(ref bits: Bits, optional: u8?, values: u8[]) -> bool { ",
            "return (((~bits.value + 0x01u8 << bits.count) >> 1u) & values[0] ",
            "^ optional! | 0x01u8) == 0x07u8 && true; }\n",
            "fn cleanup() -> u64 { return make(0x10u)->read() >> make(2u)->read(); }\n",
            "fn main() -> i64 { var bits: Bits = Bits(0x03u8, 2u); ",
            "var optional: u8? = 0x04u8; var values: u8[] = u8[](1u); values[0] = 0x07u8; ",
            "if (mix(bits, optional, values) || cleanup() == 0x04u) { return 0; } return 1; }\n",
        ),
        StandardLibraryInput::None,
    )
}

pub(crate) fn integer_bitwise_and_shift_diagnostic_dump() -> String {
    single_source_type_error_dump(
        "integer-bitwise-shift-diagnostics.ska",
        concat!(
            "class Item { init() {} }\n",
            "fn invalid(flag: bool, count: i64, owner: shared Item) -> i64 {\n",
            "  var complement: i64 = ~flag;\n",
            "  var bitwise: i64 = 1 | flag;\n",
            "  var shifted: i64 = 1 << count;\n",
            "  var owner_count: i64 = 1 >> owner;\n",
            "  return complement + bitwise + shifted + owner_count;\n",
            "}\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        StandardLibraryInput::None,
    )
}

pub(crate) fn integer_division_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/operators/integer_division_operators.ska"
        )),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

pub(crate) fn integer_division_diagnostic_dump() -> String {
    single_source_type_error_dump(
        "integer-division-diagnostics.ska",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/operators/integer_division_operator_types.ska"
        )),
        StandardLibraryInput::None,
    )
}

pub(crate) fn floating_division_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/operators/floating_division.ska"
        )),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

pub(crate) fn floating_division_diagnostic_dump() -> String {
    single_source_type_error_dump(
        "floating-division-diagnostics.ska",
        concat!(
            "class Item { init() {} }\n",
            "fn invalid(left: f64, integer: i64, flag: bool, owner: shared Item) -> f64 {\n",
            "  var mixed: f64 = left / integer;\n",
            "  var boolean: f64 = left / flag;\n",
            "  return left / owner;\n",
            "}\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        StandardLibraryInput::None,
    )
}

pub(crate) fn floating_comparison_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/operators/floating_comparisons.ska"
        )),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

pub(crate) fn floating_comparison_diagnostic_dump() -> String {
    single_source_type_error_dump(
        "floating-comparison-diagnostics.ska",
        concat!(
            "fn invalid(left: f64, integer: i64, flag: bool, optional: f64?) -> bool {\n",
            "  var mixed: bool = left < integer;\n",
            "  var boolean: bool = left == flag;\n",
            "  return left >= optional;\n",
            "}\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        StandardLibraryInput::None,
    )
}

pub(crate) fn primitive_operator_profile_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/operators/primitive_operator_profile.ska"
        )),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}
