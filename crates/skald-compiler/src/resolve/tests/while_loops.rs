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
