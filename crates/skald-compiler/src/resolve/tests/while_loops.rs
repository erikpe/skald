use crate::identity::LoopId;

use super::*;

#[test]
fn resolves_loop_conditions_in_the_enclosing_scope_and_ids_in_source_order() {
    let output = resolve_text(concat!(
        "fn main() -> i64 {\n",
        "  var condition: bool = true;\n",
        "  while (condition) {\n",
        "    var inner_condition: bool = false;\n",
        "    while (inner_condition) {}\n",
        "  }\n",
        "  while (condition) {}\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(!output.has_errors());
    let main = output
        .program
        .definitions
        .get(output.program.entry_function.unwrap())
        .unwrap();
    let ResolvedStatement::While(outer) = &main.body.statements[1] else {
        panic!("expected outer resolved while");
    };
    let ResolvedStatement::While(inner) = &outer.body.statements[1] else {
        panic!("expected nested resolved while");
    };
    let ResolvedStatement::While(later) = &main.body.statements[2] else {
        panic!("expected later resolved while");
    };
    assert_eq!(
        [outer.loop_id, inner.loop_id, later.loop_id],
        [
            LoopId::new(main.function, 0),
            LoopId::new(main.function, 1),
            LoopId::new(main.function, 2),
        ]
    );
    assert!(matches!(
        outer.condition,
        ResolvedExpression::Binding(ResolvedBindingExpr {
            binding: BindingId::Local(_),
            ..
        })
    ));

    let dump = dump_resolved(&output.program);
    assert_eq!(dump.matches("While f0:loop").count(), 3);
    assert!(dump.find("While f0:loop0").unwrap() < dump.find("While f0:loop2").unwrap());
}

#[test]
fn loop_body_bindings_do_not_escape_to_later_conditions() {
    let output = resolve_text(concat!(
        "fn main() -> i64 {\n",
        "  while (true) { var hidden: bool = false; }\n",
        "  while (hidden) {}\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(output.has_errors());
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == UNKNOWN_NAME)
            .count(),
        1
    );
}

#[test]
fn resolves_break_to_the_nearest_loop_and_rejects_it_outside_loops() {
    let output = resolve_text(concat!(
        "fn main() -> i64 {\n",
        "  while (true) {\n",
        "    while (true) { break; }\n",
        "    { break; }\n",
        "  }\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(!output.has_errors());
    let main = output
        .program
        .definitions
        .get(output.program.entry_function.unwrap())
        .unwrap();
    let ResolvedStatement::While(outer) = &main.body.statements[0] else {
        panic!("expected outer loop");
    };
    let ResolvedStatement::While(inner) = &outer.body.statements[0] else {
        panic!("expected inner loop");
    };
    let ResolvedStatement::Break(inner_break) = &inner.body.statements[0] else {
        panic!("expected inner break");
    };
    let ResolvedStatement::Block(block) = &outer.body.statements[1] else {
        panic!("expected nested block");
    };
    let ResolvedStatement::Break(outer_break) = &block.statements[0] else {
        panic!("expected outer break");
    };
    assert_eq!(inner_break.target, inner.loop_id);
    assert_eq!(outer_break.target, outer.loop_id);

    let dump = dump_resolved(&output.program);
    assert!(dump.contains("Break f0:loop1"));
    assert!(dump.contains("Break f0:loop0"));

    let outside = resolve_text("fn main() -> i64 { break; return 0; }");
    assert!(outside.has_errors());
    let diagnostics: Vec<_> = outside.diagnostics.iter().collect();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, LOOP_EXIT_OUTSIDE_LOOP);
    assert_eq!(diagnostics[0].message, "`break` requires an enclosing loop");
}

#[test]
fn resolves_continue_to_the_nearest_loop_and_rejects_it_outside_loops() {
    let output = resolve_text(concat!(
        "fn main() -> i64 {\n",
        "  while (true) {\n",
        "    while (true) { continue; }\n",
        "    { continue; }\n",
        "  }\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(!output.has_errors());
    let main = output
        .program
        .definitions
        .get(output.program.entry_function.unwrap())
        .unwrap();
    let ResolvedStatement::While(outer) = &main.body.statements[0] else {
        panic!("expected outer loop");
    };
    let ResolvedStatement::While(inner) = &outer.body.statements[0] else {
        panic!("expected inner loop");
    };
    let ResolvedStatement::Continue(inner_continue) = &inner.body.statements[0] else {
        panic!("expected inner continue");
    };
    let ResolvedStatement::Block(block) = &outer.body.statements[1] else {
        panic!("expected nested block");
    };
    let ResolvedStatement::Continue(outer_continue) = &block.statements[0] else {
        panic!("expected outer continue");
    };
    assert_eq!(inner_continue.target, inner.loop_id);
    assert_eq!(outer_continue.target, outer.loop_id);

    let dump = dump_resolved(&output.program);
    assert!(dump.contains("Continue f0:loop1"));
    assert!(dump.contains("Continue f0:loop0"));

    let outside = resolve_text("fn main() -> i64 { continue; return 0; }");
    assert!(outside.has_errors());
    let diagnostics: Vec<_> = outside.diagnostics.iter().collect();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, LOOP_EXIT_OUTSIDE_LOOP);
    assert_eq!(
        diagnostics[0].message,
        "`continue` requires an enclosing loop"
    );
}
