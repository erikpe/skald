use super::*;

#[test]
fn rejects_resolved_optional_execution_once_before_hir() {
    let output = check_text(
        "fn inspect(value: i64?) -> bool {\n\
           var empty: i64? = none;\n\
           value!;\n\
           return value is some;\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.hir.is_none());
    assert_eq!(output.diagnostics.len(), 1);
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, OPTIONAL_VALUES_NOT_IMPLEMENTED);
    assert_eq!(diagnostic.message, "optional values are not executable yet");
}

#[test]
fn optional_gate_covers_shared_owners_and_expression_only_use() {
    for source in [
        "class Item { init() {} }\n\
         fn inspect(value: shared? Item) -> unit {}\n\
         fn main() -> i64 { return 0; }",
        "fn inspect() -> unit { none; }\n\
         fn main() -> i64 { return 0; }",
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(
            output.diagnostics.iter().next().unwrap().code,
            OPTIONAL_VALUES_NOT_IMPLEMENTED
        );
    }
}
