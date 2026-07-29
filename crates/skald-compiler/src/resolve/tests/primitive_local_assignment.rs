use super::*;

#[test]
fn resolves_primitive_local_assignments_to_exact_local_identities() {
    let output = resolve_text(concat!(
        "fn main() -> i64 {\n",
        "  var signed: i64 = 0;\n",
        "  var unsigned: u64 = 0u;\n",
        "  var byte: u8 = 0u8;\n",
        "  var float: f64 = 0.0;\n",
        "  var flag: bool = false;\n",
        "  signed = 1;\n",
        "  (unsigned) = 2u;\n",
        "  byte = 3u8;\n",
        "  float = 4.0;\n",
        "  flag = true;\n",
        "  signed = 5;\n",
        "  return signed;\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty());
    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let assignments: Vec<_> = definition
        .body
        .statements
        .iter()
        .filter_map(|statement| match statement {
            ResolvedStatement::PrimitiveLocalAssignment(assignment) => Some(assignment),
            _ => None,
        })
        .collect();
    assert_eq!(
        assignments
            .iter()
            .map(|assignment| assignment.destination.index())
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 0]
    );
    assert!(matches!(
        assignments[1].source,
        ResolvedExpression::NumericLiteral(ResolvedNumericLiteralExpr {
            kind: NumericLiteralKind::U64,
            ..
        })
    ));
}

#[test]
fn assignments_follow_nested_shadowing_and_restore_the_outer_local() {
    let output = resolve_text(concat!(
        "fn main() -> i64 {\n",
        "  var value: i64 = 0;\n",
        "  value = 1;\n",
        "  { var value: i64 = 2; (value) = 3; }\n",
        "  value = 4;\n",
        "  return value;\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty());
    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let destination = |statement: &ResolvedStatement| {
        let ResolvedStatement::PrimitiveLocalAssignment(assignment) = statement else {
            panic!("expected primitive local assignment");
        };
        assignment.destination
    };
    assert_eq!(
        destination(&definition.body.statements[1]),
        LocalId::new(FunctionId::new(0), 0)
    );
    let ResolvedStatement::Block(block) = &definition.body.statements[2] else {
        panic!("expected nested block");
    };
    assert_eq!(
        destination(&block.statements[1]),
        LocalId::new(FunctionId::new(0), 1)
    );
    assert_eq!(
        destination(&definition.body.statements[3]),
        LocalId::new(FunctionId::new(0), 0)
    );
}

#[test]
fn rejects_primitive_parameter_rebinding_and_still_resolves_the_source() {
    let output = resolve_text(
        "fn update(value: i64) -> i64 { value = missing; return value; }\nfn main() -> i64 { return 0; }\n",
    );
    let diagnostics: Vec<_> = output.diagnostics.iter().collect();

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].code, INVALID_LOCAL_ASSIGNMENT);
    assert_eq!(diagnostics[0].labels.len(), 2);
    assert_eq!(diagnostics[1].code, UNKNOWN_NAME);
    assert!(dump_resolved(&output.program).contains("Binding f0:p0"));
}

#[test]
fn resolved_dump_uses_only_the_primitive_destination_identity() {
    let output =
        resolve_text("fn main() -> i64 { var value: i64 = 0; (value) = 42; return value; }\n");
    let dump = dump_resolved(&output.program);

    assert_eq!(dump, dump_resolved(&output.program));
    assert!(dump.contains("PrimitiveLocalAssignment f0:l0"));
    assert!(dump.contains("Equal @"));
    assert!(!dump.contains("PrimitiveLocalAssignment \"value\""));
}
