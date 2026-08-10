use super::*;

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
    assert_eq!(
        function.locals[0].ty,
        Type::OptionalPrimitive(crate::hir::HirPrimitiveType::I64)
    );
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
        .any(|diagnostic| diagnostic.code == crate::typeck::INVALID_OPTIONAL_TYPE));

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
