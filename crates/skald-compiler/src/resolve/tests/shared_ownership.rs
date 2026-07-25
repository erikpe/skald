use super::*;
use crate::resolve::ResolvedSharedTarget;

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

    assert!(
        !output.has_errors(),
        "resolved shared identities must cross into type checking"
    );

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

#[test]
fn resolves_explicit_star_and_arrow_to_typed_dereference_receivers() {
    let output = resolve_text(concat!(
        "interface Readable { fn read() -> i64; }\n",
        "class Leaf implements Readable {\n",
        "  value: i64;\n",
        "  init() { self.value = 1; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "}\n",
        "fn make() -> shared Leaf { return new Leaf(); }\n",
        "fn main() -> i64 {\n",
        "  var owner: shared Leaf = new Leaf();\n",
        "  var direct: i64 = owner->read();\n",
        "  var grouped: i64 = (*owner).read();\n",
        "  var produced: i64 = make()->read();\n",
        "  var matches: bool = *owner is Leaf;\n",
        "  return direct + grouped + produced;\n",
        "}\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);
    assert!(dump.contains("DereferenceReceiver class c0"));
    assert!(dump.contains("Dereference Star target class c0"));
    assert!(dump.contains("Dereference Arrow target class c0"));
    assert!(dump.contains("TypeTest target class c0"));
}

#[test]
fn rejects_dereference_of_non_shared_values_deterministically() {
    let output = resolve_text(concat!(
        "class Leaf { init() {} }\n",
        "fn done() -> unit {}\n",
        "fn inspect(ref borrowed: Leaf) -> i64 {\n",
        "  *borrowed;\n",
        "  return 0;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var number: i64 = 1;\n",
        "  var inline: Leaf = Leaf();\n",
        "  *number;\n",
        "  inline->missing;\n",
        "  *done();\n",
        "  return 0;\n",
        "}\n",
    ));
    let diagnostics: Vec<_> = output.diagnostics.iter().collect();
    assert_eq!(diagnostics.len(), 4);
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == INVALID_DEREFERENCE));
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.message == "dereference requires a shared owner"));
}
