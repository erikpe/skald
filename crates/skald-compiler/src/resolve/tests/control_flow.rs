use super::*;

#[test]
fn resolves_conditional_conditions_in_the_containing_scope_and_arms_independently() {
    let output = resolve_text(concat!(
        "fn choose(flag: bool) -> i64 {\n",
        "  if (flag) { var value: i64 = 1; }\n",
        "  elif (false) { var value: i64 = 2; }\n",
        "  else { var value: i64 = 3; }\n",
        "  return 0;\n",
        "}\n",
        "fn main() -> i64 { return choose(true); }\n",
    ));

    assert!(!output.has_errors());
    let choose = output.program.definitions.get(FunctionId::new(0)).unwrap();
    assert_eq!(choose.locals.len(), 3);
    let ResolvedStatement::Conditional(conditional) = &choose.body.statements[0] else {
        panic!("expected resolved conditional");
    };
    assert_eq!(conditional.arms.len(), 2);
    assert!(conditional.else_block.is_some());
    assert!(matches!(
        conditional.arms[0].condition,
        ResolvedExpression::Binding(ResolvedBindingExpr {
            binding: BindingId::Parameter(_),
            ..
        })
    ));
    let dump = dump_resolved(&output.program);
    assert_eq!(dump.matches("ElifArm").count(), 1);
    assert!(dump.find("IfArm").unwrap() < dump.find("ElseArm").unwrap());
}

#[test]
fn branch_bindings_do_not_escape_or_enter_later_conditions() {
    let output = resolve_text(concat!(
        "fn main() -> i64 {\n",
        "  if (true) { var hidden: bool = true; }\n",
        "  elif (hidden) {}\n",
        "  hidden;\n",
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
        2
    );
}
