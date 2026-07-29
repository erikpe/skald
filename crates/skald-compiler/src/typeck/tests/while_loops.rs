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

#[test]
fn break_effects_stop_their_path_and_preserve_sibling_outcomes() {
    let output = check_text(concat!(
        "fn main() -> i64 {\n",
        "  while (true) {\n",
        "    if (true) { break; return 1; } else { return 2; }\n",
        "  }\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    let main = hir.definitions.get(hir.entry_function).unwrap();
    let HirStatement::While(statement) = &main.body.statements[0] else {
        panic!("expected typed loop");
    };
    let HirStatement::Conditional(conditional) = &statement.body.statements[0] else {
        panic!("expected typed conditional");
    };
    let target = statement.loop_id;
    assert!(conditional.effects.can_break_to(target));
    assert!(conditional.effects.can_exit_function());
    assert!(!conditional.effects.can_fall_through());
    assert!(statement.effects.can_fall_through());
    assert!(statement.effects.can_exit_function());
    assert!(!statement.effects.can_break_to(target));

    let break_arm = &conditional.arms[0].body;
    assert!(break_arm.effects.can_break_to(target));
    assert!(!break_arm.effects.can_exit_function());
    assert!(!break_arm.effects.can_fall_through());

    let dump = dump_hir(&hir);
    assert!(dump.contains("Break f0:loop0"));
}

#[test]
fn continue_effects_stop_their_path_and_compose_independently() {
    let output = check_text(concat!(
        "fn main() -> i64 {\n",
        "  while (true) {\n",
        "    if (true) { continue; return 1; }\n",
        "    elif (false) { break; }\n",
        "    else { return 2; }\n",
        "  }\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    let main = hir.definitions.get(hir.entry_function).unwrap();
    let HirStatement::While(statement) = &main.body.statements[0] else {
        panic!("expected typed loop");
    };
    let HirStatement::Conditional(conditional) = &statement.body.statements[0] else {
        panic!("expected typed conditional");
    };
    let target = statement.loop_id;
    assert!(conditional.effects.can_continue_to(target));
    assert!(conditional.effects.can_break_to(target));
    assert!(conditional.effects.can_exit_function());
    assert!(!conditional.effects.can_fall_through());

    let continue_arm = &conditional.arms[0].body;
    assert!(continue_arm.effects.can_continue_to(target));
    assert!(!continue_arm.effects.can_break_to(target));
    assert!(!continue_arm.effects.can_exit_function());
    assert!(!continue_arm.effects.can_fall_through());

    assert!(statement.effects.can_fall_through());
    assert!(statement.effects.can_exit_function());
    assert!(!statement.effects.can_continue_to(target));
    assert!(!statement.effects.can_break_to(target));

    let dump = dump_hir(&hir);
    assert!(dump.contains("Continue f0:loop0"));
    assert!(dump.contains("Break f0:loop0"));
}
