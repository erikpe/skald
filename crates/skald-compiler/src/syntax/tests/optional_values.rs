use super::*;

#[test]
fn parses_optional_types_with_individual_source_spans() {
    let (sources, output) = parse_text(
        "class Item { init() {} }\n\
         fn inspect(value: i64?, owner: shared ? Item, canonical: (shared Item)?) -> bool? { return none; }\n",
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let source = sources.get(output.ast.span.source_id()).unwrap();
    let function = function(&output.ast, 1);

    let TypeKind::Optional {
        payload,
        question_span,
        spelling: OptionalTypeSpelling::Postfix,
    } = &function.parameters[0].type_syntax.kind
    else {
        panic!("expected inline optional parameter");
    };
    assert!(matches!(payload.kind, TypeKind::I64));
    assert_eq!(source.slice(payload.span.range()).unwrap(), "i64");
    assert_eq!(source.slice(question_span.range()).unwrap(), "?");

    let TypeKind::Optional {
        payload,
        question_span,
        spelling: OptionalTypeSpelling::SharedShorthand,
    } = &function.parameters[1].type_syntax.kind
    else {
        panic!("expected optional shared-owner parameter");
    };
    let TypeKind::Shared {
        shared_span,
        target,
    } = &payload.kind
    else {
        panic!("expected shorthand payload to retain the shared type");
    };
    assert_eq!(source.slice(shared_span.range()).unwrap(), "shared");
    assert_eq!(source.slice(question_span.range()).unwrap(), "?");
    assert_eq!(source.slice(target.span.range()).unwrap(), "Item");

    let TypeKind::Optional {
        payload,
        spelling: OptionalTypeSpelling::Postfix,
        ..
    } = &function.parameters[2].type_syntax.kind
    else {
        panic!("expected canonical optional shared-owner parameter");
    };
    assert!(matches!(payload.kind, TypeKind::Grouped { .. }));
    assert!(matches!(
        function.return_type.kind,
        TypeKind::Optional {
            spelling: OptionalTypeSpelling::Postfix,
            ..
        }
    ));
    assert!(matches!(
        return_value(function),
        Expression::Absent(AbsentExpr { .. })
    ));
    let dump = dump_ast(&output.ast);
    assert!(dump.contains("Type Optional Postfix"));
    assert!(dump.contains("Type Optional SharedShorthand"));
    assert!(dump.contains("Type Grouped"));
}

#[test]
fn parses_the_compositional_type_precedence_matrix() {
    let (_, output) = parse_text(concat!(
        "class T { init() {} }\n",
        "fn inspect(a: T?[], b: T[]?, c: (T[])?, d: (shared T)?, ",
        "e: shared? T, f: (shared T)??, g: shared T?, h: shared? T?) -> unit {}\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let inspect = function(&output.ast, 1);

    assert!(matches!(
        inspect.parameters[0].type_syntax.kind,
        TypeKind::Array { .. }
    ));
    assert!(matches!(
        inspect.parameters[1].type_syntax.kind,
        TypeKind::Optional { .. }
    ));
    assert!(matches!(
        inspect.parameters[2].type_syntax.kind,
        TypeKind::Optional { .. }
    ));
    assert!(matches!(
        inspect.parameters[3].type_syntax.kind,
        TypeKind::Optional {
            spelling: OptionalTypeSpelling::Postfix,
            ..
        }
    ));
    assert!(matches!(
        inspect.parameters[4].type_syntax.kind,
        TypeKind::Optional {
            spelling: OptionalTypeSpelling::SharedShorthand,
            ..
        }
    ));

    let TypeKind::Optional { payload, .. } = &inspect.parameters[5].type_syntax.kind else {
        panic!("expected outer optional layer");
    };
    assert!(matches!(payload.kind, TypeKind::Optional { .. }));

    let TypeKind::Shared { target, .. } = &inspect.parameters[6].type_syntax.kind else {
        panic!("expected a shared box syntax node");
    };
    assert!(matches!(target.kind, TypeKind::Optional { .. }));

    let TypeKind::Optional { payload, .. } = &inspect.parameters[7].type_syntax.kind else {
        panic!("expected shorthand optional layer");
    };
    let TypeKind::Shared { target, .. } = &payload.kind else {
        panic!("expected shorthand to wrap a shared syntax node");
    };
    assert!(matches!(target.kind, TypeKind::Optional { .. }));
}

#[test]
fn parses_deep_grouping_and_hostile_but_valid_optional_trivia() {
    let (_, output) = parse_text(concat!(
        "class Thing { init() {} }\n",
        "fn inspect(\n",
        "  nested: ((((i64?)?)[])?)?,\n",
        "  owner: ((shared\n",
        "    ? Thing))??,\n",
        "  arrays: (((Thing[])?)[])?\n",
        ") -> unit {}\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_ast(&output.ast);
    assert_eq!(dump.matches("Type Optional").count(), 9, "{dump}");
    assert!(dump.contains("Type Optional SharedShorthand"), "{dump}");
    assert!(dump.contains("Type Array"), "{dump}");
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
fn semantically_deferred_optional_shapes_cross_the_syntax_boundary() {
    let (_, output) = parse_text(
        "class Thing { init() {} }\n\
         fn shapes(unit_value: unit?, object: Obj?, nested: Thing??, array: Thing[]?, \
                   boxed: shared Thing?, maybe_boxed: shared? Thing?) -> unit {}\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(function(&output.ast, 1).parameters.len(), 6);
}

#[test]
fn parses_optional_box_allocations_as_a_distinct_source_shape() {
    let (sources, output) = parse_text(
        "class Item { init() {} }\n\
         fn boxes() -> unit {\n\
           var absent: shared i64? = new i64?();\n\
           var nested: shared Item?? = new (Item?)?(none);\n\
           var array: shared i64[]? = new i64[]?(none);\n\
           var owner: shared (shared Item)? = new (shared Item)?(none);\n\
           var grouped: shared Item? = new (Item?)();\n\
         }\n",
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let source = sources.get(output.ast.span.source_id()).unwrap();
    let boxes = function(&output.ast, 1);

    let Statement::Local(absent) = &boxes.body.statements[0] else {
        panic!("expected local declaration");
    };
    let Expression::OptionalBoxAllocation(absent) = &absent.initializer else {
        panic!("expected optional-box allocation");
    };
    assert_eq!(source.slice(absent.new_span.range()).unwrap(), "new");
    assert_eq!(source.slice(absent.target.span.range()).unwrap(), "i64?");
    let OptionalBoxInitializer::Absent {
        left_paren_span,
        right_paren_span,
    } = absent.initializer
    else {
        panic!("expected absent box initializer");
    };
    assert_eq!(source.slice(left_paren_span.range()).unwrap(), "(");
    assert_eq!(source.slice(right_paren_span.range()).unwrap(), ")");

    for statement in &boxes.body.statements[1..4] {
        let Statement::Local(local) = statement else {
            panic!("expected local declaration");
        };
        let Expression::OptionalBoxAllocation(allocation) = &local.initializer else {
            panic!("expected optional-box allocation");
        };
        assert!(matches!(
            allocation.initializer,
            OptionalBoxInitializer::Value { .. }
        ));
    }

    let Statement::Local(grouped) = &boxes.body.statements[4] else {
        panic!("expected local declaration");
    };
    let Expression::OptionalBoxAllocation(grouped) = &grouped.initializer else {
        panic!("expected grouped optional-box allocation");
    };
    assert_eq!(
        source.slice(grouped.target.span.range()).unwrap(),
        "(Item?)"
    );
    assert!(matches!(
        grouped.initializer,
        OptionalBoxInitializer::Absent { .. }
    ));

    let dump = dump_ast(&output.ast);
    assert_eq!(dump.matches("OptionalBoxAllocation").count(), 5, "{dump}");
    assert!(dump.contains("Initializer"), "{dump}");
}

#[test]
fn optional_box_initializer_arity_recovers_at_the_closing_parenthesis() {
    let (sources, output) = parse_text(
        "fn broken() -> unit { new i64?(1, some(2)); var after: i64 = 0; }\n\
         fn recovered() -> i64 { return 0; }\n",
    );

    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == INVALID_OPTIONAL_BOX_INITIALIZER)
        .expect("expected the optional-box arity diagnostic");
    assert!(diagnostic.message.contains("at most one"));
    let source = sources.get(output.ast.span.source_id()).unwrap();
    assert_eq!(
        source.slice(diagnostic.labels[0].span.range()).unwrap(),
        ","
    );
    assert_eq!(function(&output.ast, 1).name.text, "recovered");
}

#[test]
fn optional_reference_and_missing_owner_target_recover() {
    for declaration in [
        "fn broken(ref? value: Thing) -> unit {}",
        "fn broken(value: shared?) -> unit {}",
        "fn broken(value: shared?? Thing) -> unit {}",
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

#[test]
fn excessive_optional_type_nesting_is_bounded_and_recovers() {
    let source = format!(
        "fn broken(value: i64{}) -> unit {{}}\n\
         fn recovered() -> i64 {{ return 0; }}",
        "?".repeat(MAX_SYNTAX_NESTING)
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
