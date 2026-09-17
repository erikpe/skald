use super::*;

#[test]
fn abi_bindings_preserve_hidden_and_receiver_components() {
    let (resources, view, _) = catalog();
    let mut f = facts();
    let layout_id = f.add_layout(f.layouts[0]).unwrap();
    let roles = [
        plan::ComponentRole::ResultDestination(layout_id),
        plan::ComponentRole::ReceiverStatic,
        plan::ComponentRole::ReceiverComplete,
        plan::ComponentRole::ReceiverMetadata,
    ];
    f.signatures[0].inputs = roles
        .into_iter()
        .map(|role| plan::Component {
            role,
            ty: plan::ScalarType::DataAddress,
        })
        .collect();
    f.signatures[0].returns = plan::ReturnShape::Aggregate(layout_id);
    let p = CheckedPlan::check(f).unwrap();
    let signature = p.view().callable(source(0)).unwrap().signature().unwrap();
    let inputs: Vec<_> = signature
        .inputs
        .iter()
        .enumerate()
        .map(|(index, component)| AbiBinding {
            component: *component,
            representation: Representation::new(RepresentationKind::DataAddress, 64).unwrap(),
            location: if index == 0 {
                AbiLocation::Fixed(view)
            } else {
                AbiLocation::Slot {
                    area: AbiArea::Incoming,
                    index,
                }
            },
        })
        .collect();
    let bindings = AbiBindings::new(signature, 64, &resources, inputs.clone(), vec![]).unwrap();
    assert_eq!(bindings.inputs().len(), 4);
    assert!(bindings.results().is_empty());
    let mut invalid = inputs.clone();
    invalid.swap(1, 2);
    assert!(AbiBindings::new(signature, 64, &resources, invalid, vec![]).is_err());
    let mut invalid = inputs;
    invalid[0].representation = repr();
    assert!(AbiBindings::new(signature, 64, &resources, invalid, vec![]).is_err());
}

#[test]
fn symbolic_abi_slots_check_shapes_without_physical_offsets() {
    let areas = AbiAreas {
        incoming: vec![repr()],
        outgoing: vec![repr()],
        results: vec![repr()],
    };
    for area in [AbiArea::Incoming, AbiArea::Outgoing, AbiArea::Results] {
        areas.require_slot(area, 0, repr()).unwrap();
        assert_eq!(
            areas.require_slot(area, 1, repr()),
            Err(plan::PlanError::OutOfBounds)
        );
        assert_eq!(
            areas.require_slot(
                area,
                0,
                Representation::new(RepresentationKind::Bits, 8).unwrap()
            ),
            Err(plan::PlanError::InvalidSignature)
        );
    }
}
