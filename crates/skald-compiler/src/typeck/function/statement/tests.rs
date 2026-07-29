use crate::{
    hir::HirStatement,
    identity::FunctionId,
    test_support::type_check_source,
    typeck::{INVALID_CALL_STATEMENT, TYPE_MISMATCH},
};

#[test]
fn blocks_and_conditionals_retain_structured_flow_composition() {
    let output = type_check_source(concat!(
        "fn inspect(value: bool) -> unit {\n",
        "  if (value) { return; } else {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  { if (true) { return 1; } else { return 2; } }\n",
        "  return 3;\n",
        "}\n",
    ));
    assert!(!output.has_errors());
    let hir = output.hir.unwrap();

    let inspect = hir.definitions.get(FunctionId::new(0)).unwrap();
    assert!(inspect.body.effects.can_fall_through());
    let HirStatement::Conditional(conditional) = &inspect.body.statements[0] else {
        panic!("expected conditional");
    };
    assert!(conditional.effects.can_fall_through());
    assert!(!conditional.arms[0].body.effects.can_fall_through());
    assert!(conditional
        .else_block
        .as_ref()
        .unwrap()
        .effects
        .can_fall_through());

    let main = hir.definitions.get(hir.entry_function).unwrap();
    assert!(!main.body.effects.can_fall_through());
    let HirStatement::Block(nested) = &main.body.statements[0] else {
        panic!("expected nested block");
    };
    assert!(!nested.effects.can_fall_through());
    let HirStatement::Conditional(conditional) = &nested.statements[0] else {
        panic!("expected nested conditional");
    };
    assert!(!conditional.effects.can_fall_through());
}

#[test]
fn independent_statement_errors_accumulate_in_existing_order() {
    let output = type_check_source(concat!(
        "fn value() -> i64 { return 1; }\n",
        "fn main() -> i64 {\n",
        "  var local: i64 = true;\n",
        "  value();\n",
        "  if (1) { return false; } else { return true; }\n",
        "}\n",
    ));

    assert!(output.hir.is_none());
    let diagnostics: Vec<_> = output.diagnostics.iter().collect();
    assert_eq!(diagnostics.len(), 5);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.code, diagnostic.message.as_str()))
            .collect::<Vec<_>>(),
        [
            (
                TYPE_MISMATCH,
                "local initializer has type `bool` but `i64` is required"
            ),
            (
                INVALID_CALL_STATEMENT,
                "a call statement must call a function returning `unit`"
            ),
            (
                TYPE_MISMATCH,
                "return value has type `bool` but `i64` is required"
            ),
            (
                TYPE_MISMATCH,
                "conditional condition has type `i64` but `bool` is required"
            ),
            (
                TYPE_MISMATCH,
                "return value has type `bool` but `i64` is required"
            ),
        ]
    );
}
