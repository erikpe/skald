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
        "  final owner: shared Item; final maybe_owner: shared? Item; final nested: i64??;\n",
        "  final values: i64[]; final callback: fn(i64) -> i64;\n",
        "  init() { self.primitive = 1; self.object = Item(2); self.maybe = 3;\n",
        "    self.owner = new Item(4); self.maybe_owner = new Item(5); self.nested = none;\n",
        "    self.values = i64[]{6}; self.callback = identity; }\n",
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
    assert_eq!(copy.fields.len(), 8);
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
        HirSynthesizedFieldCopy::OptionalShared { .. }
    ));
    assert!(matches!(
        copy.fields[5],
        HirSynthesizedFieldCopy::Optional { .. }
    ));
    assert!(matches!(
        copy.fields[6],
        HirSynthesizedFieldCopy::Array { .. }
    ));
    assert!(matches!(
        copy.fields[7],
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
fn exact_copy_assignment_is_authorized_but_nested_and_inherited_writes_are_rejected() {
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
        Some(HirFieldWriteAuthorization::DeclaringClassFinalAssignment(
            assignment.id
        ))
    );
    assert!(dump_hir(&hir)
        .contains("WriteAuthorization DeclaringClassFinalAssignment c0:field0 c0:assign0"));

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

#[test]
fn user_copy_assignment_keeps_ordinary_control_flow_and_subset_freedom() {
    let output = check_text(concat!(
        "fn adjust(value: i64) -> i64 { return value + 1; }\n",
        "class Flexible { final first: i64; final second: i64;\n",
        "  init(first: i64, second: i64) { self.first = first; self.second = second; }\n",
        "  assign(ref other: Flexible) {\n",
        "    var next: i64 = adjust(other.first);\n",
        "    if (next > 0) { self.first = next; } else { self.first = other.first; }\n",
        "    var index: i64 = 0;\n",
        "    while (index < 1) { self.first = other.first + index; index = index + 1; }\n",
        "    if (other.second < 0) { return; }\n",
        "  }\n",
        "}\n",
        "class Empty { final value: i64; init(value: i64) { self.value = value; }\n",
        "  assign(ref other: Empty) { var observed: i64 = other.value; } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let dump = dump_hir(&hir);
    assert_eq!(
        dump.matches("WriteAuthorization DeclaringClassFinalAssignment c0:field0 c0:assign0")
            .count(),
        3,
        "{dump}"
    );
    assert!(!dump.contains("DeclaringClassFinalAssignment c0:field1"));
    let preliminary = crate::mir::lower_preliminary_hir(&hir);
    crate::mir::check_preliminary_mir(&preliminary).unwrap();
    verify_mir(&lower_hir_to_final_mir(&hir)).unwrap();
}

#[test]
fn rejects_final_static_root_assignment_for_every_stored_family_and_owner() {
    let output = check_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn identity(value: i64) -> i64 { return value; }\n",
        "class State {\n",
        "  final static scalar: i64 = 1;\n",
        "  final static callback: fn(i64) -> i64 = identity;\n",
        "  final static object: Item = Item(2);\n",
        "  final static maybe: i64? = 3;\n",
        "  final static maybe_object: Item? = Item(4);\n",
        "  final static owner: shared Item = new Item(5);\n",
        "  final static maybe_owner: shared? Item = new Item(6);\n",
        "  final static nested: i64?? = none;\n",
        "  final static values: i64[] = i64[]{7};\n",
        "  init() {}\n",
        "  static fn replace_all() -> unit {\n",
        "    State.scalar = 10; State.callback = identity; State.object = Item(11);\n",
        "    State.maybe = none; State.maybe_object = none;\n",
        "    State.owner = new Item(12); State.maybe_owner = none; State.nested = none;\n",
        "    State.values = i64[]{13};\n",
        "  }\n",
        "}\n",
        "class Derived extends State { init() { super(); }\n",
        "  static fn replace_inherited() -> unit { Derived.scalar = 14; } }\n",
        "fn main() -> i64 { State.scalar = 15; return 0; }\n",
    ));

    assert!(output.hir.is_none());
    let diagnostics = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == crate::typeck::FINAL_STATIC_REPLACEMENT)
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 11, "{:?}", output.diagnostics);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic
            .labels
            .iter()
            .any(|label| label.message == "field declared final here")
    }));
}

#[test]
fn final_static_reads_and_shallow_nested_mutation_remain_available() {
    let output = check_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; }\n",
        "  mut fn increment() -> unit { self.value = self.value + 1; } }\n",
        "class State {\n",
        "  final static scalar: i64 = 1;\n",
        "  final static object: Item = Item(10);\n",
        "  final static values: i64[] = i64[]{20};\n",
        "  final static owner: shared Item = new Item(11);\n",
        "  init() {}\n",
        "}\n",
        "fn inspect(ref value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 {\n",
        "  State.object.increment(); State.values[0] = State.values[0] + 1;\n",
        "  State.owner->increment();\n",
        "  return inspect(State.scalar) + State.object.value + State.values[0] + State.owner->value;\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let preliminary = crate::mir::lower_preliminary_hir(&hir);
    crate::mir::check_preliminary_mir(&preliminary).unwrap();
    verify_mir(&lower_hir_to_final_mir(&hir)).unwrap();
}

#[test]
fn final_static_roots_are_read_only_alias_sources() {
    let output = check_text(concat!(
        "fn replace_scalar(mut ref value: i64) -> unit { value = 0; }\n",
        "fn replace_optional(mut ref value: i64?) -> unit { value = none; }\n",
        "fn mutate_array(mut ref value: i64[]) -> unit { value[0] = value[0] + 1; }\n",
        "class State {\n",
        "  final static scalar: i64 = 1;\n",
        "  final static optional: i64? = 2;\n",
        "  final static values: i64[] = i64[]{3};\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { replace_scalar(State.scalar); ",
        "replace_optional(State.optional); mutate_array(State.values); return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INSUFFICIENT_ALIAS_ACCESS)
            .count(),
        2,
        "{:?}",
        output.diagnostics
    );
}

#[test]
fn rebinding_capable_final_field_roots_are_read_only_alias_sources() {
    let output = check_text(concat!(
        "fn clear(mut ref value: i64?) -> unit { value = none; }\n",
        "class Holder { final maybe: i64?; final values: i64[];\n",
        "  init() { self.maybe = 1; self.values = i64[]{2}; } }\n",
        "fn main() -> i64 { var holder: Holder = Holder();\n",
        "  clear(holder.maybe); return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INSUFFICIENT_ALIAS_ACCESS)
            .count(),
        1,
        "{:?}",
        output.diagnostics
    );
}

#[test]
fn final_inline_objects_and_array_elements_keep_shallow_mutable_aliases() {
    let output = check_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn touch(mut ref item: Item) -> unit { item.value = item.value + 1; }\n",
        "fn increment(mut ref value: i64) -> unit { value = value + 1; }\n",
        "fn mutate(mut ref values: i64[]) -> unit { values[0] = values[0] + 1; }\n",
        "class Holder { final item: Item; final values: i64[];\n",
        "  init() { self.item = Item(20); self.values = i64[]{20}; } }\n",
        "fn main() -> i64 { var holder: Holder = Holder();\n",
        "  touch(holder.item); increment(holder.values[0]); mutate(holder.values);\n",
        "  return holder.item.value + holder.values[0] - 1; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    verify_mir(&lower_hir_to_final_mir(&output.hir.unwrap())).unwrap();
}
