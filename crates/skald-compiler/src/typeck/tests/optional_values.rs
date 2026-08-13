use super::*;

#[test]
fn optional_metadata_selects_recursive_and_array_payload_plans() {
    let program = resolve_text(
        "fn nested(value: i64??) -> unit {}\n\
         fn array(value: (i64[])?) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    let capabilities = super::super::capabilities::CopyCapabilities::compute(&program);
    let optionals = super::super::optional_types::lower_optional_types(&program, &capabilities);

    let scalar = optionals
        .get(crate::identity::OptionalTypeId::new(0))
        .unwrap();
    assert_eq!(scalar.payload, Type::I64);
    assert_eq!(
        scalar.storage,
        crate::hir::HirOptionalStorageCategory::Scalar
    );
    assert_eq!(
        scalar.lifecycle.copy,
        Some(crate::hir::HirOptionalCopyPlan::Trivial)
    );

    let nested = optionals
        .get(crate::identity::OptionalTypeId::new(1))
        .unwrap();
    assert_eq!(nested.payload, Type::Optional(scalar.id));
    assert_eq!(
        nested.storage,
        crate::hir::HirOptionalStorageCategory::Nested(scalar.id)
    );
    assert_eq!(
        nested.lifecycle.copy,
        Some(crate::hir::HirOptionalCopyPlan::Optional(scalar.id))
    );
    assert_eq!(
        nested.lifecycle.unwrap,
        crate::hir::HirOptionalUnwrapPlan::CheckedNested(scalar.id)
    );

    let array = optionals
        .get(crate::identity::OptionalTypeId::new(2))
        .unwrap();
    let Type::Array(array_id) = array.payload else {
        panic!("expected array payload")
    };
    assert_eq!(
        array.storage,
        crate::hir::HirOptionalStorageCategory::InlineArray(array_id)
    );
    assert_eq!(
        array.lifecycle.copy,
        Some(crate::hir::HirOptionalCopyPlan::Array(array_id))
    );
    assert_eq!(
        array.boundaries.argument,
        crate::hir::HirOptionalBoundaryPlan::Copy(crate::hir::HirOptionalCopyPlan::Array(array_id),)
    );
}

#[test]
fn supported_optional_hir_uses_ids_and_dumps_selected_metadata_deterministically() {
    let output = check_text(
        "class Item { init() {} }\n\
         fn main() -> i64 {\n\
           var number: i64? = 1;\n\
           var item: Item? = Item();\n\
           var owner: shared? Item = none;\n\
           return 0;\n\
         }\n",
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    assert_eq!(hir.optional_types.iter().count(), 3);
    assert!(hir.optional_types.iter().all(|optional| matches!(
        optional.storage,
        crate::hir::HirOptionalStorageCategory::Scalar
            | crate::hir::HirOptionalStorageCategory::InlineClass(_)
            | crate::hir::HirOptionalStorageCategory::SharedOwner(_)
    )));
    let first = dump_hir(&hir);
    let second = dump_hir(&hir);
    assert_eq!(first, second);
    assert!(first.contains("OptionalTypes"));
    assert!(first.contains("Lifecycle initialization="));
    assert!(first.contains("Boundaries argument="));
}

#[test]
fn types_primitive_optional_construction_copy_assignment_and_inspection() {
    let output = check_text(
        "fn main() -> i64 {\n\
           var empty: i64? = none;\n\
           var present: i64? = 41;\n\
           var copied: i64? = present;\n\
           var unsigned: u64? = 1u;\n\
           var byte: u8? = 2u8;\n\
           var float: f64? = 3.0;\n\
           var flag: bool? = true;\n\
           empty = copied;\n\
           if (empty is some) { return (empty)! + 1; }\n\
           return 0;\n\
         }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output
        .hir
        .expect("primitive optional locals must produce HIR");
    let function = hir.definitions.get(hir.entry_function).unwrap();
    let Type::Optional(optional) = function.locals[0].ty else {
        panic!("expected optional")
    };
    assert_eq!(hir.optional_type(optional).unwrap().payload, Type::I64);
    let dump = dump_hir(&hir);
    assert!(dump.contains("OptionalAbsent"));
    assert!(dump.contains("OptionalPresent"));
    assert!(dump.contains("OptionalCopy"));
    assert!(dump.contains("OptionalAssignment"));
    assert!(dump.contains("PresenceTest Some"));
    assert!(dump.contains("OptionalUnwrap"));
}

#[test]
fn optionals_have_no_truthiness_or_implicit_unwrap() {
    for source in [
        "fn main() -> i64 { var value: i64? = 1; if (value) { return 1; } return 0; }",
        "fn main() -> i64 { var value: i64? = 1; var plain: i64 = value; return plain; }",
        "fn main() -> i64 { none; return 0; }",
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == TYPE_MISMATCH));
    }
}

#[test]
fn optional_container_aliases_preserve_type_and_access() {
    let output = check_text(
        "class Item {\n\
           value: i64;\n\
           init(value: i64) { self.value = value; }\n\
           fn read() -> i64 { return self.value; }\n\
         }\n\
         fn inspect(ref value: i64?) -> i64 {\n\
           if (value is some) { return value!; }\n\
           return 0;\n\
         }\n\
         fn inspect_item(ref value: Item?) -> i64 {\n\
           if (value is some) { return value!.read(); }\n\
           return 0;\n\
         }\n\
         fn clear(mut ref value: i64?) -> unit { value = none; }\n\
         fn main() -> i64 {\n\
           var number: i64? = 7;\n\
           var item: Item? = Item(4);\n\
           var result: i64 = inspect(number) + inspect_item(item);\n\
           clear(number);\n\
           return result;\n\
         }\n",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.unwrap());
    assert!(dump.contains("OptionalPlaceArgument i64?"));
    assert!(dump.contains("OptionalPlaceArgument class"));
}

#[test]
fn optional_aliases_reject_replacement_without_mutable_exact_place_access() {
    let output = check_text(
        "class Item { init() {} }\n\
         fn clear_read_only(ref value: Item?) -> unit { value = none; }\n\
         fn take(mut ref value: Item?) -> unit {}\n\
         fn misuse(ref value: Item?, other: i64?, plain: Item) -> unit {\n\
           take(value);\n\
           take(other);\n\
           take(plain);\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.hir.is_none());
    let codes: Vec<_> = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(
        codes,
        [
            READ_ONLY_RECEIVER,
            INSUFFICIENT_ALIAS_ACCESS,
            TYPE_MISMATCH,
            INVALID_ALIAS_ARGUMENT,
        ]
    );
}

#[test]
fn primitive_optional_fields_parameters_and_results_cross_the_type_boundary() {
    let output = check_text(
        "class Holder {\n\
           value: i64?;\n\
           init(value: i64?) { self.value = value; }\n\
           mut fn replace(value: i64?) -> i64? { self.value = value; return self.value; }\n\
         }\n\
         fn identity(value: i64?) -> i64? { return value; }\n\
         fn main() -> i64 {\n\
           var holder: Holder = Holder(none);\n\
           var value: i64? = holder.replace(identity(42));\n\
           if (value is some) { return (value)!; }\n\
           return 0;\n\
         }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.expect("supported optionals must produce HIR"));
    assert!(dump.contains("OptionalArgument"));
    assert!(dump.contains("OptionalPrimitive"));
    assert!(dump.contains("OptionalProduced"));
}

#[test]
fn contextual_none_and_optional_injection_select_initializer_overloads() {
    let output = check_text(
        "class Pick {\n\
           chosen: i64;\n\
           init(value: i64) { self.chosen = 1; }\n\
           init(value: i64?) { self.chosen = 2; }\n\
         }\n\
         fn main() -> i64 {\n\
           var exact: Pick = Pick(7);\n\
           var absent: Pick = Pick(none);\n\
           return exact.chosen + absent.chosen;\n\
         }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.expect("overloads must select into HIR"));
    assert!(dump.contains("Initializer c0:init0"));
    assert!(dump.contains("Initializer c0:init1"));
}

#[test]
fn none_keeps_distinct_optional_initializer_candidates_ambiguous() {
    let output = check_text(
        "class Pick {\n\
           init(value: i64?) {}\n\
           init(value: bool?) {}\n\
         }\n\
         fn main() -> i64 { var value: Pick = Pick(none); return 0; }\n",
    );

    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == AMBIGUOUS_INITIALIZER));
}

#[test]
fn nested_optional_overloads_rank_exact_layers_and_contextual_construction() {
    let output = check_text(
        "class Pick {\n\
           chosen: i64;\n\
           init(value: i64?) { self.chosen = 1; }\n\
           init(value: i64??) { self.chosen = 2; }\n\
         }\n\
         fn main() -> i64 {\n\
           var inner: i64? = some(7);\n\
           var outer: i64?? = some(inner);\n\
           var from_inner: Pick = Pick(inner);\n\
           var from_outer: Pick = Pick(outer);\n\
           var contextual: Pick = Pick(some(none));\n\
           return from_inner.chosen + from_outer.chosen + contextual.chosen;\n\
         }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.expect("nested overloads must select into HIR"));
    assert_eq!(
        dump.matches("Construct c0 via c0:init0").count(),
        1,
        "{dump}"
    );
    assert_eq!(
        dump.matches("Construct c0 via c0:init1").count(),
        2,
        "{dump}"
    );
}

#[test]
fn optional_shared_overloads_rank_compatible_targets_by_specificity() {
    let output = check_text(
        "class Base { init() {} }\n\
         class Derived extends Base { init() { super(); } }\n\
         class Pick {\n\
           chosen: i64;\n\
           init(value: shared? Base) { self.chosen = 1; }\n\
           init(value: shared? Derived) { self.chosen = 2; }\n\
         }\n\
         fn main() -> i64 {\n\
           var owner: shared? Derived = new Derived();\n\
           var pick: Pick = Pick(owner);\n\
           return pick.chosen;\n\
         }\n",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.unwrap());
    assert!(dump.contains("Initializer c2:init1"));
}

#[test]
fn external_optional_parameters_and_results_are_rejected_by_the_external_contract() {
    for source in [
        "extern fn inspect(value: i64?) -> unit; fn main() -> i64 { return 0; }",
        "extern fn inspect() -> i64?; fn main() -> i64 { return 0; }",
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == INVALID_EXTERNAL_DECLARATION));
    }
}

#[test]
fn optional_signature_matching_is_exact_for_overrides_and_interfaces() {
    let valid = check_text(
        "interface Mapper { fn map(value: i64?) -> i64?; }\n\
         class Base { init() {} virtual fn map(value: i64?) -> i64? { return value; } }\n\
         class Derived extends Base implements Mapper {\n\
           init() { super(); }\n\
           override fn map(value: i64?) -> i64? { return value; }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(valid.diagnostics.is_empty(), "{:?}", valid.diagnostics);

    let invalid = check_text(
        "class Base { init() {} virtual fn map(value: i64?) -> i64? { return value; } }\n\
         class Derived extends Base {\n\
           init() { super(); }\n\
           override fn map(value: i64) -> i64? { return value; }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(invalid.hir.is_none());
    assert!(!invalid.diagnostics.is_empty());
}

#[test]
fn optional_shared_owners_type_across_owning_positions_and_unwrap() {
    let output = check_text(
        "interface Readable { fn read() -> i64; }\n\
         class Item implements Readable {\n\
           value: i64;\n\
           init(value: i64) { self.value = value; }\n\
           fn read() -> i64 { return self.value; }\n\
         }\n\
         class Holder {\n\
           item: shared? Item;\n\
           init(item: shared? Item) { self.item = item; }\n\
           mut fn replace(item: shared? Item) -> shared? Item {\n\
             self.item = item;\n\
             return self.item;\n\
           }\n\
         }\n\
         fn forward(item: shared? Item) -> shared? Item { return item; }\n\
         fn main() -> i64 {\n\
           var item: shared? Item = none;\n\
           item = new Item(40);\n\
           var readable: shared? Readable = item;\n\
           var object: shared? Obj = readable;\n\
           var holder: Holder = Holder(forward(item));\n\
           item = holder.replace(none);\n\
           item = new Item(42);\n\
           if (item is some) { return item!->read(); }\n\
           return 0;\n\
         }\n",
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.expect("optional shared owners must produce HIR"));
    assert!(dump.contains("OptionalSharedInitialization"));
    assert!(dump.contains("OptionalSharedAssignment"));
    assert!(dump.contains("OptionalSharedUnwrap"));
}

#[test]
fn class_optionals_type_across_owning_positions_without_payload_access() {
    let output = check_text(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n\
         class Holder {\n\
           item: Item?;\n\
           init(item: Item?) { self.item = item; }\n\
           mut fn replace(item: Item?) -> Item? { self.item = item; return self.item; }\n\
         }\n\
         fn forward(item: Item?) -> Item? { return item; }\n\
         fn main() -> i64 {\n\
           var item: Item? = Item(7);\n\
           var holder: Holder = Holder(item);\n\
           item = none;\n\
           item = holder.replace(forward(Item(8)));\n\
           if (item is some) { return 42; }\n\
           return 0;\n\
         }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.expect("class optionals must produce HIR"));
    assert!(dump.contains("ClassOptionalInitialization"));
    assert!(dump.contains("ClassOptionalAssignment"));
    assert!(dump.contains("OptionalClass"));
}

#[test]
fn class_optional_unwrap_supplies_bounded_checked_object_consumers() {
    let output = check_text(
        "class Item {\n\
           value: i64;\n\
           init(value: i64) { self.value = value; }\n\
           mut fn set(value: i64) -> unit { self.value = value; }\n\
         }\n\
         class Holder { item: Item?; init(item: Item?) { self.item = item; } }\n\
         fn inspect(ref item: Item) -> i64 { return item.value; }\n\
         fn main() -> i64 {\n\
           var holder: Holder = Holder(Item(1));\n\
           holder.item!.set(40);\n\
           holder.item!.value = holder.item!.value + 1;\n\
           var copied: Item = holder.item!;\n\
           if (holder.item! is Item) { return inspect(holder.item!) + copied.value - 40; }\n\
           return 0;\n\
         }\n",
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(
        &output
            .hir
            .expect("checked payload consumers must produce HIR"),
    );
    assert!(dump.contains("CheckedOptionalPayload"));
    assert!(dump.contains("OptionalMethodReceiver"));
    assert!(dump.contains("OptionalFieldReceiver"));
}

#[test]
fn class_optional_payloads_participate_in_recursive_containment() {
    let output = check_text(
        "class Node { next: Node?; init() { self.next = none; } }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == RECURSIVE_INLINE_CONTAINMENT));
}

#[test]
fn containment_traverses_every_optional_layer_but_stops_at_array_and_shared_edges() {
    let recursive = check_text(
        "class Node { next: Node???; init() { self.next = none; } }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(recursive.hir.is_none());
    assert!(recursive
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == RECURSIVE_INLINE_CONTAINMENT));
    assert!(recursive
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != crate::typeck::INVALID_OPTIONAL_TYPE));

    let bounded = check_text(
        "class Node { children: Node[]?; owner: (shared Node)??; }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(bounded.hir.is_none());
    assert!(bounded
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != RECURSIVE_INLINE_CONTAINMENT));
}

#[test]
fn nested_optional_construction_adds_exactly_one_present_layer() {
    let output = check_text(
        "fn main() -> i64 {\n\
           var inner: i64? = none;\n\
           var absent: i64?? = none;\n\
           var present_absent: i64?? = some(none);\n\
           var copied_payload: i64?? = some(inner);\n\
           var present_present: i64?? = some(some(42));\n\
           absent = present_absent;\n\
           present_absent = some(inner);\n\
           if (present_present is some) { return 42; }\n\
           return 0;\n\
         }\n",
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.expect("nested optionals must produce HIR"));
    assert!(dump.contains("AggregateOptionalInitialization"));
    assert!(dump.contains("Present"));
    assert!(dump.contains("Absent"));
    assert!(dump.contains("AggregateOptionalAssignment"));
}

#[test]
fn nested_optional_conversion_never_lifts_through_multiple_layers() {
    let output = check_text("fn main() -> i64 { var invalid: i64?? = 1; return 0; }\n");
    assert!(output.hir.is_none());
    assert!(
        output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("i64??") && diagnostic.message.contains("i64")
        }),
        "{:?}",
        output.diagnostics
    );
}

#[test]
fn optional_arrays_type_as_core_local_and_function_values() {
    let output = check_text(
        "fn forward(value: i64[]?) -> i64[]? { return value; }\n\
         fn main() -> i64 {\n\
           var absent: i64[]? = none;\n\
           var empty: (i64[])? = some(i64[]{});\n\
           var values: i64[]? = i64[]{40, 2};\n\
           values = forward(values);\n\
           if (absent is none && empty is some) { return values![0] + values![1]; }\n\
           return 0;\n\
         }\n",
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.expect("optional arrays must produce HIR"));
    assert!(dump.contains("OptionalArrayUnwrap"));
    assert!(dump.contains("AggregateOptionalAssignment"));
    assert!(!dump.contains("NestedOptionalInitialization"));
    assert!(dump.contains("ArrayConstruction"));
}

#[test]
fn aggregate_operation_names_preserve_nested_optional_unwrap_semantics() {
    let output = check_text(
        "fn unwrap(value: i64??) -> i64? { return value!; }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.expect("nested unwrap must produce HIR"));

    assert!(dump.contains("NestedOptionalUnwrap"), "{dump}");
    assert!(dump.contains("AggregateOptionalPlace"), "{dump}");
}

#[test]
fn nested_unwrap_can_produce_an_inline_class_optional() {
    let output = check_text(
        "class Item { init() {} }\n\
         fn unwrap(value: Item??) -> Item? { return value!; }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.expect("nested class unwrap must produce HIR"));
    assert!(dump.contains("NestedOptionalUnwrap"), "{dump}");
    assert!(dump.contains("ClassOptionalInitialization"), "{dump}");
}

#[test]
fn optional_arrays_type_across_aggregate_and_alias_positions() {
    let output = check_text(
        "class Holder {\n\
           values: i64[]?;\n\
           init(values: i64[]?) { self.values = values; }\n\
           fn replace(values: i64[]?) -> i64[]? { return values; }\n\
         }\n\
         fn inspect(ref values: i64[]?) -> unit {}\n\
         fn main() -> i64 { var nested: i64[]?[] = i64[]?[](); return 0; }\n",
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(
        &output
            .hir
            .expect("aggregate optional arrays must produce HIR"),
    );
    assert!(dump.contains("OptionalField"));
    assert!(dump.contains("AggregateOptionalInitialization"));
    assert!(dump.contains("array"));
}
