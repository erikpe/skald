use super::super::source::{
    single_source_full_phase_dump, single_source_type_error_dump, StandardLibraryInput,
};

pub(crate) fn primitive_cast_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/primitives/primitive_cast_matrix.ska"
        )),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

pub(crate) fn primitive_cast_diagnostic_dump() -> String {
    single_source_type_error_dump(
        "primitive-cast-diagnostics.ska",
        concat!(
            "fn invalid(values: i64[]) -> u64 { return (u64) values; }\n",
            "fn implicit(value: f64) -> i64 { return value; }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        StandardLibraryInput::None,
    )
}
