use super::*;

#[test]
fn shared_fields_are_non_containing_edges_with_complete_initialization_rules() {
    let valid = type_check_source(concat!(
        "class Left {\n",
        "  right: shared Right;\n",
        "  init(right: shared Right) { self.right = right; }\n",
        "  fn snapshot() -> shared Right { return self.right; }\n",
        "}\n",
        "class Right {\n",
        "  left: shared Left;\n",
        "  init(left: shared Left) { self.left = left; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert_diagnostics(&valid.diagnostics, &[]);

    let invalid = type_check_source(concat!(
        "class Item { init() {} }\n",
        "class Holder {\n",
        "  value: shared Item;\n",
        "  init() {}\n",
        "  fn invalid(value: shared Item) -> unit { self.value = value; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(invalid.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("field `value` is not initialized")));
    assert!(invalid.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("cannot assign through a read-only receiver")));
}
