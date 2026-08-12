use super::*;
use crate::test_support::load_module_sources;

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

    let optional_kinds = declaration
        .parameters
        .iter()
        .map(|parameter| parameter.type_syntax.kind)
        .chain([declaration.return_type.kind])
        .collect::<Vec<_>>();
    assert_eq!(
        optional_kinds,
        (0..4)
            .map(|index| ResolvedTypeKind::Optional(crate::identity::OptionalTypeId::new(index)))
            .collect::<Vec<_>>()
    );
    assert_eq!(output.program.optional_types.len(), 4);
    assert_eq!(optional_payload(&output.program, 0), ResolvedTypeKind::I64);
    assert_eq!(
        optional_payload(&output.program, 1),
        ResolvedTypeKind::Class(ClassId::new(0))
    );
    assert_eq!(
        optional_payload(&output.program, 2),
        ResolvedTypeKind::Shared(ResolvedSharedTarget::Interface(
            crate::identity::InterfaceId::new(0)
        ))
    );
    assert_eq!(optional_payload(&output.program, 3), ResolvedTypeKind::Bool);

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
fn canonical_and_shorthand_optional_owners_share_existing_semantics() {
    let output = resolve_text(
        "interface View { fn read() -> i64; }\n\
         fn choose(shorthand: shared? View, canonical: (shared View)?) -> (shared View)? {\n\
           return shorthand;\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let declaration = output.program.declarations.get(FunctionId::new(0)).unwrap();

    let optional_id = |ty: &ResolvedType| match ty.kind {
        ResolvedTypeKind::Optional(optional) => optional,
        _ => panic!("expected an optional shared owner"),
    };
    let shorthand = optional_id(&declaration.parameters[0].type_syntax);
    let canonical = optional_id(&declaration.parameters[1].type_syntax);
    let result = optional_id(&declaration.return_type);
    assert_eq!(shorthand, canonical);
    assert_eq!(canonical, result);
    assert_eq!(output.program.optional_types.len(), 1);

    let checked = crate::typeck::type_check(&output.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    assert!(checked.hir.is_some());
}

#[test]
fn interns_recursive_optional_and_array_identities_bottom_up() {
    let output = resolve_text(
        "fn shapes(deep: i64?????, elements: i64?[], maybe_elements: i64?[]?) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let shapes = output.program.declarations.get(FunctionId::new(0)).unwrap();
    assert_eq!(output.program.optional_types.len(), 6);
    for index in 0..5 {
        let expected = if index == 0 {
            ResolvedTypeKind::I64
        } else {
            ResolvedTypeKind::Optional(crate::identity::OptionalTypeId::new(index - 1))
        };
        assert_eq!(optional_payload(&output.program, index), expected);
    }
    assert_eq!(
        shapes.parameters[0].type_syntax.kind,
        ResolvedTypeKind::Optional(crate::identity::OptionalTypeId::new(4))
    );
    assert_eq!(
        output
            .program
            .array_types
            .get(crate::identity::ArrayTypeId::new(0))
            .unwrap()
            .element
            .kind,
        ResolvedTypeKind::Optional(crate::identity::OptionalTypeId::new(0))
    );
    assert_eq!(
        optional_payload(&output.program, 5),
        ResolvedTypeKind::Array(crate::identity::ArrayTypeId::new(0))
    );
}

#[test]
fn optional_array_and_shared_array_spellings_keep_exact_compositional_identities() {
    let output = resolve_text(
        "fn shapes(elements: i64?[], maybe: i64[]?, nested: i64[][]?, owners: (shared i64[])[], canonical_owner: (shared i64[])?, shorthand_owner: shared? i64[]) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let shapes = output.program.declarations.get(FunctionId::new(0)).unwrap();
    let kinds = shapes
        .parameters
        .iter()
        .map(|parameter| parameter.type_syntax.kind)
        .collect::<Vec<_>>();

    assert_ne!(kinds[0], kinds[1]);
    assert_ne!(kinds[1], kinds[2]);
    assert_ne!(kinds[2], kinds[3]);
    assert_ne!(kinds[3], kinds[4]);
    assert_eq!(kinds[4], kinds[5]);

    let checked = crate::typeck::type_check(&output.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
}

#[test]
fn repeated_optional_spellings_share_identities_across_modules() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import model;\nimport helper;\nfn use(value: model::Item?) -> unit {}\nfn main() -> i64 { return 0; }\n",
            ),
            (
                "model.ska",
                "public class Item { init() {} }\npublic fn keep(value: Item?) -> Item? { return value; }\n",
            ),
            (
                "helper.ska",
                "import model;\npublic fn keep(value: model::Item?) -> model::Item? { return value; }\n",
            ),
        ],
    );
    let output = resolve_module_graph(&graph);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert_eq!(output.program.optional_types.len(), 1);
    assert_eq!(
        optional_payload(&output.program, 0),
        ResolvedTypeKind::Class(ClassId::new(0))
    );
}

#[test]
fn repeated_optional_box_views_share_deterministic_identities_across_modules() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import model;\nimport helper;\nfn use(value: shared model::Item?) -> unit {}\nfn main() -> i64 { return 0; }\n",
            ),
            (
                "model.ska",
                "public class Item { init() {} }\npublic fn keep(value: shared Item?) -> unit {}\n",
            ),
            (
                "helper.ska",
                "import model;\npublic fn keep(value: shared model::Item?) -> unit {}\n",
            ),
        ],
    );
    let output = resolve_module_graph(&graph);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert_eq!(output.program.optional_box_types.iter().count(), 1);
    let target = output.program.optional_box_types.iter().next().unwrap();
    assert_eq!(target.id, crate::identity::OptionalBoxTypeId::new(0));
    assert_eq!(target.optional_depth, 1);
    assert_eq!(
        target.object_leaf,
        Some(ResolvedObjectTarget::Class(ClassId::new(0)))
    );
}

#[test]
fn nested_optionals_and_optional_arrays_cross_type_checking() {
    let output = resolve_text(
        "class Thing { init() {} }\n\
         fn nested(value: Thing??) -> unit {}\n\
         fn optional_array(value: Thing[]?) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert_eq!(output.program.optional_types.len(), 3);
    assert_eq!(
        optional_payload(&output.program, 1),
        ResolvedTypeKind::Optional(crate::identity::OptionalTypeId::new(0))
    );
    assert!(matches!(
        optional_payload(&output.program, 2),
        ResolvedTypeKind::Array(_)
    ));

    let checked = crate::typeck::type_check(&output.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    assert!(checked.hir.is_some());
}

#[test]
fn shared_box_callable_positions_retain_canonical_identities_through_type_checking() {
    let output = resolve_text(
        "class Thing { init() {} }\n\
         fn box_value(value: shared Thing?, nested: shared Thing??) -> unit {}\n\
         fn maybe_box(value: shared? Thing?, canonical: (shared Thing?)?) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert_eq!(output.program.optional_box_types.iter().count(), 2);
    let exact = output
        .program
        .optional_box_types
        .get(crate::identity::OptionalBoxTypeId::new(0))
        .unwrap();
    assert_eq!(exact.optional_depth, 1);
    assert_eq!(
        exact.object_leaf,
        Some(ResolvedObjectTarget::Class(ClassId::new(0)))
    );
    let nested = output
        .program
        .optional_box_types
        .get(crate::identity::OptionalBoxTypeId::new(1))
        .unwrap();
    assert_eq!(nested.optional_depth, 2);

    let maybe = output.program.declarations.get(FunctionId::new(1)).unwrap();
    assert_eq!(
        maybe.parameters[0].type_syntax.kind, maybe.parameters[1].type_syntax.kind,
        "shared shorthand must retain the canonical outer optional identity"
    );

    let checked = crate::typeck::type_check(&output.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    assert!(checked.hir.is_some());
}

#[test]
fn resolves_optional_box_allocations_with_exact_targets_and_spans() {
    let output = resolve_text(
        "class Thing { init() {} }\n\
         fn boxes() -> unit {\n\
           var empty: shared i64? = new i64?();\n\
           var object: shared Thing? = new Thing?(Thing());\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();

    let ResolvedExpression::OptionalBoxAllocation(empty) =
        local_initializer(&definition.body.statements[0])
    else {
        panic!("expected optional-box allocation");
    };
    assert_eq!(
        empty.exact_optional,
        crate::identity::OptionalTypeId::new(0)
    );
    assert!(matches!(
        empty.initializer,
        ResolvedOptionalBoxInitializer::Absent { .. }
    ));

    let ResolvedExpression::OptionalBoxAllocation(object) =
        local_initializer(&definition.body.statements[1])
    else {
        panic!("expected optional-box allocation");
    };
    assert_eq!(
        object.exact_optional,
        crate::identity::OptionalTypeId::new(1)
    );
    assert!(matches!(
        object.initializer,
        ResolvedOptionalBoxInitializer::Value { .. }
    ));
    let dump = dump_resolved(&output.program);
    assert!(dump.contains("OptionalBoxTypes"), "{dump}");
    assert!(
        dump.contains("OptionalBoxAllocate exact o0 target box0"),
        "{dump}"
    );
    assert!(
        dump.contains("OptionalBoxAllocate exact o1 target box1"),
        "{dump}"
    );
}

#[test]
fn rejects_non_concrete_object_box_allocation_but_retains_static_views() {
    let output = resolve_text(
        "interface View { fn read() -> i64; }\n\
         fn views(owner: shared View?, any: shared Obj?) -> unit {}\n\
         fn broken() -> unit { new View?(); new Obj?(); }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(output.has_errors());
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_CONSTRUCTION_TARGET)
            .count(),
        2
    );
    assert!(output.program.optional_box_types.iter().any(|target| {
        target.object_leaf
            == Some(ResolvedObjectTarget::Interface(
                crate::identity::InterfaceId::new(0),
            ))
    }));
    assert!(output
        .program
        .optional_box_types
        .iter()
        .any(|target| target.object_leaf == Some(ResolvedObjectTarget::Obj)));
}

#[test]
fn interface_and_obj_box_views_do_not_create_inline_optional_identities() {
    let output = resolve_text(
        "interface View { fn read() -> i64; }\n\
         fn views(owner: shared View?, any: shared Obj?) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert!(output.program.optional_types.is_empty());
    assert_eq!(output.program.optional_box_types.iter().count(), 2);
    assert!(output
        .program
        .optional_box_types
        .iter()
        .all(|target| target.optional.is_none()));
}

#[test]
fn resolved_dump_uses_canonical_optional_spellings_and_explicit_nodes() {
    let output = resolve_text(
        "fn inspect(value: i64?) -> bool {\n\
           var empty: i64? = none;\n\
           var owner: (shared Obj)? = none;\n\
           owner = none;\n\
           value!;\n\
           return value is none;\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);

    assert!(dump.contains("OptionalType o0 payload i64"));
    assert!(dump.contains("Type Optional o0 i64?"));
    assert!(dump.contains("type (shared Obj)?"));
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

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let checked = crate::typeck::type_check(&output.program);
    assert!(checked.has_errors());
    assert!(checked.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == crate::typeck::INVALID_OPTIONAL_TYPE
            && diagnostic.message.contains("interfaces")
    }));
    assert!(output.program.entry_function.is_some());
}

fn optional_payload(program: &ResolvedProgram, index: usize) -> ResolvedTypeKind {
    program
        .optional_types
        .get(crate::identity::OptionalTypeId::new(index))
        .unwrap()
        .payload
        .kind
}

#[test]
fn primitive_optional_assignment_shape_reaches_typed_hir() {
    let output = resolve_text(
        "fn update() -> unit {\n\
           var value: i64? = none;\n\
           value = none;\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let checked = crate::typeck::type_check(&output.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    assert!(checked.hir.is_some());
}
