use super::super::source::{
    single_source_full_phase_dump, single_source_type_error_dump, StandardLibraryInput,
};

pub(crate) fn eager_boolean_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/operators/eager_boolean_operators.ska"
        )),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

pub(crate) fn eager_boolean_diagnostic_dump() -> String {
    single_source_type_error_dump(
        "eager-boolean-diagnostics.ska",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/operators/eager_boolean_operator_types.ska"
        )),
        StandardLibraryInput::None,
    )
}

pub(crate) fn short_circuit_source_phase_dump() -> String {
    single_source_full_phase_dump(
        concat!(
            "fn selected(a: bool, b: bool, c: bool) -> bool { return (a || b) && !c; }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        StandardLibraryInput::None,
    )
}
