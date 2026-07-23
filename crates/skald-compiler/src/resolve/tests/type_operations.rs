use super::*;
use crate::identity::NarrowedAliasId;

#[test]
fn resolves_type_targets_and_scopes_narrowed_alias_identity_to_its_block() {
    let output = resolve_text(
        "class Sample { init() {} fn touch() -> unit {} }\n\
         fn inspect(ref value: Obj) -> bool {\n\
           narrow ref sample: Sample = value { sample.touch(); }\n\
           return value is Sample;\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let declaration = output.program.declarations.get(FunctionId::new(0)).unwrap();
    let definition = output.program.definitions.get(declaration.id).unwrap();
    assert_eq!(definition.narrowed_aliases.len(), 1);
    assert_eq!(
        definition.narrowed_aliases[0].id,
        NarrowedAliasId::new(declaration.id, 0)
    );
    let ResolvedStatement::Narrowing(narrowing) = &definition.body.statements[0] else {
        panic!("expected resolved narrowing");
    };
    let ResolvedStatement::Expression(call) = &narrowing.body.statements[0] else {
        panic!("expected narrowed alias call");
    };
    let ResolvedExpression::MethodCall(call) = &call.expression else {
        panic!("expected method call through narrowed alias");
    };
    assert_eq!(
        call.receiver.root,
        BindingId::NarrowedAlias(narrowing.binding)
    );
    let ResolvedStatement::Return(returned) = &definition.body.statements[1] else {
        panic!("expected type-test return");
    };
    let ResolvedExpression::TypeTest(test) = returned.value.as_ref().unwrap() else {
        panic!("expected resolved type test");
    };
    assert_eq!(test.target.kind, ResolvedTypeKind::Class(ClassId::new(0)));
}

#[test]
fn narrowed_alias_does_not_escape_its_trailing_block() {
    let output = resolve_text(
        "class Sample { init() {} fn touch() -> unit {} }\n\
         fn inspect(ref value: Obj) -> unit {\n\
           narrow ref sample: Sample = value {}\n\
           sample.touch();\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.has_errors());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == UNKNOWN_NAME));
}
