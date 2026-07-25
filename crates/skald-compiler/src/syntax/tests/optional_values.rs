use super::*;

#[test]
fn parses_optional_types_with_individual_source_spans() {
    let (sources, output) = parse_text(
        "class Item { init() {} }\n\
         fn inspect(value: i64?, owner: shared ? Item) -> bool? { return none; }\n",
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let source = sources.get(output.ast.span.source_id()).unwrap();
    let function = function(&output.ast, 1);

    let TypeKind::Optional {
        payload: OptionalPayloadKind::I64,
        payload_span,
        question_span,
    } = &function.parameters[0].type_syntax.kind
    else {
        panic!("expected inline optional parameter");
    };
    assert_eq!(source.slice(payload_span.range()).unwrap(), "i64");
    assert_eq!(source.slice(question_span.range()).unwrap(), "?");

    let TypeKind::OptionalShared {
        shared_span,
        question_span,
        target,
    } = &function.parameters[1].type_syntax.kind
    else {
        panic!("expected optional shared-owner parameter");
    };
    assert_eq!(source.slice(shared_span.range()).unwrap(), "shared");
    assert_eq!(source.slice(question_span.range()).unwrap(), "?");
    assert_eq!(source.slice(target.span.range()).unwrap(), "Item");
    assert!(matches!(
        function.return_type.kind,
        TypeKind::Optional {
            payload: OptionalPayloadKind::Bool,
            ..
        }
    ));
    assert!(matches!(
        return_value(function),
        Expression::Absent(AbsentExpr { .. })
    ));
    let dump = dump_ast(&output.ast);
    assert!(dump.contains("Type Optional i64?"));
    assert!(dump.contains("Type OptionalShared shared? Item"));
}

#[test]
fn none_is_reserved_while_some_remains_an_ordinary_name_outside_presence_tests() {
    let (_, valid) = parse_text(
        "fn some() -> i64 { return 1; }\n\
         fn main() -> i64 { return some(); }\n",
    );
    assert!(valid.diagnostics.is_empty(), "{:?}", valid.diagnostics);

    let (_, invalid) = parse_text("fn none() -> i64 { return 0; }");
    assert!(invalid.has_errors());
}

#[test]
fn parses_presence_tests_and_unwrap_in_the_postfix_chain() {
    let (_, output) = parse_text(
        "fn inspect(value: i64?) -> bool {\n\
           value!.member(1)!.next;\n\
           if (value is some) { return value! is none; }\n\
           return value is none;\n\
         }\n",
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let function = function(&output.ast, 0);

    let Statement::Expression(statement) = &function.body.statements[0] else {
        panic!("expected postfix expression statement");
    };
    let Expression::MemberAccess(member) = &statement.expression else {
        panic!("expected final member access");
    };
    assert!(matches!(member.receiver.as_ref(), Expression::Unwrap(_)));

    let Statement::Conditional(conditional) = &function.body.statements[1] else {
        panic!("expected conditional");
    };
    assert!(matches!(
        &conditional.if_arm.condition,
        Expression::PresenceTest(PresenceTestExpr {
            kind: PresenceTestKind::Some,
            ..
        })
    ));
    assert!(matches!(
        return_value(function),
        Expression::PresenceTest(PresenceTestExpr {
            kind: PresenceTestKind::None,
            ..
        })
    ));
}

#[test]
fn rejected_optional_type_forms_recover_to_later_declarations() {
    for invalid in [
        "unit?",
        "Obj?",
        "Thing??",
        "shared Thing?",
        "shared? Thing?",
    ] {
        let source = format!(
            "fn broken(value: {invalid}) -> unit {{}}\n\
             fn main() -> i64 {{ return 0; }}\n"
        );
        let (_, output) = parse_text(&source);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == INVALID_OPTIONAL_TYPE),
            "missing optional diagnostic for {invalid}: {:?}",
            output.diagnostics
        );
        assert_eq!(function(&output.ast, 0).name.text, "main");
    }
}

#[test]
fn optional_reference_and_missing_owner_target_recover() {
    for declaration in [
        "fn broken(ref? value: Thing) -> unit {}",
        "fn broken(value: shared?) -> unit {}",
    ] {
        let source = format!("{declaration}\nfn main() -> i64 {{ return 0; }}");
        let (_, output) = parse_text(&source);
        assert!(output.has_errors());
        assert_eq!(function(&output.ast, 0).name.text, "main");
    }
}

#[test]
fn excessive_postfix_unwrap_nesting_is_bounded_and_recovers() {
    let source = format!(
        "fn broken(value: i64?) -> i64 {{ return value{}; }}\n\
         fn recovered() -> i64 {{ return 0; }}",
        "!".repeat(MAX_SYNTAX_NESTING)
    );
    let (_, output) = parse_text(&source);

    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == EXCESSIVE_NESTING)
            .count(),
        1
    );
    assert_eq!(function(&output.ast, 0).name.text, "recovered");
}
