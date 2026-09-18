//! Semantic synthetic instructions and genuine selected publication for checker tests.
use super::super::*;
use crate::backend::{
    effects::{Effect, Effects},
    graph::{LocalHandle, SelectedBlockId, SelectedValueId},
    lir,
    plan::{
        self,
        test_fixtures::{facts, source},
        CheckedPlan, SignatureId,
    },
    selected::*,
};
use std::{borrow::Cow, num::NonZeroU16};
pub(super) type V<'p> = LocalHandle<'p, SelectedValueId>;
pub(super) type B<'p> = LocalHandle<'p, SelectedBlockId>;
#[derive(Clone, Copy)]
pub(super) struct Val {
    pub id: SelectedValueId,
    pub rep: Representation,
}
pub(super) fn val(value: V<'_>, rep: Representation) -> Val {
    Val {
        id: value.id(),
        rep,
    }
}
pub(super) fn bits() -> Representation {
    Representation::new(RepresentationKind::Bits, 64).unwrap()
}
pub(super) fn float() -> Representation {
    Representation::new(RepresentationKind::Float, 64).unwrap()
}
#[derive(Clone, Copy)]
pub(super) enum Op {
    Constant(Val),
    Address(Val),
    Read(Val),
    Add {
        input: Val,
        other: Val,
        result: Val,
        tied: bool,
    },
    Scratch(Val),
    Pair(Val, Val),
    EarlyWrite {
        input: Val,
        output: Val,
    },
    Call {
        target: Option<Val>,
        signature: SignatureId,
    },
    Jump(usize),
    Return,
}
pub(super) struct Node {
    pub op: Op,
    pub target: Machine,
    pub objects: Vec<crate::backend::graph::SelectedObjectId>,
    effects: Effects<crate::backend::graph::SelectedObjectId>,
    artifacts: Vec<(plan::ArtifactId, plan::ArtifactCategory)>,
}
impl Node {
    pub fn new(op: Op, target: &Machine) -> Self {
        let effects = if matches!(op, Op::Call { .. }) {
            Effects::new([Effect::Call])
        } else {
            Effects::default()
        };
        let artifacts = if matches!(op, Op::Address(_) | Op::Call { target: None, .. }) {
            vec![(
                plan::ArtifactId::Callable(source(1)),
                plan::ArtifactCategory::Callable,
            )]
        } else {
            vec![]
        };
        Self {
            op,
            target: target.clone(),
            objects: vec![],
            effects,
            artifacts,
        }
    }
}
impl Payload for Node {
    fn describe(&self) -> Description<'_> {
        let resource = |rep: Representation| Constraint::Resources {
            views: self.target.views(rep),
            memory: false,
        };
        let operand = |value: Val, role, timing, constraint| Operand {
            value: value.id,
            representation: value.rep,
            role,
            timing,
            constraint,
        };
        let use_op = |v: Val| operand(v, OperandRole::Use, Timing::Early, resource(v.rep));
        let def_op = |v: Val| operand(v, OperandRole::Definition, Timing::Late, resource(v.rep));
        let operands = match self.op {
            Op::Constant(v) | Op::Address(v) | Op::Scratch(v) => vec![def_op(v)],
            Op::Read(v) => vec![use_op(v)],
            Op::Pair(a, b) => vec![def_op(a), def_op(b)],
            Op::EarlyWrite { input, output } => vec![
                operand(input, OperandRole::Use, Timing::Late, resource(input.rep)),
                operand(
                    output,
                    OperandRole::Definition,
                    Timing::Early,
                    resource(output.rep),
                ),
            ],
            Op::Add {
                input,
                other,
                result,
                ..
            } => vec![use_op(input), use_op(other), def_op(result)],
            Op::Call {
                target: Some(v), ..
            } => vec![operand(
                v,
                OperandRole::Use,
                Timing::Late,
                Constraint::Fixed(self.target.secured),
            )],
            _ => vec![],
        };
        Description {
            operands: Cow::Owned(operands),
            ties: if matches!(self.op, Op::Add { tied: true, .. }) {
                &[Tie {
                    input: 0,
                    output: 2,
                }]
            } else {
                &[]
            },
            clobbers: if matches!(self.op, Op::Call { .. }) {
                &self.target.call_kills
            } else {
                &[]
            },
            effects: &self.effects,
            artifacts: &self.artifacts,
            objects: &self.objects,
            abi_inputs: &[],
            abi_results: &[],
            indirect_target: if matches!(
                self.op,
                Op::Call {
                    target: Some(_),
                    ..
                }
            ) {
                Some(0)
            } else {
                None
            },
            bundle: if matches!(self.op, Op::Scratch(_)) {
                Some(Bundle::Bounded {
                    steps: NonZeroU16::new(2).unwrap(),
                    scratch: Cow::Owned(vec![Scratch {
                        representation: bits(),
                        views: &self.target.ints,
                        count: NonZeroU16::new(2).unwrap(),
                    }]),
                })
            } else if matches!(self.op, Op::Call { .. }) {
                Some(Bundle::Atomic)
            } else {
                None
            },
            successors: if let Op::Jump(count) = self.op {
                count
            } else {
                0
            },
            flow: match self.op {
                Op::Return => Flow::Return,
                Op::Jump(_) => Flow::Branch,
                _ => Flow::Instruction,
            },
            call_signature: if let Op::Call { signature, .. } = self.op {
                Some(signature)
            } else {
                None
            },
            call_attribution: if matches!(self.op, Op::Call { .. }) {
                Some(&lir::CallAttribution::ProcessBoundary)
            } else {
                None
            },
        }
    }
}
#[derive(Clone)]
pub(super) struct Machine {
    pub profile: plan::TargetProfile,
    pub resources: ResourceCatalog,
    pub ints: Vec<ViewId>,
    pub low: ViewId,
    pub wide: ViewId,
    pub byte: ViewId,
    pub fp: ViewId,
    pub reserved: ViewId,
    pub link: ViewId,
    pub secured: ViewId,
    pub floats: Vec<ViewId>,
    pub bytes: Vec<ViewId>,
    pub wides: Vec<ViewId>,
    pub promises: Vec<ViewId>,
    pub call_kills: Vec<(Timing, UnitId)>,
}
impl Machine {
    fn views(&self, rep: Representation) -> &[ViewId] {
        match (rep.bank(), rep.bits()) {
            (BankKind::Float, 128) => &self.wides,
            (BankKind::Float, _) => &self.floats,
            (_, 8) => &self.bytes,
            _ => &self.ints,
        }
    }
    fn new(second: bool) -> Self {
        let mut profile = facts().profile;
        if second {
            profile.architecture = plan::Architecture::Aarch64;
            profile.abi = plan::Abi::Aapcs64;
        }
        let mut resources = ResourceCatalog::default();
        let int_bank = resources.bank(BankKind::Integer);
        let fp_bank = resources.bank(BankKind::Float);
        let mut ints = vec![];
        let mut call_kills = vec![];
        let mut byte = None;
        for i in 0..4 {
            let unit = resources.unit().unwrap();
            ints.push(resources.view(int_bank, 64, &[unit], false).unwrap());
            call_kills.push((Timing::Late, unit));
            if i == 0 {
                byte = Some(resources.view(int_bank, 8, &[unit], false).unwrap());
            }
        }
        let low_unit = resources.unit().unwrap();
        let high = resources.unit().unwrap();
        let low = resources.view(fp_bank, 64, &[low_unit], false).unwrap();
        let wide = resources
            .view(fp_bank, 128, &[low_unit, high], false)
            .unwrap();
        call_kills.push((Timing::Late, high));
        if !second {
            call_kills.push((Timing::Late, low_unit));
        }
        let fp_unit = resources.unit().unwrap();
        let fp = resources.view(fp_bank, 64, &[fp_unit], false).unwrap();
        call_kills.push((Timing::Late, fp_unit));
        let sp = resources.unit().unwrap();
        let reserved = resources.view(int_bank, 64, &[sp], true).unwrap();
        let link_unit = resources.unit().unwrap();
        let link = resources.view(int_bank, 64, &[link_unit], true).unwrap();
        let secured = ints[if second { 3 } else { 1 }];
        let promises = if second { vec![low, secured] } else { vec![] };
        let floats = vec![low, fp];
        let bytes = vec![byte.unwrap()];
        let wides = vec![wide];
        Self {
            profile,
            resources,
            ints,
            low,
            wide,
            byte: byte.unwrap(),
            fp,
            reserved,
            link,
            secured,
            floats,
            bytes,
            wides,
            promises,
            call_kills,
        }
    }
}
impl TargetVerifier<Node> for Machine {
    fn profile(&self) -> plan::TargetProfile {
        self.profile
    }
    fn verify_payload(&self, node: &Node, terminal: bool) -> Result<(), &'static str> {
        if matches!(node.op, Op::Jump(_) | Op::Return) == terminal {
            Ok(())
        } else {
            Err("opcode position")
        }
    }
    fn verify_callable(&self, _: &SelectedDraft<'_, Node>) -> Result<(), &'static str> {
        Ok(())
    }
}
impl PlacementTarget for Machine {
    fn profile(&self) -> plan::TargetProfile {
        self.profile
    }
    fn preserved_views(&self) -> Vec<ViewId> {
        self.promises.clone()
    }
    fn slot_footprint(
        &self,
        _: SignatureId,
        _: AbiArea,
        index: usize,
        rep: Representation,
    ) -> Result<SlotFootprint, CheckReason> {
        let bytes = if self.profile.architecture == plan::Architecture::Aarch64 {
            16
        } else {
            8
        };
        if usize::from(rep.bits()).div_ceil(8) > bytes {
            return Err(CheckReason::Abi);
        }
        Ok(SlotFootprint {
            offset: index.checked_mul(bytes).ok_or(CheckReason::Capacity)?,
            bytes,
        })
    }
    fn check_transfer(&self, transfer: &Transfer) -> Result<Vec<UnitId>, CheckReason> {
        let memory = !matches!(transfer.source, Location::Resource(_))
            && !matches!(transfer.destination, Location::Resource(_));
        if transfer.scratch.len() != usize::from(memory) {
            return Err(CheckReason::Scratch);
        }
        let mut units = vec![];
        for &view in &transfer.scratch {
            self.resources
                .require_view(
                    view,
                    transfer.source_representation.bits(),
                    transfer.source_representation.bank(),
                    true,
                )
                .map_err(|_| CheckReason::Scratch)?;
            units.extend_from_slice(self.resources.view_units(view).unwrap());
        }
        Ok(units)
    }
}
pub(super) fn fixture(
    second: bool,
    check: impl for<'p> FnOnce(&'p SelectionContext<'p>, &lir::VerifiedCallable<'p>, &Machine),
) {
    let machine = Machine::new(second);
    let mut f = facts();
    f.profile = machine.profile;
    // Distinct signature keys must not imply disjoint physical ABI areas.
    f.signatures.push(f.signatures[0].clone());
    let plan = CheckedPlan::check(f).unwrap();
    let mut lower = lir::DraftBuilder::new(plan.view().callable(source(0)).unwrap()).unwrap();
    let entry = lower.reserve_block().unwrap();
    lower.define_block(entry, &[]).unwrap();
    lower.set_entry(entry).unwrap();
    lower
        .terminate(entry, lir::Terminator::Return(vec![]))
        .unwrap();
    let lower = lir::verify_callable(lower.finish()).unwrap();
    let catalog = lir::TargetDeclarations::new(plan.view()).freeze().unwrap();
    let signature = plan.view().callable(source(0)).unwrap().signature_id();
    let context = SelectionContext::new(&catalog, machine.resources.clone())
        .with_abi_areas(
            signature,
            AbiAreas {
                outgoing: vec![bits(), bits()],
                results: vec![bits()],
                ..AbiAreas::default()
            },
        )
        .unwrap();
    let context = context
        .with_abi_areas(
            plan.view().signatures_with_ids().nth(1).unwrap().0,
            AbiAreas {
                outgoing: vec![bits(), bits()],
                ..AbiAreas::default()
            },
        )
        .unwrap();
    let mut context = context;
    for (signature, _) in plan.view().signatures_with_ids() {
        if context.areas(signature).is_none() {
            continue;
        }
        for area in [AbiArea::Incoming, AbiArea::Outgoing, AbiArea::Results] {
            let count = context.areas(signature).unwrap().slots(area).len();
            let stride = if second { 16 } else { 8 };
            context = context
                .with_abi_layout(
                    signature,
                    area,
                    AbiAreaLayout {
                        slots: (0..count)
                            .map(|index| AbiSlotLayout {
                                offset: index * stride,
                                bytes: stride,
                                alignment: stride,
                            })
                            .collect(),
                        bytes: count * stride,
                        alignment: stride,
                    },
                )
                .unwrap();
        }
    }
    check(&context, &lower, &machine);
}
pub(super) fn begin<'p>(
    context: &'p SelectionContext<'p>,
    lower: &lir::VerifiedCallable<'p>,
) -> (SelectedBuilder<'p, Node>, B<'p>) {
    let mut b = SelectedBuilder::new(context, source(0), Some(lower)).unwrap();
    let entry = b.block(&[], None).unwrap();
    let sig = context.binding(source(0)).unwrap().signature_id();
    b.entry(
        entry,
        &[],
        context.abi_bindings(sig, vec![], vec![]).unwrap(),
    )
    .unwrap();
    (b, entry)
}
pub(super) fn operand<'s, 'p>(
    draft: &mut PlacementDraft<'s, 'p, Node>,
    block: B<'p>,
    ordinal: usize,
    slot: usize,
    view: ViewId,
) {
    draft
        .assign(
            Assignment::Operand {
                site: Site::Instruction {
                    block: block.id(),
                    ordinal,
                },
                slot,
            },
            Location::Resource(view),
        )
        .unwrap();
}
pub(super) fn copy(
    value: SelectedValueId,
    rep: Representation,
    source: Location,
    destination: Location,
) -> Transfer {
    Transfer {
        value: TransferValue::Selected(value),
        source,
        destination,
        source_representation: rep,
        destination_representation: rep,
        kind: TransferKind::Copy,
        scratch: vec![],
    }
}
pub(super) fn save_promises(
    draft: &mut PlacementDraft<'_, '_, Node>,
    machine: &Machine,
    exit: SelectedBlockId,
) {
    for &view in &machine.promises {
        let resource = machine
            .resources
            .views()
            .find(|(id, _)| *id == view)
            .unwrap()
            .1;
        let bank = machine
            .resources
            .banks()
            .find(|(id, _)| *id == resource.bank)
            .unwrap()
            .1;
        let rep = Representation::new(
            if bank == BankKind::Float {
                RepresentationKind::Float
            } else {
                RepresentationKind::Bits
            },
            resource.bits,
        )
        .unwrap();
        let save = draft.storage(Storage {
            representation: rep,
            bytes: usize::from(rep.bits()).div_ceil(8),
            alignment: 8,
            purpose: StoragePurpose::CalleeSave(view),
            lifetime: StorageLifetime::WholeCallable,
        });
        let mut transfer = Transfer {
            value: TransferValue::Preserved(view),
            source: Location::Resource(view),
            destination: Location::Storage(save),
            source_representation: rep,
            destination_representation: rep,
            kind: TransferKind::Copy,
            scratch: vec![],
        };
        draft.transfer(TransferPoint::Entry, transfer.clone());
        std::mem::swap(&mut transfer.source, &mut transfer.destination);
        draft.transfer(TransferPoint::Before(Site::Terminal(exit)), transfer);
    }
}
