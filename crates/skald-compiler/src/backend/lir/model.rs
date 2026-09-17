//! Callable-owned draft storage and explicit definition/edge identities.

use super::{
    AddressProvenance, AddressStride, BinaryOperation, Call, Constant, Conversion, DivisionResult,
    MemoryRepresentation, ShiftDirection, TraceAction, TracePlan, UnaryOperation,
};
use crate::backend::effects::Effects;
use crate::backend::failure::FailureMessage;
use crate::backend::graph::{
    LocalHandle, LoweredBlockId, LoweredObjectId, LoweredValueId, OwnedArena,
};
use crate::backend::plan::{ArtifactId, CallableBinding, LayoutFact, ScalarType};
use crate::primitive_comparison::PrimitiveComparisonPredicate;
use crate::source::Span;

pub(in crate::backend) type BlockHandle<'p> = LocalHandle<'p, LoweredBlockId>;
pub(in crate::backend) type ValueHandle<'p> = LocalHandle<'p, LoweredValueId>;
pub(in crate::backend) type ObjectHandle<'p> = LocalHandle<'p, LoweredObjectId>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct InstructionLocation {
    pub block: LoweredBlockId,
    pub ordinal: usize,
}
/// Slot in the predecessor terminator, not a predecessor/successor pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct EdgeOccurrence {
    pub predecessor: LoweredBlockId,
    pub slot: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Definition {
    EntryInput {
        component: usize,
    },
    BlockParameter {
        block: LoweredBlockId,
        ordinal: usize,
    },
    InstructionResult {
        instruction: InstructionLocation,
        ordinal: usize,
    },
}
#[derive(Clone, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct Value {
    pub ty: ScalarType,
    pub definition: Option<Definition>,
    pub origin: Option<Span>,
    pub provenance: AddressProvenance,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ObjectRole {
    SemanticStorage,
    AggregateResult,
    AggregateTemporary,
    TraceRecord,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum LifetimeDisposition {
    WholeCallable,
    Sites(usize),
}
#[derive(Clone, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct Object {
    pub layout: LayoutFact,
    pub role: ObjectRole,
    pub lifetime: LifetimeDisposition,
    pub origin: Option<Span>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum LifetimeMarker {
    Start,
    End,
}

/// Finite obligations over secured immutable values. A record is not a proof.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ScalarCheck<V = LoweredValueId> {
    NonZeroDivisor { ty: ScalarType, divisor: V },
    ShiftCountBelowWidth { count: V, width: u8 },
    FiniteTruncatedF64InIntegerRange { source: V, target: ScalarType },
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ScalarDomainEvidence<V = LoweredValueId, B = LoweredBlockId> {
    /// The block's scalar-check terminator; success protection is independently verified later.
    SuccessCheck(B),
    /// The verifier must find this exact value's constant definition and check its bits.
    ExactConstant(V),
}

/// Generic operands let the builder accept context-bound handles while stored
/// instructions use compact IDs. The vocabulary remains the same at both boundaries.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Operation<V = LoweredValueId, O = LoweredObjectId, B = LoweredBlockId> {
    Call(Call<V>),
    Trace(TraceAction<O, B>),
    Constant(Constant),
    Unary {
        operation: UnaryOperation,
        value: V,
    },
    Binary {
        operation: BinaryOperation,
        left: V,
        right: V,
    },
    Compare {
        predicate: PrimitiveComparisonPredicate,
        left: V,
        right: V,
    },
    Divide {
        result: DivisionResult,
        dividend: V,
        divisor: V,
        evidence: ScalarDomainEvidence<V, B>,
    },
    Shift {
        direction: ShiftDirection,
        value: V,
        count: V,
        evidence: ScalarDomainEvidence<V, B>,
    },
    Convert {
        conversion: Conversion,
        value: V,
        target: ScalarType,
        evidence: Option<ScalarDomainEvidence<V, B>>,
    },
    SymbolAddress {
        symbol: ArtifactId,
        ty: ScalarType,
    },
    ObjectAddress(O),
    ByteOffset {
        base: V,
        offset: V,
    },
    ScaledIndex {
        base: V,
        index: V,
        stride: AddressStride,
    },
    Load {
        address: V,
        representation: MemoryRepresentation,
    },
    Store {
        address: V,
        value: V,
        representation: MemoryRepresentation,
    },
    Lifetime {
        marker: LifetimeMarker,
        object: O,
        site: usize,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct Edge<V = LoweredValueId, B = LoweredBlockId> {
    pub target: B,
    pub arguments: Vec<V>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Terminator<V = LoweredValueId, B = LoweredBlockId> {
    Jump(Edge<V, B>),
    Branch {
        condition: V,
        true_edge: Edge<V, B>,
        false_edge: Edge<V, B>,
    },
    ScalarCheck {
        relation: ScalarCheck<V>,
        success: Edge<V, B>,
        failure: Edge<V, B>,
    },
    Return(Vec<V>),
    ReportFailure {
        call: Call<V>,
        reason: FailureMessage,
    },
    NonReturningCall(Call<V>),
    HardTrap,
}
#[derive(Clone, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct Instruction {
    pub operation: Operation,
    pub results: Vec<LoweredValueId>,
    pub effects: Effects<LoweredObjectId>,
}
#[derive(Clone, Debug, Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct Block {
    /// None is a forward reservation, distinct from a defined block with no parameters.
    pub parameters: Option<Vec<LoweredValueId>>,
    pub instructions: Vec<Instruction>,
    pub terminator: Option<Terminator>,
    pub terminal_effects: Option<Effects<LoweredObjectId>>,
}
/// Mutable construction product. It grants no seal, receipt or emission authority.
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct CallableDraft<'p> {
    pub(super) owner: CallableBinding<'p>,
    pub(super) entry: Option<LoweredBlockId>,
    pub(super) trace_plan: Option<TracePlan>,
    pub(super) inputs: Vec<LoweredValueId>,
    pub(super) blocks: OwnedArena<'p, LoweredBlockId, Block>,
    pub(super) values: OwnedArena<'p, LoweredValueId, Value>,
    pub(super) objects: OwnedArena<'p, LoweredObjectId, Object>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p> CallableDraft<'p> {
    pub(in crate::backend) fn trace_plan(&self) -> Option<&TracePlan> {
        self.trace_plan.as_ref()
    }
    pub(in crate::backend) fn owner(&self) -> CallableBinding<'p> {
        self.owner
    }
    pub(in crate::backend) fn entry(&self) -> Option<LoweredBlockId> {
        self.entry
    }
    pub(in crate::backend) fn inputs(&self) -> &[LoweredValueId] {
        &self.inputs
    }
    pub(in crate::backend) fn blocks(
        &self,
    ) -> impl ExactSizeIterator<Item = (LoweredBlockId, &Block)> {
        self.blocks.iter()
    }
    pub(in crate::backend) fn values(
        &self,
    ) -> impl ExactSizeIterator<Item = (LoweredValueId, &Value)> {
        self.values.iter()
    }
    pub(in crate::backend) fn objects(
        &self,
    ) -> impl ExactSizeIterator<Item = (LoweredObjectId, &Object)> {
        self.objects.iter()
    }
    pub(in crate::backend) fn block(
        &self,
        block: BlockHandle<'p>,
    ) -> Result<&Block, super::BuildError> {
        Ok(self.blocks.get(block)?)
    }
    pub(in crate::backend) fn value(
        &self,
        value: ValueHandle<'p>,
    ) -> Result<&Value, super::BuildError> {
        Ok(self.values.get(value)?)
    }
    pub(in crate::backend) fn object(
        &self,
        object: ObjectHandle<'p>,
    ) -> Result<&Object, super::BuildError> {
        Ok(self.objects.get(object)?)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl Terminator {
    pub(in crate::backend) fn edges(
        &self,
        predecessor: LoweredBlockId,
    ) -> impl Iterator<Item = (EdgeOccurrence, &Edge)> {
        let edges = match self {
            Self::Jump(edge) => [Some(edge), None],
            Self::Branch {
                true_edge,
                false_edge,
                ..
            } => [Some(true_edge), Some(false_edge)],
            Self::ScalarCheck {
                success, failure, ..
            } => [Some(success), Some(failure)],
            Self::Return(_)
            | Self::HardTrap
            | Self::ReportFailure { .. }
            | Self::NonReturningCall(_) => [None, None],
        };
        edges
            .into_iter()
            .enumerate()
            .filter_map(move |(slot, edge)| {
                edge.map(|edge| (EdgeOccurrence { predecessor, slot }, edge))
            })
    }
}
