use super::*;
use crate::identity::{ArrayTypeId, InterfaceId};

#[test]
fn interns_exact_recursive_array_identities_in_first_use_order() {
    let output = resolve_text(
        "class Item { init() {} }\n\
         interface View { fn read(values: Item[]) -> Item[][]; }\n\
         fn shapes(\n\
           first: Item[], second: Item[], nested: Item[][],\n\
           owner: shared Item[], maybe_owner: shared? Item[],\n\
           elements: (shared Item)[], maybe_elements: (shared? Item)[],\n\
           element_owner: shared (shared Item)[],\n\
           maybe_element_owner: shared? (shared? Item)[],\n\
           view_elements: View[], object_elements: Obj[], unit_elements: unit[]\n\
         ) -> Item[] { return first; }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let shapes = output.program.declarations.get(FunctionId::new(0)).unwrap();
    let kinds = shapes
        .parameters
        .iter()
        .map(|parameter| parameter.type_syntax.kind)
        .collect::<Vec<_>>();

    assert_eq!(kinds[0], ResolvedTypeKind::Array(ArrayTypeId::new(0)));
    assert_eq!(kinds[1], kinds[0]);
    assert_eq!(kinds[2], ResolvedTypeKind::Array(ArrayTypeId::new(1)));
    assert_eq!(
        kinds[3],
        ResolvedTypeKind::Shared(ResolvedSharedTarget::Array(ArrayTypeId::new(0)))
    );
    assert!(matches!(
        kinds[4],
        ResolvedTypeKind::OptionalShared {
            target: ResolvedSharedTarget::Array(id),
            ..
        } if id == ArrayTypeId::new(0)
    ));
    assert_eq!(kinds[5], ResolvedTypeKind::Array(ArrayTypeId::new(2)));
    assert_eq!(kinds[6], ResolvedTypeKind::Array(ArrayTypeId::new(3)));
    assert_eq!(
        kinds[7],
        ResolvedTypeKind::Shared(ResolvedSharedTarget::Array(ArrayTypeId::new(2)))
    );
    assert!(matches!(
        kinds[8],
        ResolvedTypeKind::OptionalShared {
            target: ResolvedSharedTarget::Array(id),
            ..
        } if id == ArrayTypeId::new(3)
    ));
    assert_eq!(kinds[9], ResolvedTypeKind::Array(ArrayTypeId::new(4)));
    assert_eq!(kinds[10], ResolvedTypeKind::Array(ArrayTypeId::new(5)));
    assert_eq!(kinds[11], ResolvedTypeKind::Array(ArrayTypeId::new(6)));
    assert_eq!(
        shapes.return_type.kind,
        ResolvedTypeKind::Array(ArrayTypeId::new(0))
    );

    let table = &output.program.array_types;
    assert_eq!(table.len(), 7);
    assert_eq!(
        table.get(ArrayTypeId::new(1)).unwrap().element.kind,
        ResolvedTypeKind::Array(ArrayTypeId::new(0))
    );
    assert_eq!(
        table.get(ArrayTypeId::new(2)).unwrap().element.kind,
        ResolvedTypeKind::Shared(ResolvedSharedTarget::Class(ClassId::new(0)))
    );
    assert!(matches!(
        table.get(ArrayTypeId::new(3)).unwrap().element.kind,
        ResolvedTypeKind::OptionalShared {
            target: ResolvedSharedTarget::Class(class),
            ..
        } if class == ClassId::new(0)
    ));
    assert_eq!(
        table.get(ArrayTypeId::new(4)).unwrap().element.kind,
        ResolvedTypeKind::Interface(InterfaceId::new(0))
    );
    assert_eq!(
        table.get(ArrayTypeId::new(5)).unwrap().element.kind,
        ResolvedTypeKind::Obj
    );
    assert_eq!(
        table.get(ArrayTypeId::new(6)).unwrap().element.kind,
        ResolvedTypeKind::Unit
    );
}

#[test]
fn resolves_array_types_in_fields_aliases_locals_results_and_constructors() {
    let output = resolve_text(
        "class Buffer {\n\
           values: i64[];\n\
           init(ref source: i64[]) { self.values = i64[](copy source); }\n\
           mut fn replace(mut ref target: i64[]) -> i64[] {\n\
             var local: i64[] = i64[](4u);\n\
             target = local;\n\
             return local;\n\
           }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert_eq!(output.program.array_types.len(), 1);

    let array = ResolvedTypeKind::Array(ArrayTypeId::new(0));
    let class = output.program.class(ClassId::new(0)).unwrap();
    assert_eq!(class.fields[0].type_syntax.kind, array);
    assert_eq!(class.initializers[0].parameters[0].type_syntax.kind, array);
    assert_eq!(class.methods[0].parameters[0].type_syntax.kind, array);
    assert_eq!(class.methods[0].return_type.kind, array);

    let definition = output
        .program
        .member_definition(class.methods[0].id.into())
        .unwrap();
    assert_eq!(definition.locals[0].type_syntax.kind, array);
    assert!(matches!(
        local_initializer(&definition.body.statements[0]),
        ResolvedExpression::ArrayConstruction(_)
    ));
}

#[test]
fn retains_structured_construction_and_projection_until_type_checking() {
    let output = resolve_text(
        "fn main() -> i64 {\n\
           var values: i64[] = i64[](4u);\n\
           var copied: i64[] = i64[](copy values[1:3]);\n\
           return values[-1];\n\
         }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();

    assert!(matches!(
        local_initializer(&definition.body.statements[0]),
        ResolvedExpression::ArrayConstruction(construction)
            if matches!(construction.arguments, ResolvedArrayConstructionArguments::Length { .. })
    ));
    assert!(matches!(
        local_initializer(&definition.body.statements[1]),
        ResolvedExpression::ArrayConstruction(construction)
            if matches!(
                &construction.arguments,
                ResolvedArrayConstructionArguments::Copy {
                    source,
                    ..
                } if matches!(**source, ResolvedExpression::ArrayProjection(_))
            )
    ));
    assert!(matches!(
        return_value(&definition.body.statements[2]),
        ResolvedExpression::ArrayProjection(projection)
            if matches!(projection.bounds, ResolvedArrayProjectionBounds::Index(_))
    ));
}

#[test]
fn resolved_dump_declares_canonical_nested_arrays_before_their_uses() {
    let output = resolve_text(
        "fn inspect(first: i64[][], second: i64[][], owner: shared i64[][]) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);

    assert_eq!(dump.matches("ArrayType a0").count(), 1);
    assert_eq!(dump.matches("ArrayType a1").count(), 1);
    assert!(dump.find("ArrayTypes").unwrap() < dump.find("\n  Declarations\n").unwrap());
    assert!(dump.contains("Type Array a1"));
    assert!(dump.contains("Type Shared array a1"));
}

#[test]
fn canonical_entries_retain_the_first_exact_element_source_span() {
    let text = "class Item { init() {} }\n\
                fn inspect(value: (shared Item)[][]) -> unit {}\n\
                fn main() -> i64 { return 0; }\n";
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("arrays.ska", text);
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    let parsed = syntax::parse(source, &lexed.tokens);
    let output = resolve(&parsed.ast);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let inner = output.program.array_types.get(ArrayTypeId::new(0)).unwrap();
    let outer = output.program.array_types.get(ArrayTypeId::new(1)).unwrap();
    assert_eq!(
        source.slice(inner.element.span.range()),
        Some("(shared Item)")
    );
    assert_eq!(
        source.slice(outer.element.span.range()),
        Some("(shared Item)[]")
    );
}

#[test]
fn resolved_array_projection_crosses_into_typed_hir() {
    let resolved =
        resolve_text("fn main() -> i64 { var values: i64[] = i64[](4u); return values[-1]; }");
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);

    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked
        .hir
        .expect("valid array projection must produce HIR");
    assert!(crate::hir::dump_hir(&hir).contains("ArrayElementPlace : i64"));
}

#[test]
fn dynamic_buffer_members_are_not_array_intrinsics() {
    let output = resolve_text(concat!(
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[](1u);\n",
        "  values.resize(2u);\n",
        "  var capacity: u64 = values.capacity();\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(output.has_errors());
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == INVALID_MEMBER_SELECTION));
}
