use super::*;

#[test]
fn placement_contract_uses_the_canonical_native_resource_footprints() {
    let resources = NativeResources::new().unwrap();
    crate::backend::placement::tests::check_native_resource_events(
        resources.catalog(),
        resources.gpr(Gpr::Rdi, 64).unwrap(),
        resources.gpr(Gpr::R11, 64).unwrap(),
        resources.gpr(Gpr::Rax, 8).unwrap(),
        resources.gpr(Gpr::Rax, 64).unwrap(),
        resources.xmm(0, 64).unwrap(),
        resources.caller_clobbers(),
    );
}
use crate::backend::{
    plan::{
        self, test_fixtures::facts, CheckedPlan, Component, ComponentRole, Convention, ReturnShape,
        ScalarType, SignatureFact,
    },
    selected::{AbiArea, AbiLocation, BankKind, ResourceError},
};
fn checked(signature: SignatureFact) -> CheckedPlan {
    let mut supplied = facts();
    supplied.signatures[0] = signature;
    CheckedPlan::check(supplied).unwrap()
}
fn signature(types: &[ScalarType], result: ReturnShape) -> SignatureFact {
    SignatureFact {
        convention: Convention::Language,
        inputs: types
            .iter()
            .enumerate()
            .map(|(i, ty)| Component {
                ty: *ty,
                role: ComponentRole::Parameter(i),
            })
            .collect(),
        results: if let ReturnShape::Scalar(ty) = result {
            vec![Component {
                ty,
                role: ComponentRole::Result,
            }]
        } else {
            vec![]
        },
        returns: result,
    }
}
fn classify_first(plan: &CheckedPlan, resources: &NativeResources) -> ComponentAbi {
    classify(
        plan.view(),
        plan.view().callables().next().unwrap().signature,
        resources,
        CallArity::Fixed,
    )
    .unwrap()
}

#[test]
fn all_views_have_explicit_overlap_reservations_and_call_footprints() {
    let resources = NativeResources::new().unwrap();
    let catalog = resources.catalog();
    assert_eq!(catalog.units(), 33);
    assert_eq!(catalog.views().len(), 96);
    for register in Gpr::ALL {
        let full = resources.gpr(register, 64).unwrap();
        for bits in [8, 16, 32, 64] {
            let view = resources.gpr(register, bits).unwrap();
            assert!(catalog.overlaps(view, full).unwrap());
            assert_eq!(
                catalog
                    .require_view(view, bits, BankKind::Integer, true)
                    .is_ok(),
                !register.reserved()
            );
            assert_eq!(
                catalog
                    .preserved(view, resources.preserved_units())
                    .unwrap(),
                register.preserved()
            );
            assert_eq!(
                catalog
                    .view_units(view)
                    .unwrap()
                    .iter()
                    .any(|unit| resources.caller_clobbers().contains(unit)),
                !register.preserved()
            );
        }
    }
    for index in 0..16 {
        let narrow = resources.xmm(index, 64).unwrap();
        let wide = resources.xmm(index, 128).unwrap();
        assert!(catalog.overlaps(narrow, wide).unwrap());
        assert!(!catalog
            .preserved(narrow, resources.preserved_units())
            .unwrap());
        assert!(catalog
            .view_units(wide)
            .unwrap()
            .iter()
            .all(|unit| resources.caller_clobbers().contains(unit)));
        assert!(!catalog
            .overlaps(narrow, resources.gpr(Gpr::Rax, 64).unwrap())
            .unwrap());
    }
    assert!(resources.caller_clobbers().contains(&resources.flags()));
    assert!(catalog
        .views()
        .all(|(_, view)| !view.units.contains(&resources.flags())));
    assert_eq!(resources.gpr(Gpr::Rax, 128), Err(ResourceError::Width));
    assert_eq!(resources.xmm(16, 64), Err(ResourceError::Unknown));
    assert_eq!(resources.xmm(0, 256), Err(ResourceError::Width));
    assert!(Gpr::Rsi.requires_rex(8));
    assert!(Gpr::R8.requires_rex(32));
    assert!(!Gpr::Rax.requires_rex(8));
    assert!(!catalog
        .overlaps(
            resources.gpr(Gpr::Rax, 64).unwrap(),
            resources.gpr(Gpr::Rdx, 64).unwrap()
        )
        .unwrap());
    assert!(!catalog
        .overlaps(resources.xmm(0, 64).unwrap(), resources.xmm(1, 64).unwrap())
        .unwrap());
}

#[test]
fn pressure_matches_the_existing_classifier_and_distinguishes_entry_slots() {
    use super::super::{
        abi::{ArgumentLocation, CallLayout},
        machine::{Register, XmmRegister},
    };
    use crate::mir::{MirParameter, MirType};
    let resources = NativeResources::new().unwrap();
    for (integers, floats) in [(0, 0), (6, 8), (7, 9), (9, 10)] {
        let types: Vec<_> = (0..integers)
            .map(|_| ScalarType::I64)
            .chain((0..floats).map(|_| ScalarType::F64))
            .collect();
        let plan = checked(signature(&types, ReturnShape::Unit));
        let new = classify_first(&plan, &resources);
        let parameters = MirParameter::values(types.iter().map(|ty| {
            if *ty == ScalarType::F64 {
                MirType::F64
            } else {
                MirType::I64
            }
        }));
        let old = CallLayout::classify(&parameters).unwrap();
        assert_eq!(new.outgoing_bytes(), old.stack_size() as usize);
        for ((call, entry), old) in new
            .call()
            .inputs()
            .iter()
            .zip(new.entry().inputs())
            .zip(old.locations())
        {
            let expected = match old {
                ArgumentLocation::IntegerRegister(reg) => AbiLocation::Fixed(
                    resources
                        .gpr(
                            match reg {
                                Register::Rdi => Gpr::Rdi,
                                Register::Rsi => Gpr::Rsi,
                                Register::Rdx => Gpr::Rdx,
                                Register::Rcx => Gpr::Rcx,
                                Register::R8 => Gpr::R8,
                                Register::R9 => Gpr::R9,
                                _ => panic!("unexpected argument register"),
                            },
                            64,
                        )
                        .unwrap(),
                ),
                ArgumentLocation::SseRegister(reg) => AbiLocation::Fixed(
                    resources
                        .xmm(
                            match reg {
                                XmmRegister::Xmm0 => 0,
                                XmmRegister::Xmm1 => 1,
                                XmmRegister::Xmm2 => 2,
                                XmmRegister::Xmm3 => 3,
                                XmmRegister::Xmm4 => 4,
                                XmmRegister::Xmm5 => 5,
                                XmmRegister::Xmm6 => 6,
                                XmmRegister::Xmm7 => 7,
                                _ => panic!("unexpected argument register"),
                            },
                            64,
                        )
                        .unwrap(),
                ),
                ArgumentLocation::Stack(offset) => AbiLocation::Slot {
                    area: AbiArea::Outgoing,
                    index: offset as usize / 8,
                },
            };
            assert_eq!(call.location, expected);
            match call.location {
                AbiLocation::Fixed(view) => assert_eq!(entry.location, AbiLocation::Fixed(view)),
                AbiLocation::Slot { index, .. } => {
                    assert_eq!(new.stack_slots()[index], call.representation);
                    assert_eq!(
                        entry.location,
                        AbiLocation::Slot {
                            area: AbiArea::Incoming,
                            index
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn hidden_destination_receiver_and_alias_roles_classify_independently_of_list_order() {
    let mut supplied = facts();
    let layout_fact = supplied.layouts[0];
    supplied.layouts.clear();
    let layout = supplied.add_layout(layout_fact).unwrap();
    let mut sig = signature(&[], ReturnShape::Aggregate(layout));
    sig.inputs = [
        ComponentRole::ResultDestination(layout),
        ComponentRole::ReceiverStatic,
        ComponentRole::ReceiverComplete,
        ComponentRole::ReceiverMetadata,
        ComponentRole::AliasAddress(0),
        ComponentRole::AliasComplete(0),
        ComponentRole::AliasMetadata(0),
    ]
    .into_iter()
    .map(|role| Component {
        ty: ScalarType::DataAddress,
        role,
    })
    .collect();
    let old = super::super::abi::CallLayout::classify_internal_call(
        &[crate::mir::MirParameter::read_only_alias(
            crate::mir::MirType::Obj,
        )],
        true,
        true,
    )
    .unwrap();
    assert_eq!(old.stack_size(), 16);
    assert_eq!(
        old.return_destination(),
        Some(super::super::abi::ArgumentLocation::IntegerRegister(
            super::super::machine::Register::Rdi
        ))
    );
    assert_eq!(
        old.receiver_locations().unwrap().metadata(),
        super::super::abi::ArgumentLocation::IntegerRegister(super::super::machine::Register::Rcx)
    );
    assert_eq!(
        old.parameter_locations()[0].origin().unwrap().metadata(),
        super::super::abi::ArgumentLocation::Stack(0)
    );
    let mut external = sig.clone();
    external.convention = Convention::ExternC;
    let external = checked(external);
    let resources = NativeResources::new().unwrap();
    assert!(matches!(
        classify(
            external.view(),
            external.view().callables().next().unwrap().signature,
            &resources,
            CallArity::Fixed
        ),
        Err(AbiError::UnsupportedExternal)
    ));
    let expected = [Gpr::Rdi, Gpr::Rsi, Gpr::Rdx, Gpr::Rcx, Gpr::R8, Gpr::R9];
    for reverse in [false, true] {
        if reverse {
            sig.inputs.reverse();
        }
        let plan = checked(sig.clone());
        let abi = classify_first(&plan, &resources);
        assert_eq!(abi.outgoing_bytes(), 16);
        assert!(abi.call().results().is_empty());
        for binding in abi.call().inputs() {
            let rank = match binding.component.role {
                ComponentRole::ResultDestination(_) => 0,
                ComponentRole::ReceiverStatic => 1,
                ComponentRole::ReceiverComplete => 2,
                ComponentRole::ReceiverMetadata => 3,
                ComponentRole::AliasAddress(_) => 4,
                ComponentRole::AliasComplete(_) => 5,
                ComponentRole::AliasMetadata(_) => 6,
                _ => unreachable!(),
            };
            assert_eq!(
                binding.location,
                if rank < 6 {
                    AbiLocation::Fixed(resources.gpr(expected[rank], 64).unwrap())
                } else {
                    AbiLocation::Slot {
                        area: AbiArea::Outgoing,
                        index: 0,
                    }
                }
            );
        }
    }
}

#[test]
fn scalar_returns_and_external_cells_are_explicit() {
    let resources = NativeResources::new().unwrap();
    for ty in [
        ScalarType::I64,
        ScalarType::U64,
        ScalarType::U8,
        ScalarType::Bool,
        ScalarType::F64,
    ] {
        let mut sig = signature(&[ty], ReturnShape::Scalar(ty));
        sig.convention = Convention::ExternC;
        let plan = checked(sig);
        let abi = classify_first(&plan, &resources);
        let result = abi.call().results()[0];
        assert_eq!(
            result.location,
            AbiLocation::Fixed(if ty == ScalarType::F64 {
                resources.xmm(0, 64).unwrap()
            } else {
                resources
                    .gpr(
                        Gpr::Rax,
                        if matches!(ty, ScalarType::U8 | ScalarType::Bool) {
                            8
                        } else {
                            64
                        },
                    )
                    .unwrap()
            })
        );
        assert_eq!(abi.entry().results()[0], result);
        assert!(!abi.noreturn());
    }
    let plan = checked(signature(&[], ReturnShape::Never));
    assert!(classify_first(&plan, &resources).noreturn());
    assert!(matches!(
        classify(
            plan.view(),
            plan.view().callables().next().unwrap().signature,
            &resources,
            CallArity::Variadic
        ),
        Err(AbiError::UnsupportedVariadic)
    ));
    let mut sig = signature(&[ScalarType::DataAddress], ReturnShape::Unit);
    sig.convention = Convention::ExternC;
    let plan = checked(sig);
    assert!(matches!(
        classify(
            plan.view(),
            plan.view().callables().next().unwrap().signature,
            &resources,
            CallArity::Fixed
        ),
        Err(AbiError::UnsupportedExternal)
    ));
    let mut f = facts();
    f.profile.architecture = plan::Architecture::Aarch64;
    f.profile.abi = plan::Abi::Aapcs64;
    let plan = CheckedPlan::check(f).unwrap();
    assert!(matches!(
        classify(
            plan.view(),
            plan.view().callables().next().unwrap().signature,
            &resources,
            CallArity::Fixed
        ),
        Err(AbiError::Plan(plan::PlanError::WrongTarget))
    ));
}

#[test]
fn runtime_address_cells_remain_distinct_from_user_external_signatures() {
    let resources = NativeResources::new().unwrap();
    let mut sig = signature(
        &[ScalarType::DataAddress, ScalarType::U64],
        ReturnShape::Never,
    );
    sig.convention = Convention::Runtime;
    for (i, input) in sig.inputs.iter_mut().enumerate() {
        input.role = ComponentRole::RuntimeParameter(i);
    }
    let plan = checked(sig);
    let abi = classify_first(&plan, &resources);
    assert!(abi.noreturn());
    assert!(abi.call().results().is_empty());
    assert_eq!(
        abi.call().inputs()[0].location,
        AbiLocation::Fixed(resources.gpr(Gpr::Rdi, 64).unwrap())
    );
    assert_eq!(
        abi.call().inputs()[1].location,
        AbiLocation::Fixed(resources.gpr(Gpr::Rsi, 64).unwrap())
    );
}

#[test]
fn stack_slot_shapes_belong_to_each_signature_boundary() {
    let mut supplied = facts();
    supplied.signatures[0] = signature(&[ScalarType::I64; 7], ReturnShape::Unit);
    let floating = supplied
        .add_signature(signature(&[ScalarType::F64; 9], ReturnShape::Unit))
        .unwrap();
    supplied.callables[1].signature = floating;
    let integer = supplied.callables[0].signature;
    let plan = CheckedPlan::check(supplied).unwrap();
    let resources = NativeResources::new().unwrap();
    let integer = classify(plan.view(), integer, &resources, CallArity::Fixed).unwrap();
    let floating = classify(plan.view(), floating, &resources, CallArity::Fixed).unwrap();
    assert_eq!(integer.stack_slots().len(), 1);
    assert_eq!(floating.stack_slots().len(), 1);
    assert_ne!(integer.stack_slots()[0], floating.stack_slots()[0]);
    assert_eq!(integer.outgoing_bytes(), floating.outgoing_bytes());
}

#[test]
fn narrow_spills_and_internal_code_addresses_keep_their_logical_representation() {
    let resources = NativeResources::new().unwrap();
    let narrow = checked(signature(&[ScalarType::Bool; 7], ReturnShape::Unit));
    let abi = classify_first(&narrow, &resources);
    assert_eq!(abi.stack_slots()[0].bits(), 8);
    assert_eq!(abi.outgoing_bytes(), 16);
    let code = ScalarType::CodeAddress(facts().callables[0].signature);
    let internal = checked(signature(&[code], ReturnShape::Unit));
    let abi = classify_first(&internal, &resources);
    assert_eq!(
        abi.call().inputs()[0].representation.kind,
        crate::backend::selected::RepresentationKind::CodeAddress(
            internal.view().callables().next().unwrap().signature
        )
    );
    let mut external = signature(&[code], ReturnShape::Unit);
    external.convention = Convention::ExternC;
    let external = checked(external);
    assert!(matches!(
        classify(
            external.view(),
            external.view().callables().next().unwrap().signature,
            &resources,
            CallArity::Fixed
        ),
        Err(AbiError::UnsupportedExternal)
    ));
}
