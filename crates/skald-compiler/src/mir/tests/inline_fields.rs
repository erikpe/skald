use super::*;
use crate::{passes::run_mir_pipeline, test_support::INLINE_FIELD_SOURCE};

#[test]
fn lowers_deep_source_places_without_class_values_and_preserves_them_through_passes() {
    let program = lower_text(INLINE_FIELD_SOURCE);
    verify_mir(&program).unwrap();

    let root = program.class(ClassId::new(0)).unwrap();
    let leaf = program.class(ClassId::new(2)).unwrap();
    let branch = program.class(ClassId::new(3)).unwrap();
    let left = FieldId::new(root.id, 1);
    let right = FieldId::new(root.id, 2);
    let branch_leaf = FieldId::new(branch.id, 2);
    let leaf_value = FieldId::new(leaf.id, 1);

    let initializer = program
        .member_definition(root.initializers[0].id.into())
        .unwrap();
    let constructions: Vec<_> = initializer
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Initialize(initialize) => Some(initialize),
            _ => None,
        })
        .collect();
    assert_eq!(constructions.len(), 2);
    assert_eq!(
        constructions[0].destination.base,
        MirPlaceBase::Storage(initializer.receiver)
    );
    assert_eq!(
        constructions[0].destination.projections,
        [MirPlaceProjection::Field(right)]
    );
    assert_eq!(
        constructions[1].destination.projections,
        [MirPlaceProjection::Field(left)]
    );

    let forward = program.definitions.get(FunctionId::new(2)).unwrap();
    let deep_aliases: Vec<_> = forward
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Call(call) => match &call.arguments[0] {
                MirArgument::Place(place) => Some(place),
                MirArgument::Value(_) => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(deep_aliases.len(), 2);
    assert!(deep_aliases
        .iter()
        .all(|place| matches!(place.base, MirPlaceBase::AliasParameter(_))));
    assert_eq!(
        deep_aliases[0].projections,
        [
            MirPlaceProjection::Field(left),
            MirPlaceProjection::Field(branch_leaf),
        ]
    );

    let adjust = program
        .member_definition(root.methods[1].id.into())
        .unwrap();
    let deep_store = adjust
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Store(store) if store.destination.projections.len() == 3 => Some(store),
            _ => None,
        })
        .expect("mutable receiver method should contain one deep scalar store");
    assert_eq!(
        deep_store.destination.projections,
        [
            MirPlaceProjection::Field(left),
            MirPlaceProjection::Field(branch_leaf),
            MirPlaceProjection::Field(leaf_value),
        ]
    );
    assert!(program
        .executable_definitions()
        .flat_map(|definition| definition.values())
        .all(|value| value.ty.is_scalar_value()));

    let dump = dump_mir(&program);
    assert!(
        dump.contains("initialize c0:init0:s0.field(c0:field2) with c3:init0(value(c0:init0:v0))")
    );
    assert!(dump.contains("store c0:method1:s0.field(c0:field1).field(c3:field2).field(c2:field1)"));
    assert!(dump.contains("place(indirect(f2:s0).field(c0:field1).field(c3:field2))"));

    let expected = program.clone();
    assert_eq!(run_mir_pipeline(program).unwrap(), expected);
}

#[test]
fn deep_source_places_are_checked_by_the_shared_verifier_path() {
    let mut program = lower_text(INLINE_FIELD_SOURCE);
    let root = program.class(ClassId::new(0)).unwrap();
    let method = root.methods[1].id;
    program.classes.entries_mut_for_test()[0].methods[1].receiver_access =
        MirReceiverAccess::ReadOnly;

    let errors = verify_mir(&program).unwrap_err().to_string();
    assert!(errors.contains(&format!("{method}")));
    assert!(errors.contains("store destination requires mutable access"));
    assert!(errors.contains("mutable access"));
}
