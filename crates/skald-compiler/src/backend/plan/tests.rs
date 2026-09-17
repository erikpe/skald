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
        ArtifactId::Data(DataKey::TraceRecord(0)),
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
    for service in [
        RuntimeService::Allocate,
        RuntimeService::Free,
        RuntimeService::Panic,
        RuntimeService::IoStandardHandle,
        RuntimeService::IoOpen,
        RuntimeService::IoRead,
        RuntimeService::IoWrite,
        RuntimeService::IoClose,
        RuntimeService::AbiMarker,
    ] {
        supplied.artifacts.push(ArtifactDeclaration {
            key: ArtifactId::Runtime(service),
            signature: Some(SignatureId::new(1)),
            layout: None,
        });
    }
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
