use super::source::{
    single_source_full_phase_dump, single_source_type_error_dump, StandardLibraryInput,
};

pub(crate) fn object_phase_dump() -> String {
    let text = concat!(
        "class Box { value: i64; init(value: i64) { self.value = value; } ",
        "copy(ref other: Box) { self.value = other.value; } ",
        "assign(ref other: Box) { self.value = other.value; } ",
        "mut fn set(value: i64) -> unit { self.value = value; } ",
        "fn get() -> i64 { return self.value; } destroy {} }\n",
        "class Snapshot { box: Box; init(ref source: Box) { self.box = Box(read(source)); } ",
        "destroy {} }\n",
        "fn read(ref value: Box) -> i64 { return value.get(); }\n",
        "fn write(mut ref value: Box, amount: i64) -> unit { value.set(amount); }\n",
        "fn forward(mut ref value: Box) -> unit { write(value, read(value) + 1); }\n",
        "fn produce(value: i64) -> Box { return Box(value); }\n",
        "fn choose(ref source: Box, first: bool) -> Box { ",
        "if (first) { return source; } else { return (Box(source.get())); } }\n",
        "fn consume(value: Box, ref alias: Box) -> i64 { ",
        "value = produce(alias.get()); return value.get(); }\n",
        "fn main() -> i64 { var value: Box = Box(1); forward(value); ",
        "var grouped: Box = (Box(2)); grouped = produce(read(value)); ",
        "var copied: Box = value; var result: Box = choose(copied, false); ",
        "var snapshot: Snapshot = Snapshot(result); ",
        "return consume(produce(snapshot.box.get()), grouped); }\n",
    );
    single_source_full_phase_dump(text, StandardLibraryInput::None)
}

pub(crate) fn polymorphism_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/polymorphism/polymorphism.ska"
        )),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

pub(crate) fn private_initializer_phase_dump() -> String {
    single_source_full_phase_dump(
        concat!(
            "class Secret { value: i64; init(value: i64) { self.value = value; } ",
            "private init(flag: bool) { self.value = 42; } ",
            "static fn make(flag: bool) -> Secret { return Secret(flag); } ",
            "fn reveal() -> i64 { return self.value; } }\n",
            "fn main() -> i64 { var public: Secret = Secret(1); ",
            "var private: Secret = Secret.make(true); return public.reveal() + private.reveal(); }\n",
        ),
        StandardLibraryInput::None,
    )
}

pub(crate) fn private_initializer_diagnostic_dump() -> String {
    let text = concat!(
        "interface Named {}\n",
        "class Key implements Named { init() {} }\n",
        "class Choice { init(ref value: Obj) {} private init(ref value: Named) {} }\n",
        "fn main() -> i64 { var key: Key = Key(); ",
        "var choice: Choice = Choice(key); return 0; }\n",
    );
    single_source_type_error_dump(
        "private-initializer-diagnostic.ska",
        text,
        StandardLibraryInput::None,
    )
}

pub(crate) fn private_cell_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/objects/private_cell_dispatch_composition.ska"
        )),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

pub(crate) fn private_cell_diagnostic_dump() -> String {
    single_source_type_error_dump(
        "private-cell-diagnostics.ska",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/objects/private_cell_exclusions.ska"
        )),
        StandardLibraryInput::None,
    )
}

pub(crate) fn final_field_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/objects/final_field_composition.ska"
        )),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

pub(crate) fn final_field_diagnostic_dump() -> String {
    single_source_type_error_dump(
        "final-field-diagnostics.ska",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/objects/final_field_alias_rebinding.ska"
        )),
        StandardLibraryInput::None,
    )
}
