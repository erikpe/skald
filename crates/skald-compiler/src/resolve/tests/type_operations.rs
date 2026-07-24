use super::*;
use crate::identity::FieldId;

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
    let cast = call.receiver.cast().expect("checked receiver");
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
fn produced_cast_receivers_retain_only_cast_relative_projections() {
    let output = resolve_text(
        "class Child { init() {} fn touch() -> unit {} }\n\
         class Leaf {\n\
           child: Child;\n\
           init() { self.child = Child(); }\n\
         }\n\
         fn inspect() -> unit { ((Leaf) Leaf()).child.touch(); }\n\
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
    let ResolvedObjectReceiver::CastRelative {
        cast,
        projections,
        class,
        ..
    } = &call.receiver
    else {
        panic!("expected a cast-relative receiver");
    };
    assert!(matches!(*cast.source, ResolvedExpression::Construct(_)));
    assert_eq!(
        projections,
        &[crate::object_path::ObjectProjection::Field(FieldId::new(
            ClassId::new(1),
            0
        ))]
    );
    assert_eq!(*class, ClassId::new(0));
    assert_eq!(call.receiver.root(), None);

    let dump = dump_resolved(&output.program);
    assert!(dump.contains("CastRelativeReceiver class c0"));
    assert!(dump.contains("FieldProjection c1:field0"));
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
