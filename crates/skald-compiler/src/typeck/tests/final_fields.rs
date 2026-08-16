use super::*;
use crate::{
    hir::{HirCopyCapability, HirFieldWriteAuthorization, HirStatement, HirSynthesizedFieldCopy},
    identity::ClassId,
    mir::{dump_mir, verify_mir},
    test_support::lower_hir_to_final_mir,
};

fn assert_final_replacement_rejected(source: &str) {
    let output = check_text(source);
    assert!(output.hir.is_none());
    let replacements = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == FINAL_FIELD_REPLACEMENT)
        .collect::<Vec<_>>();
    assert_eq!(replacements.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(
        replacements[0].message,
        "final field `value` cannot be replaced"
    );
    assert_eq!(replacements[0].labels.len(), 2);
}

#[test]
fn carries_final_metadata_through_typed_hir_and_mir() {
    let output = check_text(concat!(
        "class Values {\n",
        "  final value: i64;\n",
        "  final static version: u64 = 1u;\n",
        "  init(value: i64) { self.value = value; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let values = hir.class(ClassId::new(0)).unwrap();
    assert!(values.fields[0].final_span.is_some());
    assert!(values.static_fields[0].final_span.is_some());
    let hir_dump = dump_hir(&hir);
    assert!(
        hir_dump.contains("Field c0:field0 final \"value\""),
        "{hir_dump}"
    );
    assert!(
        hir_dump.contains("StaticField c0:static0 final \"version\""),
        "{hir_dump}"
    );

    let mir = lower_hir_to_final_mir(&hir);
    verify_mir(&mir).unwrap();
    let values = mir.class(ClassId::new(0)).unwrap();
    assert!(values.fields[0].final_span.is_some());
    assert!(values.static_fields[0].final_span.is_some());
    assert_eq!(dump_mir(&mir).matches("Final @").count(), 2);
}

#[test]
fn rejects_direct_final_replacement_from_every_complete_object_body() {
    let cases = [
        concat!(
            "class Value { final value: i64; init() { self.value = 0; } ",
            "mut fn replace() -> unit { self.value = 1; } }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        concat!(
            "class Value { private final value: i64; init() { self.value = 0; } ",
            "fn replace() -> unit { self.value = 1; } }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        concat!(
            "class Value { final value: i64; init() { self.value = 0; } ",
            "static fn replace(mut ref target: Value) -> unit { target.value = 1; } }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        concat!(
            "class Value { final value: i64; init() { self.value = 0; } }\n",
            "fn replace(mut ref target: Value) -> unit { target.value = 1; }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        concat!(
            "class Value { final value: i64; init() { self.value = 0; } ",
            "destroy { self.value = 1; } }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        concat!(
            "class Base { final value: i64; init() { self.value = 0; } }\n",
            "class Derived extends Base { init() { super(); } ",
            "mut fn replace() -> unit { self.value = 1; } }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        concat!(
            "class Value { final value: i64; init() { self.value = 0; } }\n",
            "class Other { init() {} static fn replace(mut ref target: Value) -> unit { ",
            "target.value = 1; } }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
    ];

    for source in cases {
        assert_final_replacement_rejected(source);
    }
}

#[test]
fn private_visibility_is_diagnosed_before_finality_outside_the_declaring_class() {
    let resolved = crate::test_support::resolve_source(concat!(
        "class Value { private final value: i64; init() { self.value = 0; } }\n",
        "fn replace(mut ref target: Value) -> unit { target.value = 1; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(resolved
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == crate::resolve::PRIVATE_MEMBER_ACCESS));
    assert!(!resolved
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == FINAL_FIELD_REPLACEMENT));
}

#[test]
fn rejects_direct_inherited_final_initialization_in_derived_constructors() {
    for source in [
        concat!(
            "class Base { final value: i64; init() { self.value = 0; } }\n",
            "class Derived extends Base { init() { super(); self.value = 1; } }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        concat!(
            "class Base { final value: i64; init() { self.value = 0; } }\n",
            "class Derived extends Base { init() { super(); } ",
            "copy(ref other: Derived) { self.value = other.value; } }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == INVALID_INITIALIZER_BODY));
    }
}

#[test]
fn ordinary_and_copy_construction_initialize_direct_final_fields() {
    let output = check_text(concat!(
        "class Value {\n",
        "  final value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref other: Value) { self.value = other.value; }\n",
        "  fn get() -> i64 { return self.value; }\n",
        "}\n",
        "fn read(ref value: Value) -> i64 { return value.value; }\n",
        "fn main() -> i64 { var first: Value = Value(7); var second: Value = first; ",
        "return second.get() + read(first); }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let class = hir.class(ClassId::new(0)).unwrap();
    assert!(matches!(class.copy_constructor, HirCopyCapability::User(_)));
    let mir = lower_hir_to_final_mir(&hir);
    verify_mir(&mir).unwrap();
}

#[test]
fn final_construction_reuses_exact_once_and_straight_line_initializer_rules() {
    let cases = [
        concat!(
            "class Value { final value: i64; init() {} }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        concat!(
            "class Value { final value: i64; init() { self.value = 1; self.value = 2; } }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        concat!(
            "class Value { final value: i64; init() { if (true) { self.value = 1; } } }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
    ];

    for source in cases {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == FIELD_INITIALIZATION || diagnostic.code == INVALID_INITIALIZER_BODY
        }));
    }
}

#[test]
fn synthesized_copy_construction_preserves_every_final_storage_family() {
    let output = check_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn identity(value: i64) -> i64 { return value; }\n",
        "class Values {\n",
        "  final primitive: i64; final object: Item; final maybe: i64?;\n",
        "  final owner: shared Item; final values: i64[]; final callback: fn(i64) -> i64;\n",
        "  init() { self.primitive = 1; self.object = Item(2); self.maybe = 3;\n",
        "    self.owner = new Item(4); self.values = i64[]{5}; self.callback = identity; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let values = hir.class(ClassId::new(1)).unwrap();
    let HirCopyCapability::Synthesized(copy) = &values.copy_constructor else {
        panic!("expected synthesized copy construction");
    };
    assert!(matches!(
        values.copy_assignment,
        HirCopyCapability::Synthesized(_)
    ));
    assert_eq!(copy.fields.len(), 6);
    assert!(matches!(
        copy.fields[0],
        HirSynthesizedFieldCopy::Scalar { .. }
    ));
    assert!(matches!(
        copy.fields[1],
        HirSynthesizedFieldCopy::Class { .. }
    ));
    assert!(matches!(
        copy.fields[2],
        HirSynthesizedFieldCopy::OptionalPrimitive { .. }
    ));
    assert!(matches!(
        copy.fields[3],
        HirSynthesizedFieldCopy::Shared { .. }
    ));
    assert!(matches!(
        copy.fields[4],
        HirSynthesizedFieldCopy::Array { .. }
    ));
    assert!(matches!(
        copy.fields[5],
        HirSynthesizedFieldCopy::Scalar { .. }
    ));
    verify_mir(&lower_hir_to_final_mir(&hir)).unwrap();
}

#[test]
fn preserves_reads_aliases_produced_reads_and_shallow_nested_mutation() {
    let output = check_text(concat!(
        "class Child { value: i64; init(value: i64) { self.value = value; } ",
        "mut fn increment() -> unit { self.value = self.value + 1; } }\n",
        "fn inspect(ref child: Child) -> i64 { return child.value; }\n",
        "class Holder { final child: Child; final values: i64[];\n",
        "  init() { self.child = Child(1); self.values = i64[]{2}; }\n",
        "  mut fn update() -> i64 { self.child.value = 3; self.child.increment(); ",
        "self.values[0] = 4; return inspect(self.child) + self.values[0]; }\n",
        "}\n",
        "fn produce() -> Holder { return Holder(); }\n",
        "fn main() -> i64 { var holder: Holder = Holder(); return holder.update() + produce().child.value; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    verify_mir(&lower_hir_to_final_mir(&output.hir.unwrap())).unwrap();
}

#[test]
fn exact_copy_assignment_is_deferred_but_nested_and_inherited_writes_are_rejected() {
    let output = check_text(concat!(
        "class Value { final value: i64; init(value: i64) { self.value = value; }\n",
        "  assign(ref other: Value) { self.value = other.value; } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let assignment = hir
        .class(ClassId::new(0))
        .unwrap()
        .copy_assignment_declaration
        .as_ref()
        .unwrap();
    let definition = hir.member_definition(assignment.id.into()).unwrap();
    let HirStatement::FieldAssignment(write) = &definition.body.statements[0] else {
        panic!("expected final field assignment");
    };
    assert_eq!(
        write.place.write_authorization,
        Some(HirFieldWriteAuthorization::DeferredFinalAssignment)
    );
    assert!(dump_hir(&hir).contains("WriteAuthorization DeferredFinalAssignment c0:field0"));

    for source in [
        concat!(
            "class Inner { final value: i64; init() { self.value = 0; } }\n",
            "class Outer { inner: Inner; init() { self.inner = Inner(); } ",
            "assign(ref other: Outer) { self.inner.value = other.inner.value; } }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        concat!(
            "class Base { final value: i64; init() { self.value = 0; } }\n",
            "class Derived extends Base { init() { super(); } ",
            "assign(ref other: Derived) { self.value = other.value; } }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
    ] {
        assert_final_replacement_rejected(source);
    }
}
