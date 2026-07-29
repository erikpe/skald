use crate::{
    hir::HirStatement,
    identity::LoopId,
    typeck::{MISSING_RETURN, TYPE_MISMATCH},
};

use super::*;

#[test]
fn checks_exact_boolean_while_conditions_and_builds_structured_hir() {
    let output = check_text(concat!(
        "fn main() -> i64 {\n",
        "  var count: i64 = 0;\n",
        "  while (count < 2) { count = count + 1; }\n",
        "  return count;\n",
        "}\n",
    ));

    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    let main = hir.definitions.get(hir.entry_function).unwrap();
    let HirStatement::While(statement) = &main.body.statements[1] else {
        panic!("expected typed while");
    };
    assert_eq!(statement.loop_id, LoopId::new(main.function, 0));
    assert_eq!(statement.condition.ty, Type::Bool);
    assert!(statement.effects.can_fall_through());
    let dump = dump_hir(&hir);
    assert!(dump.contains("While f0:loop0"));
    assert!(dump.contains("IntegerComparison lt.i64 : bool"));
}

#[test]
fn rejects_non_boolean_while_conditions_without_truthiness() {
    for (condition, actual) in [("1", "i64"), ("1.0", "f64")] {
        let source = format!("fn main() -> i64 {{ while ({condition}) {{}} return 0; }}");
        let output = check_text(&source);

        assert!(output.hir.is_none());
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == TYPE_MISMATCH
                && diagnostic.message
                    == format!("while condition has type `{actual}` but `bool` is required")
        }));
    }
}

#[test]
fn every_while_conservatively_falls_through_for_definite_return() {
    for source in [
        "fn main() -> i64 { while (true) { return 1; } }",
        "fn main() -> i64 { while (false) { return 1; } }",
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == MISSING_RETURN));
    }
}
