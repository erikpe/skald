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
fn unsupported_optional_positions_still_stop_before_hir() {
    for source in [
        "fn inspect(value: i64?) -> unit {} fn main() -> i64 { return 0; }",
        "fn inspect() -> i64? { return none; } fn main() -> i64 { return 0; }",
        "class Item { value: i64?; init() { self.value = none; } } fn main() -> i64 { return 0; }",
        "class Item { init() {} } fn main() -> i64 { var item: Item? = none; return 0; }",
        "class Item { init() {} } fn main() -> i64 { var item: shared? Item = none; return 0; }",
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
        assert_eq!(
            output.diagnostics.iter().next().unwrap().code,
            OPTIONAL_VALUES_NOT_IMPLEMENTED
        );
    }
}
