use super::*;
use crate::resolve::{ResolvedSharedTarget, UNSUPPORTED_SHARED_OWNERSHIP};

#[test]
fn resolves_shared_targets_and_allocation_modes_to_stable_identities() {
    let output = resolve_text(concat!(
        "interface Drawable {}\n",
        "class Widget { init(value: i64) {} }\n",
        "fn produce(\n",
        "  concrete: shared Widget,\n",
        "  view: shared Drawable,\n",
        "  erased: shared Obj\n",
        ") -> shared Widget {\n",
        "  var ordinary: shared Widget = new Widget(1);\n",
        "  var copied: shared Widget = new Widget(copy ordinary);\n",
        "  return copied;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.has_errors(), "shared execution must remain gated");
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == UNSUPPORTED_SHARED_OWNERSHIP));

    let produce = output.program.declarations.get(FunctionId::new(0)).unwrap();
    assert_eq!(
        produce.parameters[0].type_syntax.kind,
        ResolvedTypeKind::Shared(ResolvedSharedTarget::Class(ClassId::new(0)))
    );
    assert_eq!(
        produce.parameters[1].type_syntax.kind,
        ResolvedTypeKind::Shared(ResolvedSharedTarget::Interface(
            crate::identity::InterfaceId::new(0)
        ))
    );
    assert_eq!(
        produce.parameters[2].type_syntax.kind,
        ResolvedTypeKind::Shared(ResolvedSharedTarget::Obj)
    );
    assert_eq!(
        produce.return_type.kind,
        ResolvedTypeKind::Shared(ResolvedSharedTarget::Class(ClassId::new(0)))
    );

    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    assert_eq!(
        definition.locals[0].type_syntax.kind,
        ResolvedTypeKind::Shared(ResolvedSharedTarget::Class(ClassId::new(0)))
    );
    let ResolvedExpression::Allocation(ordinary) =
        local_initializer(&definition.body.statements[0])
    else {
        panic!("expected ordinary allocation");
    };
    assert_eq!(ordinary.class, ClassId::new(0));
    let ResolvedConstructionMode::Initialize { arguments } = &ordinary.mode else {
        panic!("expected ordinary allocation mode");
    };
    assert_eq!(arguments.len(), 1);

    let ResolvedExpression::Allocation(copied) = local_initializer(&definition.body.statements[1])
    else {
        panic!("expected copy allocation");
    };
    assert_eq!(copied.class, ClassId::new(0));
    let ResolvedConstructionMode::Copy { copy_span, source } = &copied.mode else {
        panic!("expected copy allocation mode");
    };
    assert!(copy_span.range().start() < source.span().range().start());
    assert!(matches!(**source, ResolvedExpression::Binding(_)));

    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert!(dump.contains("Type Shared class c0"));
    assert!(dump.contains("Type Shared interface i0"));
    assert!(dump.contains("Type Shared Obj"));
    assert!(dump.contains("Allocate c0"));
    assert!(dump.contains("CopyAllocate c0"));
}

#[test]
fn rejects_non_concrete_unknown_and_non_constructible_allocation_targets() {
    let output = resolve_text(concat!(
        "interface Shape {}\n",
        "class Empty {}\n",
        "fn factory() -> i64 { return 0; }\n",
        "fn main() -> i64 {\n",
        "  new Obj();\n",
        "  new Shape();\n",
        "  new factory();\n",
        "  new Missing();\n",
        "  new Empty();\n",
        "  return 0;\n",
        "}\n",
    ));

    let diagnostics: Vec<_> = output.diagnostics.iter().collect();
    assert_eq!(diagnostics.len(), 5);
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_CONSTRUCTION_TARGET)
            .count(),
        4
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == UNKNOWN_NAME
            && diagnostic
                .message
                .contains("unknown allocation class `Missing`")
    }));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("`Obj`")));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("interface `Shape`")));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("function `factory`")));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("no ordinary initializer")));
}

#[test]
fn unknown_shared_targets_do_not_gain_placeholder_identities() {
    let output = resolve_text(concat!(
        "fn consume(value: shared Missing) -> i64 { return 0; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.has_errors());
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, UNKNOWN_TYPE);
    assert_eq!(diagnostic.message, "unknown shared target `Missing`");
    let consume = output.program.declarations.get(FunctionId::new(0)).unwrap();
    assert!(consume.parameters.is_empty());
}
