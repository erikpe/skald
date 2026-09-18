//! Opcode-derived descriptions borrow immutable payload data; no editable use/def lists.
use super::{BankKind, UnitId, ViewId};
use crate::backend::{
    effects::Effects,
    graph::{SelectedObjectId, SelectedValueId},
    plan::{ArtifactCategory, ArtifactId, Component, ScalarType, SignatureId},
};
use std::{borrow::Cow, num::NonZeroU16};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum RepresentationKind {
    Bits,
    Float,
    DataAddress,
    CodeAddress(SignatureId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct Representation {
    pub kind: RepresentationKind,
    bits: NonZeroU16,
}
#[cfg_attr(not(test), allow(dead_code))]
impl Representation {
    pub(in crate::backend) fn new(kind: RepresentationKind, bits: u16) -> Option<Self> {
        Some(Self {
            kind,
            bits: NonZeroU16::new(bits)?,
        })
    }
    /// Logical scalar representation, independent of target register assignment.
    pub(in crate::backend) fn from_scalar(ty: ScalarType, pointer_bits: u16) -> Option<Self> {
        let (kind, bits) = match ty {
            ScalarType::I64 | ScalarType::U64 => (RepresentationKind::Bits, 64),
            ScalarType::U8 | ScalarType::Bool => (RepresentationKind::Bits, 8),
            ScalarType::F64 => (RepresentationKind::Float, 64),
            ScalarType::DataAddress => (RepresentationKind::DataAddress, pointer_bits),
            ScalarType::CodeAddress(signature) => {
                (RepresentationKind::CodeAddress(signature), pointer_bits)
            }
        };
        Self::new(kind, bits)
    }
    pub(in crate::backend) fn bits(self) -> u16 {
        self.bits.get()
    }
    pub(in crate::backend) fn bank(self) -> BankKind {
        if self.kind == RepresentationKind::Float {
            BankKind::Float
        } else {
            BankKind::Integer
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Timing {
    Early,
    Late,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum OperandRole {
    Use,
    Definition,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum AbiArea {
    Incoming,
    Outgoing,
    Results,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum AbiLocation {
    Fixed(ViewId),
    Slot { area: AbiArea, index: usize },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct AbiBinding {
    pub component: Component,
    pub representation: Representation,
    pub location: AbiLocation,
}
#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Constraint<'a> {
    Resources { views: &'a [ViewId], memory: bool },
    Fixed(ViewId),
    AbiSlot { area: AbiArea, index: usize },
}
#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct Operand<'a> {
    pub value: SelectedValueId,
    pub representation: Representation,
    pub role: OperandRole,
    pub timing: Timing,
    pub constraint: Constraint<'a>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct Tie {
    pub input: usize,
    pub output: usize,
}
#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct Scratch<'a> {
    pub representation: Representation,
    pub views: &'a [ViewId],
    pub count: NonZeroU16,
}
#[derive(Clone, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Bundle<'a> {
    Atomic,
    Bounded {
        steps: NonZeroU16,
        scratch: Cow<'a, [Scratch<'a>]>,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Flow {
    Instruction,
    Branch,
    Return,
    Never,
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct Description<'a> {
    pub operands: Cow<'a, [Operand<'a>]>,
    pub ties: &'a [Tie],
    pub clobbers: &'a [(Timing, UnitId)],
    pub effects: &'a Effects<SelectedObjectId>,
    pub artifacts: &'a [(ArtifactId, ArtifactCategory)],
    pub objects: &'a [SelectedObjectId],
    pub abi_inputs: &'a [AbiBinding],
    pub abi_results: &'a [AbiBinding],
    /// An indirect call's secured target is separate from logical arguments.
    pub indirect_target: Option<usize>,
    pub bundle: Option<Bundle<'a>>,
    /// Zero for instructions/returns; all branch successors live in graph storage.
    pub successors: usize,
    pub flow: Flow,
    pub call_signature: Option<SignatureId>,
    pub call_attribution: Option<&'a crate::backend::lir::CallAttribution>,
}
/// A target-owned immutable opcode record. Shared borrows must not change any
/// execution or descriptor fact: no interior mutation, mutable external aliases
/// or descriptor callbacks into MIR/source state. Cloning an editable payload
/// must not share mutable opcode storage with its published predecessor.
/// `describe` must be total even for malformed drafts; verifiers, not indexing
/// panics, reject invalid IDs and constraints. Concrete target review enforces
/// this contract; the trait is private and is not an untrusted extension API.
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) trait Payload {
    fn describe(&self) -> Description<'_>;
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Phase {
    EarlyUses,
    EarlyClobbers,
    EarlyDefinitions,
    LateUses,
    LateClobbers,
    LateDefinitions,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Event {
    Operand { phase: Phase, slot: usize },
    Clobber { phase: Phase, unit: UnitId },
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'a> Description<'a> {
    pub(in crate::backend) fn events(&'a self) -> impl Iterator<Item = Event> + 'a {
        [
            Phase::EarlyUses,
            Phase::EarlyClobbers,
            Phase::EarlyDefinitions,
            Phase::LateUses,
            Phase::LateClobbers,
            Phase::LateDefinitions,
        ]
        .into_iter()
        .flat_map(|phase| {
            let operands = self
                .operands
                .iter()
                .enumerate()
                .filter_map(move |(slot, op)| {
                    let expected = match (op.timing, op.role) {
                        (Timing::Early, OperandRole::Use) => Phase::EarlyUses,
                        (Timing::Early, OperandRole::Definition) => Phase::EarlyDefinitions,
                        (Timing::Late, OperandRole::Use) => Phase::LateUses,
                        (Timing::Late, OperandRole::Definition) => Phase::LateDefinitions,
                    };
                    (phase == expected).then_some(Event::Operand { phase, slot })
                });
            let clobbers = self.clobbers.iter().filter_map(move |(timing, unit)| {
                let expected = if *timing == Timing::Early {
                    Phase::EarlyClobbers
                } else {
                    Phase::LateClobbers
                };
                (phase == expected).then_some(Event::Clobber { phase, unit: *unit })
            });
            operands.chain(clobbers)
        })
    }
}
