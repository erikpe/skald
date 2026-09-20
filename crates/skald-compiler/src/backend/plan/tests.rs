use super::{test_fixtures::*, *};
use crate::{
    backend::RuntimeTracePolicy,
    identity::{ClassId, StaticFieldId, VirtualFamilyId},
};

fn rejected(facts: PlanFacts, reason: PlanError) {
    assert_eq!(CheckedPlan::check(facts).err(), Some(reason));
}

fn address(role: ComponentRole) -> Component {
    Component {
        ty: ScalarType::DataAddress,
        role,
    }
}

fn resource_facts(disposition: StaticStorageDisposition) -> PlanFacts {
    let mut supplied = minimal_semantic_facts();
    let field = StaticFieldId::new(ClassId::new(0), 0);
    supplied.semantic.types.push(TypeLayoutBinding {
        ty: SemanticType::I64,
        layout: LayoutId::new(0),
    });
    supplied.artifacts.push(ArtifactDeclaration {
        key: ArtifactId::Data(DataKey::Static(field)),
        signature: None,
        layout: Some(LayoutId::new(0)),
    });
    supplied.resources.statics.push(StaticStorageFact {
        field,
        ty: SemanticType::I64,
        layout: LayoutId::new(0),
        disposition,
    });
    supplied.resources.data.push(DataFact {
        key: DataKey::Static(field),
        purpose: DataPurpose::StaticStorage(disposition),
        layout: LayoutId::new(0),
        initializers: vec![DataInitializerFact::Zero(8)],
        dependencies: Default::default(),
    });
    for index in 0..2 {
        let root = ArtifactRootFact {
            artifact: ArtifactId::Callable(source(index)),
            reason: ArtifactRootReason::CompleteDefinition,
        };
        supplied.resources.complete_roots.insert(root);
        supplied.resources.reachable_roots.insert(root);
    }
    let storage_root = ArtifactRootFact {
        artifact: ArtifactId::Data(DataKey::Static(field)),
        reason: ArtifactRootReason::StaticStorage(field),
    };
    supplied.resources.complete_roots.insert(storage_root);
    if disposition == StaticStorageDisposition::Active {
        supplied.active_statics.insert(field);
        supplied.resources.activation.push(StaticActivationFact {
            field,
            action: StaticActivationKind::ZeroDefault,
        });
        supplied.resources.shutdown.push(StaticShutdownFact {
            field,
            cleanup: StaticCleanupFact::None,
        });
        supplied.resources.reachable_roots.insert(storage_root);
    }
    supplied
}

#[test]
fn static_resources_distinguish_active_and_retained_inactive_storage() {
    for disposition in [
        StaticStorageDisposition::Active,
        StaticStorageDisposition::RetainedInactive,
    ] {
        let plan = CheckedPlan::check(resource_facts(disposition)).unwrap();
        let field = StaticFieldId::new(ClassId::new(0), 0);
        assert_eq!(
            plan.view().static_storage_disposition(field),
            Some(disposition)
        );
    }
}

#[test]
fn resource_catalog_rejects_static_promotion_missing_storage_and_nonzero_retention() {
    let field = StaticFieldId::new(ClassId::new(0), 0);

    let mut reachable = resource_facts(StaticStorageDisposition::RetainedInactive);
    reachable.artifact_policy = ArtifactPolicy::Reachable;
    rejected(reachable, PlanError::InvalidDomain);

    let mut promoted = resource_facts(StaticStorageDisposition::RetainedInactive);
    promoted.resources.activation.push(StaticActivationFact {
        field,
        action: StaticActivationKind::ZeroDefault,
    });
    rejected(promoted, PlanError::InvalidDomain);

    let mut missing = resource_facts(StaticStorageDisposition::RetainedInactive);
    missing.artifacts.clear();
    rejected(missing, PlanError::UnknownDeclaration);

    let mut nonzero = resource_facts(StaticStorageDisposition::RetainedInactive);
    nonzero.resources.data[0].initializers = vec![DataInitializerFact::Bytes(vec![1; 8])];
    rejected(nonzero, PlanError::InvalidDomain);
}

#[test]
fn resource_catalog_rejects_forged_relocations_roots_and_data_identity() {
    let field = StaticFieldId::new(ClassId::new(0), 0);
    let mut relocated = resource_facts(StaticStorageDisposition::Active);
    relocated.resources.data[0].initializers = vec![DataInitializerFact::Address {
        target: ArtifactId::Callable(source(0)),
        category: ArtifactCategory::Data,
        addend: 0,
    }];
    relocated.resources.data[0]
        .dependencies
        .insert(ArtifactId::Callable(source(0)));
    rejected(relocated, PlanError::ArtifactCategoryMismatch);

    let mut wrong_purpose = resource_facts(StaticStorageDisposition::Active);
    wrong_purpose.resources.data[0].purpose = DataPurpose::FailureMessage;
    rejected(wrong_purpose, PlanError::InvalidArtifact);

    let mut leaked_root = resource_facts(StaticStorageDisposition::RetainedInactive);
    leaked_root
        .resources
        .reachable_roots
        .insert(ArtifactRootFact {
            artifact: ArtifactId::Data(DataKey::Static(field)),
            reason: ArtifactRootReason::CompleteDefinition,
        });
    rejected(leaked_root, PlanError::InvalidDomain);

    let mut unknown_root = resource_facts(StaticStorageDisposition::Active);
    unknown_root
        .resources
        .complete_roots
        .insert(ArtifactRootFact {
            artifact: ArtifactId::Data(DataKey::Table(99)),
            reason: ArtifactRootReason::CompleteDefinition,
        });
    rejected(unknown_root, PlanError::UnknownDeclaration);

    let mut missing_root = resource_facts(StaticStorageDisposition::Active);
    missing_root
        .resources
        .complete_roots
        .retain(|root| root.artifact != ArtifactId::Data(DataKey::Static(field)));
    rejected(missing_root, PlanError::InvalidDomain);
}

#[test]
fn live_context_identity_is_checked_for_every_declaration_lookup() {
    let first = CheckedPlan::check(facts()).unwrap();
    let second = CheckedPlan::check(facts()).unwrap();
    let a = first.view();
    let b = second.view();
    assert_eq!(a.profile(), b.profile());
    assert_eq!(a.require_same_context(a), Ok(()));
    assert_eq!(a.require_same_context(b), Err(PlanError::WrongContext));
    assert_eq!(
        a.layout(b.layout_id(0).unwrap()),
        Err(PlanError::WrongContext)
    );
    assert_eq!(
        a.signature(b.signature_id(0).unwrap()),
        Err(PlanError::WrongContext)
    );
    assert_eq!(
        a.artifact(
            b.artifact_id(ArtifactId::Callable(source(0))).unwrap(),
            ArtifactCategory::Callable
        ),
        Err(PlanError::WrongContext)
    );
    assert_eq!(
        a.callable(source(0))
            .unwrap()
            .require_same_owner(b.callable(source(0)).unwrap()),
        Err(PlanError::WrongContext)
    );
    let id: DeclarationId<'_, LayoutId> = a.layout_id(0).unwrap();
    assert_eq!(a.layout(id).unwrap().size, 8);
    assert_eq!(
        a.callable(source(0)).unwrap().signature().unwrap().returns,
        ReturnShape::Unit
    );
    assert_eq!(
        a.callable(source(0)).unwrap().context().artifact_policy(),
        ArtifactPolicy::Complete
    );
    assert_eq!(a.runtime_trace(), RuntimeTracePolicy::Omitted);
}

#[test]
fn profile_and_capability_mismatches_are_rejected_separately() {
    let first = CheckedPlan::check(facts()).unwrap();
    let mut other = facts();
    other.profile.architecture = Architecture::Aarch64;
    other.profile.abi = Abi::Aapcs64;
    let second = CheckedPlan::check(other).unwrap();
    assert_eq!(
        first.view().require_same_context(second.view()),
        Err(PlanError::WrongTarget)
    );
    let mut invalid = facts();
    invalid.profile.abi = Abi::Aapcs64;
    rejected(invalid, PlanError::InvalidProfile);
    for bytes in [0, 4, 16] {
        let mut invalid = facts();
        invalid.profile.data_layout.pointer_bytes = bytes;
        rejected(invalid, PlanError::InvalidProfile);
    }
    let mut invalid = facts();
    invalid.profile.data_layout.endianness = Endianness::Big;
    rejected(invalid, PlanError::InvalidProfile);
    let mut invalid = facts();
    invalid.profile.capabilities.runtime_trace = false;
    invalid.runtime_trace = RuntimeTracePolicy::Enabled;
    rejected(invalid, PlanError::UnsupportedCapability);
    for ty in [
        ScalarType::F64,
        ScalarType::CodeAddress(SignatureId::new(0)),
    ] {
        let mut invalid = facts();
        invalid.profile.capabilities.binary64 = false;
        invalid.profile.capabilities.indirect_calls = false;
        invalid.signatures[0].inputs.push(Component {
            ty,
            role: ComponentRole::Parameter(0),
        });
        rejected(invalid, PlanError::UnsupportedCapability);
    }
}

#[test]
fn layout_dispositions_and_checked_extents_do_not_invent_payloads() {
    let mut supplied = facts();
    supplied.layouts.extend([
        LayoutFact {
            size: 0,
            alignment: 8,
            disposition: LayoutDisposition::Addressable,
        },
        LayoutFact {
            size: 0,
            alignment: 1,
            disposition: LayoutDisposition::ElidedUnit,
        },
        LayoutFact {
            size: 0,
            alignment: 1,
            disposition: LayoutDisposition::ElidedMetadata,
        },
    ]);
    let plan = CheckedPlan::check(supplied).unwrap();
    let v = plan.view();
    assert_eq!(
        v.layout(v.layout_id(1).unwrap())
            .unwrap()
            .checked_access(0, 0),
        Ok(())
    );
    let layout = *v.layout(v.layout_id(0).unwrap()).unwrap();
    assert_eq!(layout.checked_access(4, 4), Ok(()));
    assert_eq!(layout.checked_access(8, 1), Err(PlanError::OutOfBounds));
    assert_eq!(
        layout.checked_access(usize::MAX, 1),
        Err(PlanError::SizeOverflow)
    );
    for alignment in [0, 3] {
        let mut invalid = facts();
        invalid.layouts[0].alignment = alignment;
        rejected(invalid, PlanError::InvalidLayout);
    }
    let mut invalid = facts();
    invalid.layouts[0].size = usize::MAX;
    rejected(invalid, PlanError::SizeOverflow);
    let mut invalid = facts();
    invalid.layouts[0].disposition = LayoutDisposition::ElidedUnit;
    rejected(invalid, PlanError::InvalidLayout);
    assert_eq!(
        v.layout_id(usize::MAX).err(),
        Some(PlanError::UnknownDeclaration)
    );
}

#[test]
fn mixed_pressure_roles_are_independent_of_list_position_and_architecture() {
    let mut supplied = facts();
    let s = &mut supplied.signatures[0];
    s.inputs = vec![
        address(ComponentRole::ReceiverMetadata),
        address(ComponentRole::ResultDestination(LayoutId::new(0))),
        address(ComponentRole::ReceiverStatic),
        address(ComponentRole::ReceiverComplete),
        address(ComponentRole::AliasAddress(16)),
        address(ComponentRole::AliasComplete(16)),
        address(ComponentRole::AliasMetadata(16)),
    ];
    s.inputs.extend((0..16).map(|i| Component {
        ty: if i < 7 {
            ScalarType::I64
        } else {
            ScalarType::F64
        },
        role: ComponentRole::Parameter(i),
    }));
    s.returns = ReturnShape::Aggregate(LayoutId::new(0));
    for (architecture, abi) in [
        (Architecture::X86_64, Abi::SysV),
        (Architecture::Aarch64, Abi::Aapcs64),
    ] {
        let mut candidate = supplied.clone();
        candidate.profile.architecture = architecture;
        candidate.profile.abi = abi;
        let plan = CheckedPlan::check(candidate).unwrap();
        let signature = plan
            .view()
            .callable(source(0))
            .unwrap()
            .signature()
            .unwrap();
        assert!(signature.results.is_empty());
        assert_eq!(
            signature.inputs[1].role,
            ComponentRole::ResultDestination(LayoutId::new(0))
        );
        assert_eq!(
            signature
                .inputs
                .iter()
                .filter(|c| c.ty == ScalarType::F64)
                .count(),
            9
        );
    }
}

#[test]
fn signatures_reject_fake_results_and_incomplete_or_conflicting_roles() {
    let mut invalid = facts();
    invalid.signatures[0].results.push(Component {
        ty: ScalarType::I64,
        role: ComponentRole::Result,
    });
    rejected(invalid, PlanError::InvalidSignature);
    for role in [
        ComponentRole::ReceiverStatic,
        ComponentRole::AliasMetadata(0),
        ComponentRole::ResultDestination(LayoutId::new(0)),
        ComponentRole::Result,
        ComponentRole::RuntimeParameter(0),
    ] {
        let mut invalid = facts();
        invalid.signatures[0].inputs.push(address(role));
        rejected(invalid, PlanError::InvalidSignature);
    }
    let mut invalid = facts();
    invalid.signatures[0].inputs = vec![
        Component {
            ty: ScalarType::I64,
            role: ComponentRole::Parameter(0),
        },
        address(ComponentRole::AggregateAddress {
            parameter: 0,
            layout: LayoutId::new(0),
        }),
    ];
    rejected(invalid, PlanError::InvalidSignature);
    let mut invalid = facts();
    invalid.signatures[0].inputs = vec![address(ComponentRole::Parameter(0)); 2];
    rejected(invalid, PlanError::InvalidSignature);
    let mut invalid = facts();
    invalid.signatures[0].returns = ReturnShape::Aggregate(LayoutId::new(0));
    invalid.signatures[0].inputs.push(Component {
        ty: ScalarType::I64,
        role: ComponentRole::ResultDestination(LayoutId::new(0)),
    });
    rejected(invalid, PlanError::InvalidSignature);
}

#[test]
fn all_scalar_components_and_signature_typed_addresses_are_checked() {
    for ty in [
        ScalarType::I64,
        ScalarType::U64,
        ScalarType::U8,
        ScalarType::Bool,
        ScalarType::F64,
        ScalarType::DataAddress,
        ScalarType::CodeAddress(SignatureId::new(0)),
    ] {
        let mut supplied = facts();
        supplied.signatures[0].returns = ReturnShape::Scalar(ty);
        supplied.signatures[0].results = vec![Component {
            ty,
            role: ComponentRole::Result,
        }];
        CheckedPlan::check(supplied).unwrap();
    }
    let mut invalid = facts();
    invalid.signatures[0].inputs.push(Component {
        ty: ScalarType::CodeAddress(SignatureId::new(usize::MAX)),
        role: ComponentRole::Parameter(0),
    });
    rejected(invalid, PlanError::UnknownDeclaration);
    let mut supplied = facts();
    supplied.signatures[0].convention = Convention::Runtime;
    supplied.signatures[0].returns = ReturnShape::Never;
    supplied.signatures[0]
        .inputs
        .push(address(ComponentRole::RuntimeParameter(0)));
    CheckedPlan::check(supplied).unwrap();
}

#[test]
fn typed_artifacts_validate_category_and_trace_policy_before_lookup() {
    let mut supplied = facts();
    supplied.runtime_trace = RuntimeTracePolicy::Enabled;
    let mut runtime = supplied.signatures[0].clone();
    runtime.convention = Convention::Runtime;
    supplied.signatures.push(runtime);
    supplied.artifacts = vec![
        ArtifactDeclaration {
            key: ArtifactId::Data(DataKey::Table(0)),
            signature: None,
            layout: Some(LayoutId::new(0)),
        },
        ArtifactDeclaration {
            key: ArtifactId::TraceTls,
            signature: None,
            layout: Some(LayoutId::new(0)),
        },
        ArtifactDeclaration {
            key: ArtifactId::Runtime(RuntimeService::AbiMarker),
            signature: Some(SignatureId::new(1)),
            layout: None,
        },
    ];
    let plan = CheckedPlan::check(supplied.clone()).unwrap();
    let v = plan.view();
    let table = v.artifact_id(ArtifactId::Data(DataKey::Table(0))).unwrap();
    assert_eq!(
        v.artifact(table, ArtifactCategory::Callable),
        Err(PlanError::ArtifactCategoryMismatch)
    );
    assert!(v.artifact(table, ArtifactCategory::Data).is_ok());
    assert_eq!(
        v.artifact_id(ArtifactId::Data(DataKey::Table(usize::MAX)))
            .err(),
        Some(PlanError::UnknownDeclaration)
    );
    for key in [
        ArtifactId::TraceTls,
        ArtifactId::Data(DataKey::TraceBytes(0)),
        ArtifactId::Data(DataKey::TraceContext(0)),
        ArtifactId::Data(DataKey::TraceLocation(0)),
    ] {
        let mut invalid = facts();
        invalid.artifacts.push(ArtifactDeclaration {
            key,
            signature: None,
            layout: Some(LayoutId::new(0)),
        });
        rejected(invalid, PlanError::OmittedTrace);
    }
    supplied.artifacts[0].signature = Some(SignatureId::new(0));
    rejected(supplied, PlanError::InvalidArtifact);
}

#[test]
fn catalogs_are_canonical_and_do_not_reconstruct_absent_bodies() {
    let mut supplied = facts();
    supplied.callables.push(CallableDeclaration {
        key: source(2),
        signature: SignatureId::new(0),
        body: BodyDisposition::Absent,
    });
    let a = CheckedPlan::check(supplied.clone()).unwrap();
    supplied.callables.reverse();
    let b = CheckedPlan::check(supplied.clone()).unwrap();
    assert_eq!(
        a.view().callables().collect::<Vec<_>>(),
        b.view().callables().collect::<Vec<_>>()
    );
    assert_eq!(
        a.view().artifacts().collect::<Vec<_>>(),
        b.view().artifacts().collect::<Vec<_>>()
    );
    assert_eq!(
        a.view().callable(source(2)).err(),
        Some(PlanError::AbsentBody)
    );
    assert_eq!(
        a.view().callable(source(usize::MAX)).err(),
        Some(PlanError::UnknownDeclaration)
    );
    supplied.callables.last_mut().unwrap().body = BodyDisposition::Absent;
    rejected(supplied, PlanError::InvalidDomain);
    let mut invalid = facts();
    invalid.callables.push(invalid.callables[0]);
    rejected(invalid, PlanError::DuplicateDeclaration);
}

#[test]
fn helper_and_target_keys_cannot_masquerade_as_source_definitions() {
    for family in [
        HelperFamily::ArrayElementInitializer,
        HelperFamily::ArrayElementCopier,
        HelperFamily::ArrayClone,
        HelperFamily::ArrayElementDestroyer,
        HelperFamily::ArrayRelease,
        HelperFamily::ArraySharedFinalizer,
        HelperFamily::RawClassCopy,
        HelperFamily::Retain,
        HelperFamily::Release,
        HelperFamily::ClassFinalizer,
        HelperFamily::OptionalBoxFinalizer,
    ] {
        let mut supplied = facts();
        let key = LirCallableId::Helper(HelperKey {
            family,
            layout: LayoutId::new(0),
            signature: SignatureId::new(0),
        });
        supplied.callables.push(CallableDeclaration {
            key,
            signature: SignatureId::new(0),
            body: BodyDisposition::Required,
        });
        let plan = CheckedPlan::check(supplied).unwrap();
        assert_eq!(plan.view().callable(key).unwrap().key(), key);
    }
    for kind in [Coordinator::Initializer, Coordinator::Finalizer] {
        let mut supplied = facts();
        supplied.callables.push(CallableDeclaration {
            key: LirCallableId::Coordinator(kind),
            signature: SignatureId::new(0),
            body: BodyDisposition::Required,
        });
        CheckedPlan::check(supplied).unwrap();
    }
    let mut invalid = facts();
    invalid.callables.push(CallableDeclaration {
        key: LirCallableId::TargetThunk(TargetThunkKey {
            family: 0,
            specialization: 0,
        }),
        signature: SignatureId::new(0),
        body: BodyDisposition::Required,
    });
    rejected(invalid, PlanError::InvalidDomain);
}

#[test]
fn certified_static_domain_and_stable_dispatch_remain_supplied_authority() {
    let field = StaticFieldId::new(ClassId::new(0), 0);
    let mut supplied = facts();
    supplied.artifacts.push(ArtifactDeclaration {
        key: ArtifactId::Data(DataKey::Static(field)),
        signature: None,
        layout: Some(LayoutId::new(0)),
    });
    supplied.dispatch = vec![
        DispatchSlot {
            family: VirtualFamilyId::new(0),
            index: 1,
            target: None,
        },
        DispatchSlot {
            family: VirtualFamilyId::new(0),
            index: 0,
            target: Some(source(0)),
        },
    ];
    let plan = CheckedPlan::check(supplied.clone()).unwrap();
    assert!(!plan.view().is_active_static(field));
    assert_eq!(plan.view().dispatch()[0].index, 0);
    assert_eq!(plan.view().dispatch()[1].target, None);
    supplied.artifact_policy = ArtifactPolicy::Reachable;
    rejected(supplied.clone(), PlanError::InvalidDomain);
    supplied.active_statics.insert(field);
    assert!(CheckedPlan::check(supplied.clone())
        .unwrap()
        .view()
        .is_active_static(field));
    supplied.dispatch[0].index = usize::MAX;
    rejected(supplied, PlanError::InvalidDispatch);
}

#[test]
fn runtime_external_and_literal_declarations_keep_distinct_contracts() {
    use crate::identity::{ExternalLinkId, LiteralDataId};
    let mut supplied = facts();
    let mut runtime = supplied.signatures[0].clone();
    runtime.convention = Convention::Runtime;
    let mut external = runtime.clone();
    external.convention = Convention::ExternC;
    supplied.signatures.extend([runtime, external]);
    super::test_fixtures::runtime_declarations(&mut supplied);
    supplied.artifacts.extend([
        ArtifactDeclaration {
            key: ArtifactId::External(ExternalLinkId::new(0)),
            signature: Some(SignatureId::new(2)),
            layout: None,
        },
        ArtifactDeclaration {
            key: ArtifactId::Data(DataKey::Literal(LiteralDataId::new(0))),
            signature: None,
            layout: Some(LayoutId::new(0)),
        },
    ]);
    supplied.callables.push(CallableDeclaration {
        key: LirCallableId::Entry,
        signature: SignatureId::new(2),
        body: BodyDisposition::Required,
    });
    let plan = CheckedPlan::check(supplied.clone()).unwrap();
    let v = plan.view();
    for (key, category) in [
        (
            ArtifactId::Runtime(RuntimeService::Free),
            ArtifactCategory::Runtime,
        ),
        (
            ArtifactId::External(ExternalLinkId::new(0)),
            ArtifactCategory::External,
        ),
    ] {
        assert!(v.artifact(v.artifact_id(key).unwrap(), category).is_ok());
    }
    supplied.artifacts.reverse();
    let reversed = CheckedPlan::check(supplied.clone()).unwrap();
    assert_eq!(
        v.artifacts().collect::<Vec<_>>(),
        reversed.view().artifacts().collect::<Vec<_>>()
    );
    let i = supplied
        .artifacts
        .iter()
        .position(|d| matches!(d.key, ArtifactId::External(_)))
        .unwrap();
    supplied.artifacts[i].signature = Some(SignatureId::new(0));
    rejected(supplied, PlanError::InvalidArtifact);
}

#[test]
fn declaration_builders_allocate_typed_dense_ids_before_checked_freeze() {
    let mut supplied = facts();
    let layout = supplied
        .add_layout(LayoutFact {
            size: 0,
            alignment: 1,
            disposition: LayoutDisposition::Addressable,
        })
        .unwrap();
    assert_eq!(layout.index(), 1);
    let signature = supplied
        .add_signature(SignatureFact {
            convention: Convention::Language,
            inputs: vec![
                address(ComponentRole::ResultDestination(layout)),
                address(ComponentRole::AggregateAddress {
                    parameter: 0,
                    layout,
                }),
            ],
            results: vec![],
            returns: ReturnShape::Aggregate(layout),
        })
        .unwrap();
    assert_eq!(signature.index(), 1);
    supplied.callables[0].signature = signature;
    let plan = CheckedPlan::check(supplied).unwrap();
    assert_eq!(
        plan.view()
            .callable(source(0))
            .unwrap()
            .signature()
            .unwrap()
            .returns,
        ReturnShape::Aggregate(layout)
    );
}

#[test]
fn incomplete_domains_references_and_dispatch_slots_are_rejected() {
    let mut invalid = facts();
    invalid.callables.remove(0);
    rejected(invalid, PlanError::InvalidDomain);
    let mut invalid = facts();
    invalid.callables[0].signature = SignatureId::new(usize::MAX);
    rejected(invalid, PlanError::UnknownDeclaration);
    let mut invalid = facts();
    invalid.dispatch = vec![DispatchSlot {
        family: VirtualFamilyId::new(0),
        index: 0,
        target: Some(source(usize::MAX)),
    }];
    rejected(invalid, PlanError::AbsentBody);
    let mut invalid = facts();
    invalid.dispatch = vec![
        DispatchSlot {
            family: VirtualFamilyId::new(0),
            index: 0,
            target: None,
        };
        2
    ];
    rejected(invalid, PlanError::DuplicateDeclaration);
    let mut invalid = facts();
    invalid.artifacts = vec![ArtifactDeclaration {
        key: ArtifactId::Data(DataKey::Table(0)),
        signature: None,
        layout: Some(LayoutId::new(usize::MAX)),
    }];
    rejected(invalid, PlanError::UnknownDeclaration);
}

#[test]
fn semantic_catalog_is_target_portable_and_requires_dynamic_view_metadata() {
    let supplied = minimal_semantic_facts();
    assert_eq!(
        CheckedPlan::check(supplied.clone())
            .unwrap()
            .view()
            .semantic()
            .shared_header
            .unwrap()
            .header_size,
        16
    );

    let mut aarch64 = supplied.clone();
    aarch64.profile.architecture = Architecture::Aarch64;
    aarch64.profile.abi = Abi::Aapcs64;
    CheckedPlan::check(aarch64).unwrap();

    let mut invalid = supplied;
    invalid.semantic.object_views[0].components.pop();
    rejected(invalid, PlanError::InvalidDispatch);
}

#[test]
fn complete_fact_checkpoint_is_portable_across_target_profiles() {
    let mut supplied = resource_facts(StaticStorageDisposition::RetainedInactive);
    supplied.signatures[0] = SignatureFact {
        convention: Convention::Language,
        inputs: vec![
            address(ComponentRole::ResultDestination(LayoutId::new(0))),
            address(ComponentRole::ReceiverStatic),
            address(ComponentRole::ReceiverComplete),
            address(ComponentRole::ReceiverMetadata),
        ],
        results: vec![],
        returns: ReturnShape::Aggregate(LayoutId::new(0)),
    };
    let helper_signature = supplied
        .add_signature(SignatureFact {
            convention: Convention::Language,
            inputs: vec![],
            results: vec![],
            returns: ReturnShape::Unit,
        })
        .unwrap();
    let helpers = [HelperFamily::ArrayClone, HelperFamily::ArrayRelease].map(|family| {
        LirCallableId::Helper(HelperKey {
            family,
            layout: LayoutId::new(0),
            signature: helper_signature,
        })
    });
    for (helper, dependency) in [(helpers[0], helpers[1]), (helpers[1], helpers[0])] {
        supplied.callables.push(CallableDeclaration {
            key: helper,
            signature: helper_signature,
            body: BodyDisposition::Required,
        });
        supplied.resources.generated.push(GeneratedCallableFact {
            callable: helper,
            attribution: GeneratedAttribution::InheritedSourceOperation,
            dependencies: [ArtifactId::Callable(dependency)].into_iter().collect(),
        });
        let root = ArtifactRootFact {
            artifact: ArtifactId::Callable(helper),
            reason: ArtifactRootReason::GeneratedFamily,
        };
        supplied.resources.complete_roots.insert(root);
        supplied.resources.reachable_roots.insert(root);
    }
    supplied
        .resources
        .generated
        .sort_by_key(|generated| generated.callable);

    for (architecture, abi) in [
        (Architecture::X86_64, Abi::SysV),
        (Architecture::Aarch64, Abi::Aapcs64),
    ] {
        let mut candidate = supplied.clone();
        candidate.profile.architecture = architecture;
        candidate.profile.abi = abi;
        let plan = CheckedPlan::check(candidate).unwrap();
        assert_eq!(
            plan.view()
                .static_storage_disposition(StaticFieldId::new(ClassId::new(0), 0)),
            Some(StaticStorageDisposition::RetainedInactive)
        );
        assert_eq!(plan.view().resources().generated.len(), 2);
        let signature = plan
            .view()
            .callable(source(0))
            .unwrap()
            .signature()
            .unwrap();
        assert_eq!(signature.returns, ReturnShape::Aggregate(LayoutId::new(0)));
        assert_eq!(
            signature
                .inputs
                .iter()
                .map(|component| component.role)
                .collect::<Vec<_>>(),
            vec![
                ComponentRole::ResultDestination(LayoutId::new(0)),
                ComponentRole::ReceiverStatic,
                ComponentRole::ReceiverComplete,
                ComponentRole::ReceiverMetadata,
            ]
        );
        assert_eq!(
            plan.view().resources().generated[0].dependencies,
            [ArtifactId::Callable(helpers[1])].into_iter().collect()
        );
        assert_eq!(
            plan.view().resources().generated[1].dependencies,
            [ArtifactId::Callable(helpers[0])].into_iter().collect()
        );
        assert!(plan.view().callable(helpers[0]).is_ok());
        assert!(plan.view().callable(helpers[1]).is_ok());
    }
}

#[test]
fn semantic_catalog_rejects_foreign_recursive_and_inconsistent_layout_facts() {
    let mut foreign = minimal_semantic_facts();
    foreign.semantic.types.push(TypeLayoutBinding {
        ty: SemanticType::Class(ClassId::new(0)),
        layout: LayoutId::new(0),
    });
    rejected(foreign, PlanError::InvalidLayout);

    let mut recursive = minimal_semantic_facts();
    recursive.semantic.types.extend([TypeLayoutBinding {
        ty: SemanticType::Optional(crate::identity::OptionalTypeId::new(0)),
        layout: LayoutId::new(0),
    }]);
    recursive.semantic.optionals.push(OptionalLayoutFact {
        optional: crate::identity::OptionalTypeId::new(0),
        payload: SemanticType::Optional(crate::identity::OptionalTypeId::new(0)),
        storage: OptionalStorageFact::Nested(crate::identity::OptionalTypeId::new(0)),
        layout: LayoutId::new(0),
        payload_layout: LayoutId::new(0),
        state_offset: None,
        payload_offset: 0,
        nullable_niche: true,
    });
    rejected(recursive, PlanError::InvalidLayout);

    let mut overflowing = minimal_semantic_facts();
    overflowing.semantic.types.push(TypeLayoutBinding {
        ty: SemanticType::Array(crate::identity::ArrayTypeId::new(0)),
        layout: LayoutId::new(0),
    });
    overflowing.semantic.arrays.push(ArrayLayoutFact {
        array: crate::identity::ArrayTypeId::new(0),
        descriptor_layout: LayoutId::new(0),
        element: SemanticType::U64,
        element_layout: LayoutId::new(0),
        inline_owner_count_offset: 0,
        inline_length_offset: 8,
        shared_length_offset: 16,
        element_offset: 16,
        shared_element_offset: 24,
        stride: 8,
        maximum_length: u64::MAX,
        shared_maximum_length: u64::MAX,
        default: Some(ArrayDefaultElementFact::Primitive),
        copy: Some(ArrayCopyElementFact::Primitive),
        assignment: Some(ArrayAssignElementFact::Primitive),
        destruction: ArrayDestroyElementFact::Trivial,
    });
    rejected(overflowing, PlanError::SizeOverflow);

    let mut inconsistent = minimal_semantic_facts();
    inconsistent
        .semantic
        .shared_header
        .as_mut()
        .unwrap()
        .dynamic_metadata_offset = 0;
    rejected(inconsistent, PlanError::InvalidLayout);
}
