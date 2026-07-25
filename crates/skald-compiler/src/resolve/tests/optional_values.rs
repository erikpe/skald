use super::*;

#[test]
fn resolves_flat_inline_and_shared_optional_identities() {
    let output = resolve_text(
        "class Item { init() {} }\n\
         interface View { fn read() -> i64; }\n\
         fn inspect(value: i64?, item: Item?, owner: shared? View) -> bool? {\n\
           var empty: i64? = none;\n\
           value!;\n\
           return value is some;\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let declaration = output.program.declarations.get(FunctionId::new(0)).unwrap();

    assert!(matches!(
        declaration.parameters[0].type_syntax.kind,
        ResolvedTypeKind::Optional {
            payload: ResolvedOptionalPayload::I64,
            ..
        }
    ));
    assert!(matches!(
        declaration.parameters[1].type_syntax.kind,
        ResolvedTypeKind::Optional {
            payload: ResolvedOptionalPayload::Class(class),
            ..
        } if class == ClassId::new(0)
    ));
    assert!(matches!(
        declaration.parameters[2].type_syntax.kind,
        ResolvedTypeKind::OptionalShared {
            target: ResolvedSharedTarget::Interface(interface),
            ..
        } if interface == crate::identity::InterfaceId::new(0)
    ));
    assert!(matches!(
        declaration.return_type.kind,
        ResolvedTypeKind::Optional {
            payload: ResolvedOptionalPayload::Bool,
            ..
        }
    ));

    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    assert!(matches!(
        local_initializer(&definition.body.statements[0]),
        ResolvedExpression::Absent(_)
    ));
    let ResolvedStatement::Expression(statement) = &definition.body.statements[1] else {
        panic!("expected unwrap expression statement");
    };
    assert!(matches!(
        statement.expression,
        ResolvedExpression::Unwrap(_)
    ));
    assert!(matches!(
        return_value(&definition.body.statements[2]),
        ResolvedExpression::PresenceTest(ResolvedPresenceTestExpr {
            kind: ResolvedPresenceTestKind::Some,
            ..
        })
    ));
}

#[test]
fn resolved_dump_uses_canonical_optional_spellings_and_explicit_nodes() {
    let output = resolve_text(
        "fn inspect(value: i64?) -> bool {\n\
           var empty: i64? = none;\n\
           value!;\n\
           return value is none;\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);

    assert!(dump.contains("Type Optional i64"));
    assert!(dump.contains("Question"));
    assert!(dump.contains("Absent"));
    assert!(dump.contains("Unwrap"));
    assert!(dump.contains("PresenceTest None"));
}

#[test]
fn rejects_interface_inline_optionals_but_recovers_other_declarations() {
    let output = resolve_text(
        "interface View { fn read() -> i64; }\n\
         fn broken(value: View?) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.has_errors());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_OPTIONAL_TYPE && diagnostic.message.contains("interface `View`")
    }));
    assert!(output.program.entry_function.is_some());
}

#[test]
fn optional_assignment_shape_reaches_the_type_check_gate() {
    let output = resolve_text(
        "fn update() -> unit {\n\
           var value: i64? = none;\n\
           value = none;\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let checked = crate::typeck::type_check(&output.program);
    assert_eq!(checked.diagnostics.len(), 1);
    assert_eq!(
        checked.diagnostics.iter().next().unwrap().code,
        crate::typeck::OPTIONAL_VALUES_NOT_IMPLEMENTED
    );
}
