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
        assert!(!output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == OPTIONAL_VALUES_NOT_IMPLEMENTED));
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
fn unsupported_optional_positions_still_stop_before_hir() {
    let output = check_text(
        "class Item { init() {} } fn main() -> i64 { var item: shared? Item = none; return 0; }",
    );
    assert!(output.hir.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        OPTIONAL_VALUES_NOT_IMPLEMENTED
    );
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
fn class_optional_unwrap_remains_reserved_for_checked_views() {
    let output = check_text(
        "class Item { init() {} }\n\
         fn main() -> i64 { var item: Item? = Item(); item!; return 0; }\n",
    );
    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == OPTIONAL_VALUES_NOT_IMPLEMENTED));
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
