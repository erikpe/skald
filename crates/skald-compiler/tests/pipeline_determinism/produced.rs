use super::source::{single_source_full_phase_dump, StandardLibraryInput};

pub(crate) fn produced_alias_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/aliases/produced_alias_arguments.ska"
        )),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

pub(crate) fn produced_receiver_phase_dump() -> String {
    single_source_full_phase_dump(
        concat!(
            "class Item { value: i64; init(value: i64) { self.value = value; } ",
            "fn next(amount: i64) -> Item { return Item(self.value + amount); } ",
            "fn read() -> i64 { return self.value; } }\n",
            "fn produce(value: i64) -> Item { return Item(value); }\n",
            "fn main() -> i64 { return produce(40).next(2).read(); }\n",
        ),
        StandardLibraryInput::None,
    )
}

pub(crate) fn produced_field_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/produced_fields/primitive_and_inline_consumers.ska"
        )),
        StandardLibraryInput::None,
    )
}
