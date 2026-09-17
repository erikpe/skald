use super::*;
use crate::backend::effects::{Effect, Effects, MemoryRegion};
use crate::backend::plan::{
    test_fixtures::runtime_declarations, ArtifactDeclaration, ArtifactId, Convention,
    RuntimeService, SignatureFact,
};

pub(super) fn runtime_call<'p>(
    service: RuntimeService,
    signature: crate::backend::plan::SignatureId,
    values: &[ValueHandle<'p>],
    attribution: CallAttribution,
) -> Call<ValueHandle<'p>> {
    Call {
        target: CallTarget::Direct(ArtifactId::Runtime(service)),
        signature,
        arguments: values
            .iter()
            .enumerate()
            .map(|(index, value)| CallArgument {
                role: ComponentRole::RuntimeParameter(index),
                value: *value,
            })
            .collect(),
        attribution,
    }
}

#[test]
fn runtime_catalog_checks_all_shapes_and_rejects_role_type_and_return_drift() {
    let mut supplied = facts();
    let signatures = runtime_declarations(&mut supplied);
    CheckedPlan::check(supplied.clone()).unwrap();
    for (service, signature) in signatures {
        let mut invalid = supplied.clone();
        let fact = &mut invalid.signatures[signature.index()];
        fact.results.clear();
        fact.returns = if fact.returns == ReturnShape::Unit {
            ReturnShape::Never
        } else {
            ReturnShape::Unit
        };
        assert!(
            CheckedPlan::check(invalid).is_err(),
            "wrong return for {service:?}"
        );
        if !supplied.signatures[signature.index()].inputs.is_empty() {
            let mut invalid = supplied.clone();
            invalid.signatures[signature.index()].inputs[0].role =
                ComponentRole::RuntimeParameter(42);
            assert_eq!(
                CheckedPlan::check(invalid).err(),
                Some(PlanError::InvalidSignature)
            );
            let mut invalid = supplied.clone();
            invalid.signatures[signature.index()].inputs[0].ty = ScalarType::Bool;
            assert_eq!(
                CheckedPlan::check(invalid).err(),
                Some(PlanError::InvalidSignature)
            );
        }
    }
}

#[test]
fn aggregate_and_receiver_alias_components_use_exact_logical_roles_without_scalar_results() {
    let mut supplied = facts();
    let layout = supplied
        .add_layout(LayoutFact {
            size: 0,
            alignment: 1,
            disposition: LayoutDisposition::Addressable,
        })
        .unwrap();
    let roles = [
        ComponentRole::ReceiverStatic,
        ComponentRole::ReceiverComplete,
        ComponentRole::ReceiverMetadata,
        ComponentRole::AliasAddress(3),
        ComponentRole::AliasComplete(3),
        ComponentRole::AliasMetadata(3),
        ComponentRole::ResultDestination(layout),
    ];
    supplied.signatures[0] = SignatureFact {
        convention: Convention::Language,
        inputs: roles
            .iter()
            .map(|role| Component {
                ty: ScalarType::DataAddress,
                role: *role,
            })
            .collect(),
        results: vec![],
        returns: ReturnShape::Aggregate(layout),
    };
    let signature = supplied.callables[0].signature;
    let plan = CheckedPlan::check(supplied).unwrap();
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    let inputs = builder.inputs().collect::<Vec<_>>();
    let call = Call {
        target: CallTarget::Direct(ArtifactId::Callable(source(1))),
        signature,
        arguments: roles
            .iter()
            .zip(&inputs)
            .map(|(role, value)| CallArgument {
                role: *role,
                value: *value,
            })
            .collect(),
        attribution: CallAttribution::ProcessBoundary,
    };
    let mut bad = call.clone();
    bad.arguments.swap(0, 1);
    assert_eq!(
        err(builder.append(entry, Operation::Call(bad))),
        BuildError::InvalidCall
    );
    assert!(builder
        .append(entry, Operation::Call(call))
        .unwrap()
        .is_empty());
    builder
        .terminate(entry, Terminator::Return(vec![]))
        .unwrap();
    let draft = builder.finish();
    assert_eq!(draft.inputs().len(), 7);
    let instruction = &draft.block(entry).unwrap().instructions[0];
    assert!(instruction.results.is_empty());
    assert!(instruction.effects.contains(Effect::Call));
    assert!(instruction
        .effects
        .contains(Effect::Read(MemoryRegion::Unknown)));
}

#[test]
fn indirect_targets_and_saved_arguments_are_values_secured_before_later_effects() {
    let mut supplied = facts();
    let signatures = runtime_declarations(&mut supplied);
    let signature = supplied.callables[0].signature;
    let plan = CheckedPlan::check(supplied).unwrap();
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    let target = constant(
        &mut builder,
        entry,
        Constant::Null(ScalarType::CodeAddress(signature)),
    );
    let allocation_size = constant(&mut builder, entry, Constant::U64(8));
    let allocate = runtime_call(
        RuntimeService::Allocate,
        signatures[&RuntimeService::Allocate],
        &[allocation_size],
        CallAttribution::ProcessBoundary,
    );
    let allocation = builder.append(entry, Operation::Call(allocate)).unwrap()[0];
    let call = Call {
        target: CallTarget::Indirect(target),
        signature,
        arguments: vec![],
        attribution: CallAttribution::ProcessBoundary,
    };
    assert_eq!(
        err(builder.append_with_effects(
            entry,
            Operation::Call(call.clone()),
            Effects::new([Effect::Call])
        )),
        BuildError::NarrowedEffects
    );
    builder.append(entry, Operation::Call(call)).unwrap();
    let free = runtime_call(
        RuntimeService::Free,
        signatures[&RuntimeService::Free],
        &[allocation],
        CallAttribution::HardDefectOnly,
    );
    builder.append(entry, Operation::Call(free)).unwrap();
    let wrong = Call {
        target: CallTarget::Indirect(target),
        signature: signatures[&RuntimeService::Free],
        arguments: vec![CallArgument {
            role: ComponentRole::RuntimeParameter(0),
            value: allocation,
        }],
        attribution: CallAttribution::ProcessBoundary,
    };
    assert_eq!(
        err(builder.append(entry, Operation::Call(wrong))),
        BuildError::InvalidCall
    );
    let draft = builder.finish();
    match &draft.block(entry).unwrap().instructions[3].operation {
        Operation::Call(Call {
            target: CallTarget::Indirect(actual),
            ..
        }) => assert_eq!(*actual, target.id()),
        _ => panic!("expected secured target"),
    }
    match &draft.block(entry).unwrap().instructions[4].operation {
        Operation::Call(call) => assert_eq!(call.arguments[0].value, allocation.id()),
        _ => panic!("expected free"),
    }
}

#[test]
fn effects_follow_known_objects_and_statics_but_loaded_and_alias_addresses_are_unknown() {
    use crate::backend::plan::DataKey;
    use crate::identity::StaticFieldId;
    let mut supplied = facts();
    let field = StaticFieldId::new(crate::identity::ClassId::new(0), 0);
    let layout = supplied
        .add_layout(LayoutFact {
            size: 16,
            alignment: 8,
            disposition: LayoutDisposition::Addressable,
        })
        .unwrap();
    supplied.artifacts.push(ArtifactDeclaration {
        key: ArtifactId::Data(DataKey::Static(field)),
        signature: None,
        layout: Some(layout),
    });
    supplied.signatures[0].inputs = vec![Component {
        ty: ScalarType::DataAddress,
        role: ComponentRole::AliasAddress(0),
    }];
    let plan = CheckedPlan::check(supplied).unwrap();
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    let alias = builder.inputs().next().unwrap();
    let object = builder
        .declare_object(object(
            LayoutDisposition::Addressable,
            16,
            8,
            ObjectRole::SemanticStorage,
            LifetimeDisposition::WholeCallable,
        ))
        .unwrap();
    let base = builder
        .append(entry, Operation::ObjectAddress(object))
        .unwrap()[0];
    let offset = constant(&mut builder, entry, Constant::U64(8));
    let address = builder
        .append(entry, Operation::ByteOffset { base, offset })
        .unwrap()[0];
    let pointer = builder
        .append(
            entry,
            Operation::Load {
                address,
                representation: MemoryRepresentation {
                    scalar: ScalarType::DataAddress,
                    bytes: 8,
                    alignment: 8,
                },
            },
        )
        .unwrap()[0];
    let value = constant(&mut builder, entry, Constant::U64(0));
    let representation = MemoryRepresentation {
        scalar: ScalarType::U64,
        bytes: 8,
        alignment: 8,
    };
    assert_eq!(
        err(builder.append_with_effects(
            entry,
            Operation::Store {
                address: pointer,
                value,
                representation
            },
            Effects::new([Effect::Write(MemoryRegion::Object(object.id()))])
        )),
        BuildError::NarrowedEffects
    );
    assert_eq!(
        err(builder.append_with_effects(
            entry,
            Operation::Store {
                address: base,
                value,
                representation
            },
            Effects::default()
        )),
        BuildError::NarrowedEffects
    );
    builder
        .append_with_effects(
            entry,
            Operation::Store {
                address: base,
                value,
                representation,
            },
            Effects::new([Effect::Write(MemoryRegion::Unknown)]),
        )
        .unwrap();
    builder
        .append(
            entry,
            Operation::Store {
                address: alias,
                value,
                representation,
            },
        )
        .unwrap();
    let static_address = builder
        .append(
            entry,
            Operation::SymbolAddress {
                symbol: ArtifactId::Data(DataKey::Static(field)),
                ty: ScalarType::DataAddress,
            },
        )
        .unwrap()[0];
    builder
        .append(
            entry,
            Operation::Load {
                address: static_address,
                representation,
            },
        )
        .unwrap();
    let draft = builder.finish();
    assert_eq!(
        draft.value(address).unwrap().provenance,
        AddressProvenance::Object {
            object: object.id(),
            offset: 8
        }
    );
    assert_eq!(
        draft.value(pointer).unwrap().provenance,
        AddressProvenance::Unknown
    );
    let effects = draft
        .block(entry)
        .unwrap()
        .instructions
        .iter()
        .map(|instruction| &instruction.effects)
        .collect::<Vec<_>>();
    assert!(effects[3].contains(Effect::Read(MemoryRegion::Object(object.id()))));
    assert!(effects[6].contains(Effect::Write(MemoryRegion::Unknown)));
    assert!(effects[8].contains(Effect::Read(MemoryRegion::Static(field))));
}
