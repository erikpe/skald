use super::source::{
    single_source_full_phase_dump, single_source_planned_lifecycle_dump, StandardLibraryInput,
};

pub(crate) fn shared_ownership_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/shared_ownership/shared_copy_allocation.ska"
        )),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

pub(crate) fn optional_phase_dump() -> String {
    format!(
        "BASELINE\n{}COMPOSITIONAL\n{}SHARED OPTIONAL BOXES\n{}",
        single_source_full_phase_dump(
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/golden/optionals/optional_shared_profile.ska"
            )),
            StandardLibraryInput::GoldenCallsAsExternalStubs,
        ),
        single_source_full_phase_dump(
            concat!(
                "interface Forward { fn forward(values: i64[]?) -> i64[]?; }\n",
                "class Holder implements Forward {\n",
                "  static current: i64[]?; values: i64[]?;\n",
                "  init(values: i64[]?) { self.values = values; }\n",
                "  fn forward(values: i64[]?) -> i64[]? { return values; }\n",
                "}\n",
                "fn mutate(mut ref values: i64[]) -> unit { values[0] = 42; }\n",
                "fn inspect(deep: i64?????, values: i64[]?[]) -> unit {}\n",
                "fn main() -> i64 {\n",
                "  var holder: Holder = Holder(i64[]{1});\n",
                "  var nested: i64[]?[] = i64[]?[]{none, holder.values};\n",
                "  mutate(holder.values!); Holder.current = nested[1]; return holder.values![0];\n",
                "}\n",
            ),
            StandardLibraryInput::None,
        ),
        single_source_planned_lifecycle_dump(
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/golden/optionals/optional_boxes_profile.ska"
            )),
            StandardLibraryInput::None,
        )
    )
}
