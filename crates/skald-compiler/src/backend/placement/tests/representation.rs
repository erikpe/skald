use super::super::*;
use crate::backend::{
    effects::Effects,
    graph::{SelectedBlockId, SelectedValueId},
    lir,
    plan::{
        self,
        test_fixtures::{facts, source},
        CheckedPlan,
    },
    selected::{self, *},
};
use std::borrow::Cow;

enum Node {
    Constant(
        SelectedValueId,
        ViewId,
        Effects<crate::backend::graph::SelectedObjectId>,
    ),
    Return(Effects<crate::backend::graph::SelectedObjectId>),
    Jump(usize, Effects<crate::backend::graph::SelectedObjectId>),
    Bounded(
        SelectedValueId,
        Vec<ViewId>,
        Effects<crate::backend::graph::SelectedObjectId>,
    ),
}
impl Payload for Node {
    fn describe(&self) -> Description<'_> {
        let operands = match self {
            Self::Constant(value, view, _) => vec![Operand {
                value: *value,
                representation: repr(),
                role: OperandRole::Definition,
                timing: Timing::Late,
                constraint: Constraint::Fixed(*view),
            }],
            Self::Return(_) => vec![],
            Self::Jump(..) => vec![],
            Self::Bounded(value, views, _) => vec![Operand {
                value: *value,
                representation: repr(),
                role: OperandRole::Definition,
                timing: Timing::Late,
                constraint: Constraint::Resources {
                    views,
                    memory: false,
                },
            }],
        };
        let effects = match self {
            Self::Constant(_, _, effects)
            | Self::Return(effects)
            | Self::Jump(_, effects)
            | Self::Bounded(_, _, effects) => effects,
        };
        Description {
            operands: Cow::Owned(operands),
            ties: &[],
            clobbers: &[],
            effects,
            artifacts: &[],
            objects: &[],
            abi_inputs: &[],
            abi_results: &[],
            indirect_target: None,
            bundle: match self {
                Self::Bounded(_, views, _) => Some(Bundle::Bounded {
                    steps: std::num::NonZeroU16::new(2).unwrap(),
                    scratch: Cow::Owned(vec![Scratch {
                        representation: repr(),
                        views,
                        count: std::num::NonZeroU16::new(1).unwrap(),
                    }]),
                }),
                _ => None,
            },
            successors: if let Self::Jump(count, _) = self {
                *count
            } else {
                0
            },
            flow: if matches!(self, Self::Return(_)) {
                Flow::Return
            } else if matches!(self, Self::Jump(..)) {
                Flow::Branch
            } else {
                Flow::Instruction
            },
            call_signature: None,
            call_attribution: None,
        }
    }
}
struct Target;
impl TargetVerifier<Node> for Target {
    fn profile(&self) -> plan::TargetProfile {
        facts().profile
    }
    fn verify_payload(&self, node: &Node, terminal: bool) -> Result<(), &'static str> {
        if matches!(node, Node::Return(_) | Node::Jump(..)) == terminal {
            Ok(())
        } else {
            Err("wrong opcode position")
        }
    }
    fn verify_callable(&self, _: &SelectedDraft<'_, Node>) -> Result<(), &'static str> {
        Ok(())
    }
}

#[test]
fn parameters_edges_and_unreachable_scratch_require_complete_coordinates() {
    with_selected(|context, lower, view| {
        let mut builder = SelectedBuilder::new(context, source(0), Some(lower)).unwrap();
        let entry = builder.block(&[], None).unwrap();
        let constant = builder.value(repr(), None).unwrap();
        let parameter = builder.value(repr(), None).unwrap();
        let exit = builder.block(&[parameter], None).unwrap();
        let unreachable = builder.block(&[], None).unwrap();
        let other = builder.value(repr(), None).unwrap();
        let signature = context.binding(source(0)).unwrap().signature_id();
        builder
            .entry(
                entry,
                &[],
                context.abi_bindings(signature, vec![], vec![]).unwrap(),
            )
            .unwrap();
        builder
            .append(
                entry,
                Node::Constant(constant.id(), view, Effects::default()),
            )
            .unwrap();
        builder
            .terminate(
                entry,
                Node::Jump(2, Effects::default()),
                &[(exit, vec![constant]), (exit, vec![constant])],
            )
            .unwrap();
        builder
            .terminate(exit, Node::Return(Effects::default()), &[])
            .unwrap();
        builder
            .append(
                unreachable,
                Node::Bounded(other.id(), vec![view], Effects::default()),
            )
            .unwrap();
        builder
            .terminate(unreachable, Node::Return(Effects::default()), &[])
            .unwrap();
        let selected = selected::verify_selected(builder.finish(), &Target)
            .ok()
            .unwrap();
        let mut draft = PlacementDraft::new(&selected);
        for block in [entry.id(), unreachable.id()] {
            draft
                .assign(
                    Assignment::Operand {
                        site: Site::Instruction { block, ordinal: 0 },
                        slot: 0,
                    },
                    Location::Resource(view),
                )
                .unwrap();
        }
        draft
            .assign(
                Assignment::Parameter {
                    block: exit.id(),
                    slot: 0,
                },
                Location::Resource(view),
            )
            .unwrap();
        draft
            .assign(
                Assignment::EdgeArgument {
                    block: entry.id(),
                    edge: 0,
                    slot: 0,
                },
                Location::Resource(view),
            )
            .unwrap();
        let edge = Assignment::EdgeArgument {
            block: entry.id(),
            edge: 1,
            slot: 0,
        };
        assert_eq!(
            draft.validate_structure(),
            Err(PlacementError::MissingAssignment(edge))
        );
        draft.assign(edge, Location::Resource(view)).unwrap();
        let scratch = Assignment::Scratch {
            site: Site::Instruction {
                block: unreachable.id(),
                ordinal: 0,
            },
            group: 0,
            slot: 0,
        };
        assert_eq!(
            draft.validate_structure(),
            Err(PlacementError::MissingAssignment(scratch))
        );
        draft.assign(scratch, Location::Resource(view)).unwrap();
        assert_eq!(draft.validate_structure(), Ok(()));
        draft
            .assign(Assignment::Input(0), Location::Resource(view))
            .unwrap();
        assert_eq!(
            draft.validate_structure(),
            Err(PlacementError::UnknownAssignment(Assignment::Input(0)))
        );
    });
}
fn repr() -> Representation {
    Representation::new(RepresentationKind::Bits, 64).unwrap()
}
fn with_selected(
    check: impl for<'p> FnOnce(&'p SelectionContext<'p>, &lir::VerifiedCallable<'p>, ViewId),
) {
    with_fixture(false, check);
}
fn with_fixture(
    with_input: bool,
    check: impl for<'p> FnOnce(&'p SelectionContext<'p>, &lir::VerifiedCallable<'p>, ViewId),
) {
    let mut declarations = facts();
    if with_input {
        declarations.signatures[0].inputs.push(plan::Component {
            ty: plan::ScalarType::I64,
            role: plan::ComponentRole::Parameter(0),
        });
    }
    let plan = CheckedPlan::check(declarations).unwrap();
    let mut lower = lir::DraftBuilder::new(plan.view().callable(source(0)).unwrap()).unwrap();
    let entry = lower.reserve_block().unwrap();
    lower.define_block(entry, &[]).unwrap();
    lower.set_entry(entry).unwrap();
    lower
        .terminate(entry, lir::Terminator::Return(vec![]))
        .unwrap();
    let lower = lir::verify_callable(lower.finish()).unwrap();
    let catalog = lir::TargetDeclarations::new(plan.view()).freeze().unwrap();
    let mut resources = ResourceCatalog::default();
    let bank = resources.bank(BankKind::Integer);
    let unit = resources.unit().unwrap();
    let view = resources.view(bank, 64, &[unit], false).unwrap();
    let float_bank = resources.bank(BankKind::Float);
    let float_unit = resources.unit().unwrap();
    resources
        .view(float_bank, 64, &[float_unit], false)
        .unwrap();
    let signature = plan.view().callable(source(0)).unwrap().signature_id();
    let context = SelectionContext::new(&catalog, resources)
        .with_abi_areas(
            signature,
            AbiAreas {
                outgoing: vec![repr()],
                ..AbiAreas::default()
            },
        )
        .unwrap();
    check(&context, &lower, view);
}
fn body<'p>(
    context: &'p SelectionContext<'p>,
    lower: &lir::VerifiedCallable<'p>,
    view: ViewId,
) -> (
    VerifiedSelectedCallable<'p, Node>,
    SelectedBlockId,
    SelectedValueId,
) {
    let mut builder = SelectedBuilder::new(context, source(0), Some(lower)).unwrap();
    let block = builder.block(&[], None).unwrap();
    let value = builder.value(repr(), None).unwrap();
    let signature = context.binding(source(0)).unwrap().signature_id();
    let components = context
        .binding(source(0))
        .unwrap()
        .signature()
        .unwrap()
        .inputs
        .clone();
    let inputs: Vec<_> = components
        .iter()
        .map(|_| builder.value(repr(), None).unwrap())
        .collect();
    let bindings = components
        .iter()
        .map(|component| AbiBinding {
            component: *component,
            representation: repr(),
            location: AbiLocation::Fixed(view),
        })
        .collect();
    builder
        .entry(
            block,
            &inputs,
            context.abi_bindings(signature, bindings, vec![]).unwrap(),
        )
        .unwrap();
    builder
        .append(block, Node::Constant(value.id(), view, Effects::default()))
        .unwrap();
    builder
        .terminate(block, Node::Return(Effects::default()), &[])
        .unwrap();
    (
        selected::verify_selected(builder.finish(), &Target)
            .ok()
            .unwrap(),
        block.id(),
        value.id(),
    )
}

#[test]
fn entry_abi_inputs_require_their_own_assignment() {
    with_fixture(true, |context, lower, view| {
        let (selected, block, _) = body(context, lower, view);
        let mut draft = PlacementDraft::new(&selected);
        assert_eq!(
            draft.validate_structure(),
            Err(PlacementError::MissingAssignment(Assignment::Input(0)))
        );
        draft
            .assign(Assignment::Input(0), Location::Resource(view))
            .unwrap();
        let operand = Assignment::Operand {
            site: Site::Instruction { block, ordinal: 0 },
            slot: 0,
        };
        assert_eq!(
            draft.validate_structure(),
            Err(PlacementError::MissingAssignment(operand))
        );
        draft.assign(operand, Location::Resource(view)).unwrap();
        assert_eq!(draft.validate_structure(), Ok(()));
    });
}
#[test]
fn structure_is_complete_but_confers_no_value_flow_authority() {
    with_selected(|context, lower, view| {
        let (selected, block, value) = body(context, lower, view);
        let mut draft = PlacementDraft::new(&selected);
        let assignment = Assignment::Operand {
            site: Site::Instruction { block, ordinal: 0 },
            slot: 0,
        };
        assert_eq!(
            draft.validate_structure(),
            Err(PlacementError::MissingAssignment(assignment))
        );
        let home = draft.storage(Storage {
            representation: repr(),
            bytes: 8,
            alignment: 8,
            purpose: StoragePurpose::Home(value),
            lifetime: StorageLifetime::WholeCallable,
        });
        draft.assign(assignment, Location::Storage(home)).unwrap();
        assert_eq!(
            draft.assign(assignment, Location::Resource(view)),
            Err(PlacementError::DuplicateAssignment(assignment))
        );
        // The opcode demands a register. Shape checking deliberately does not grant acceptance.
        assert_eq!(draft.validate_structure(), Ok(()));
        draft.transfer(
            TransferPoint::After(Site::Terminal(block)),
            Transfer {
                value: TransferValue::Selected(value),
                source: Location::Resource(view),
                destination: Location::Storage(home),
                source_representation: repr(),
                destination_representation: repr(),
                kind: TransferKind::Copy,
                scratch: vec![],
            },
        );
        assert_eq!(
            draft.validate_structure(),
            Err(PlacementError::UnknownTransferPoint(TransferPoint::After(
                Site::Terminal(block)
            )))
        );
    });
}
#[test]
fn equal_ids_do_not_bind_a_replacement_snapshot() {
    with_selected(|context, lower, view| {
        let (first, block, value) = body(context, lower, view);
        let (second, other_block, other_value) = body(context, lower, view);
        assert_eq!((block, value), (other_block, other_value));
        let draft = PlacementDraft::new(&first);
        assert_eq!(draft.require_selected(&first), Ok(()));
        assert_eq!(
            draft.require_selected(&second),
            Err(PlacementError::WrongSnapshot)
        );
    });
}
#[test]
fn invalid_storage_and_transfer_types_reject_deterministically() {
    with_selected(|context, lower, view| {
        let (selected, _, value) = body(context, lower, view);
        let mut draft = PlacementDraft::new(&selected);
        draft.storage(Storage {
            representation: repr(),
            bytes: 7,
            alignment: 3,
            purpose: StoragePurpose::Home(value),
            lifetime: StorageLifetime::WholeCallable,
        });
        assert_eq!(
            draft.validate_structure(),
            Err(PlacementError::InvalidStorage(0))
        );
    });
}

#[test]
fn typed_storage_abi_and_bitwise_transfers_remain_distinct() {
    with_selected(|context, lower, view| {
        let (selected, block, value) = body(context, lower, view);
        let mut draft = PlacementDraft::new(&selected);
        let point = TransferPoint::After(Site::Instruction { block, ordinal: 0 });
        let float = Representation::new(RepresentationKind::Float, 64).unwrap();
        let float_view = context
            .resources
            .views()
            .find(|(_, v)| {
                v.bits == 64 && v.bank != context.resources.views().next().unwrap().1.bank
            })
            .unwrap()
            .0;
        let scratch = draft.storage(Storage {
            representation: float,
            bytes: 8,
            alignment: 8,
            purpose: StoragePurpose::TransferScratch,
            lifetime: StorageLifetime::Transfer(point),
        });
        draft.storage(Storage {
            representation: repr(),
            bytes: 8,
            alignment: 8,
            purpose: StoragePurpose::Spill(value),
            lifetime: StorageLifetime::WholeCallable,
        });
        let saved = draft.storage(Storage {
            representation: repr(),
            bytes: 8,
            alignment: 8,
            purpose: StoragePurpose::CalleeSave(view),
            lifetime: StorageLifetime::WholeCallable,
        });
        draft.transfer(
            TransferPoint::Entry,
            Transfer {
                value: TransferValue::Preserved(view),
                source: Location::Resource(view),
                destination: Location::Storage(saved),
                source_representation: repr(),
                destination_representation: repr(),
                kind: TransferKind::Copy,
                scratch: vec![],
            },
        );
        draft.transfer(
            TransferPoint::Before(Site::Terminal(block)),
            Transfer {
                value: TransferValue::Preserved(view),
                source: Location::Storage(saved),
                destination: Location::Resource(view),
                source_representation: repr(),
                destination_representation: repr(),
                kind: TransferKind::Copy,
                scratch: vec![],
            },
        );
        let assignment = Assignment::Operand {
            site: Site::Instruction { block, ordinal: 0 },
            slot: 0,
        };
        draft.assign(assignment, Location::Resource(view)).unwrap();
        let signature = context.binding(source(0)).unwrap().signature_id();
        draft.transfer(
            point,
            Transfer {
                value: TransferValue::Selected(value),
                source: Location::Resource(view),
                destination: Location::Abi {
                    signature,
                    area: AbiArea::Outgoing,
                    index: 0,
                },
                source_representation: repr(),
                destination_representation: repr(),
                kind: TransferKind::Copy,
                scratch: vec![],
            },
        );
        draft.transfer(
            point,
            Transfer {
                value: TransferValue::Selected(value),
                source: Location::Resource(view),
                destination: Location::Storage(scratch),
                source_representation: repr(),
                destination_representation: float,
                kind: TransferKind::Bitwise,
                scratch: vec![float_view],
            },
        );
        assert_eq!(draft.validate_structure(), Ok(()));
        draft.transfers.get_mut(&point).unwrap()[1].kind = TransferKind::Copy;
        assert_eq!(
            draft.validate_structure(),
            Err(PlacementError::InvalidTransfer(point, 1))
        );
        draft.transfers.get_mut(&point).unwrap()[1].kind = TransferKind::Bitwise;
        draft.transfers.get_mut(&point).unwrap()[0].destination = Location::Abi {
            signature,
            area: AbiArea::Outgoing,
            index: 1,
        };
        assert_eq!(
            draft.validate_structure(),
            Err(PlacementError::InvalidTransfer(point, 0))
        );
        draft
            .assignments
            .insert(assignment, Location::Storage(StorageId(999)));
        assert_eq!(
            draft.validate_structure(),
            Err(PlacementError::InvalidLocation(assignment))
        );
    });
}
