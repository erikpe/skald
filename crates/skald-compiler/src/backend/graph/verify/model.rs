//! Stage adapters describe storage; shared checks do not recover execution meaning.
use crate::backend::plan::{LirCallableId, TargetProfile};
use crate::source::Span;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum GraphStage {
    Lowered,
    Selected,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct GraphIdentity {
    pub stage: GraphStage,
    pub target: TargetProfile,
    pub callable: LirCallableId,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum GraphLocation {
    Entry,
    Value(usize),
    Object(usize),
    Block(usize),
    Parameter {
        block: usize,
        ordinal: usize,
    },
    Instruction {
        block: usize,
        ordinal: usize,
        operand: usize,
    },
    Terminator {
        block: usize,
        operand: usize,
    },
    Edge {
        block: usize,
        slot: usize,
        operand: usize,
    },
}
#[cfg_attr(not(test), allow(dead_code))]
impl GraphLocation {
    pub(in crate::backend) fn sort_key(self) -> (usize, usize, usize, usize, usize) {
        match self {
            GraphLocation::Entry => (0, 0, 0, 0, 0),
            GraphLocation::Value(value) => (1, value, 0, 0, 0),
            GraphLocation::Object(object) => (1, object, 1, 0, 0),
            GraphLocation::Block(block) => (2, block, 0, 0, 0),
            GraphLocation::Parameter { block, ordinal } => (2, block, 1, ordinal, 0),
            GraphLocation::Instruction {
                block,
                ordinal,
                operand,
            } => (2, block, 2, ordinal, operand),
            GraphLocation::Terminator { block, operand } => (2, block, 3, 0, operand),
            GraphLocation::Edge {
                block,
                slot,
                operand,
            } => (2, block, 4, slot, operand),
        }
    }
    pub(in crate::backend) fn operand(self, operand: usize) -> Self {
        match self {
            Self::Instruction { block, ordinal, .. } => Self::Instruction {
                block,
                ordinal,
                operand,
            },
            Self::Terminator { block, .. } => Self::Terminator { block, operand },
            Self::Edge { block, slot, .. } => Self::Edge {
                block,
                slot,
                operand,
            },
            other => other,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum GraphReason {
    WrongContext,
    WrongTarget,
    WrongOwner,
    OutOfBounds,
    InvalidEntry,
    EntryPredecessor,
    UnresolvedBlock,
    MissingTerminator,
    UnresolvedValue,
    DuplicateDefinition,
    DefinitionMismatch,
    TypeMismatch,
    ArityMismatch,
    UseBeforeDefinition,
    NonDominatingUse,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct GraphFailure {
    pub identity: Box<GraphIdentity>,
    pub location: GraphLocation,
    pub reason: GraphReason,
    pub origin: Option<Span>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum DefinitionSite {
    EntryInput(usize),
    Parameter {
        block: usize,
        ordinal: usize,
    },
    Result {
        block: usize,
        instruction: usize,
        ordinal: usize,
    },
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct ValueDescription<T> {
    pub ty: T,
    pub definition: Option<DefinitionSite>,
    pub origin: Option<Span>,
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct TypedUse<T> {
    pub value: usize,
    pub expected: Option<T>,
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct InstructionDescription<T> {
    pub uses: Vec<TypedUse<T>>,
    pub results: Vec<(usize, T)>,
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct EdgeDescription<T> {
    pub target: usize,
    pub arguments: Vec<TypedUse<T>>,
}
type TerminalDescription<T> = (Vec<TypedUse<T>>, Vec<EdgeDescription<T>>);

#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct BlockDescription<T> {
    pub parameters: Option<Vec<usize>>,
    pub instructions: Vec<InstructionDescription<T>>,
    /// None means a missing terminator, not a terminal without successors.
    pub terminal: Option<TerminalDescription<T>>,
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct GraphDescription<T> {
    pub entry: Option<usize>,
    pub inputs: Vec<(usize, T)>,
    pub values: Vec<ValueDescription<T>>,
    pub blocks: Vec<BlockDescription<T>>,
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) trait GraphView {
    type Type: Copy + Eq;
    fn identity(&self) -> GraphIdentity;
    /// Adapters validate stage-owned IDs before converting them to indices.
    fn describe(&self) -> Result<GraphDescription<Self::Type>, GraphFailure>;
}
