use super::*;
use crate::identity::NarrowedAliasId;

#[test]
fn resolves_object_cast_targets_modes_and_member_receivers() {
    let output = resolve_text(
        "class Leaf { init() {} fn touch() -> unit {} }\n\
         fn inspect(ref value: Obj) -> unit {\n\
           ((Leaf) value).touch();\n\
           (shared Leaf) value;\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedStatement::Expression(call) = &definition.body.statements[0] else {
        panic!("expected method call");
    };
    let ResolvedExpression::MethodCall(call) = &call.expression else {
        panic!("expected resolved method call");
    };
    let cast = call.receiver.cast.as_ref().expect("checked receiver");
    assert_eq!(cast.target.kind, ResolvedTypeKind::Class(ClassId::new(0)));
    assert_eq!(
        cast.target_mode,
        crate::resolve::ResolvedObjectCastTargetMode::Plain
    );

    let ResolvedStatement::Expression(shared) = &definition.body.statements[1] else {
        panic!("expected shared cast expression");
    };
    let ResolvedExpression::ObjectCast(shared) = &shared.expression else {
        panic!("expected resolved shared cast");
    };
    assert!(matches!(
        shared.target_mode,
        crate::resolve::ResolvedObjectCastTargetMode::Shared { .. }
    ));
}

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

#[test]
fn cast_places_cannot_replace_whole_objects() {
    let output = resolve_text(
        "class Leaf { init() {} }\n\
         fn invalid(mut ref value: Obj, ref leaf: Leaf) -> unit {\n\
           ((Leaf) value) = leaf;\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.has_errors());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_MEMBER_SELECTION && diagnostic.message.contains("object place")
    }));
}
