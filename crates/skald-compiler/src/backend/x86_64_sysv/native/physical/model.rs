use super::super::{selected::Instruction as SelectedInstruction, Gpr};
use crate::backend::{
    frame::{FrameError, FramePlan},
    graph::SelectedBlockId,
    placement::{Site, TransferPoint},
    plan::ArtifactId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Register {
    Gpr(Gpr),
    Xmm(u8),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Operand {
    Register(Register),
    Memory { base: Gpr, displacement: i32 },
    Immediate(u64),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MoveKind {
    Integer,
    Float,
    Bits,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Alu {
    Add,
    Subtract,
    Multiply,
    And,
    Or,
    Xor,
    DivideFloat,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Unary {
    Negate,
    Complement,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Convert {
    ZeroExtend,
    SignedToFloat,
    TruncateFloat,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Condition {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Below,
    BelowEqual,
    Above,
    AboveEqual,
    Parity,
    NotParity,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Shift {
    Left,
    ArithmeticRight,
    LogicalRight,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct BlockId(pub usize);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CallTarget {
    Direct(ArtifactId),
    Indirect(Register),
}
/// Closed physical instruction vocabulary: no values, objects, homes or string opcodes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Instruction {
    Move {
        kind: MoveKind,
        bits: u16,
        source: Operand,
        destination: Operand,
    },
    Alu {
        op: Alu,
        float: bool,
        bits: u16,
        source: Operand,
        destination: Register,
    },
    Unary {
        op: Unary,
        bits: u16,
        destination: Register,
    },
    Compare {
        float: bool,
        bits: u16,
        left: Operand,
        right: Operand,
    },
    Set {
        condition: Condition,
        destination: Register,
    },
    Convert {
        op: Convert,
        input_bits: u16,
        output_bits: u16,
        source: Register,
        destination: Register,
    },
    DividendSignExtend,
    Divide {
        signed: bool,
        divisor: Register,
    },
    Shift {
        op: Shift,
        bits: u16,
        destination: Register,
        variable: bool,
    },
    Lea {
        source: Operand,
        destination: Register,
    },
    RipAddress {
        artifact: ArtifactId,
        destination: Register,
    },
    TlsBase {
        destination: Register,
    },
    TlsOffset {
        artifact: ArtifactId,
        destination: Register,
    },
    Call(CallTarget),
    Jump(BlockId),
    JumpIf {
        condition: Condition,
        destination: BlockId,
    },
    Push(Gpr),
    Pop(Gpr),
    StackSubtract(u32),
    Return,
    HardTrap,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Origin {
    Prologue,
    Selected(Site),
    Transfer { point: TransferPoint, index: usize },
    Epilogue(Site),
    Forward { block: SelectedBlockId, slot: usize },
}
#[derive(Clone, Debug)]
pub(super) struct Group {
    #[cfg_attr(not(test), allow(dead_code))]
    pub origin: Origin,
    #[cfg_attr(not(test), allow(dead_code))]
    pub instructions: Vec<Instruction>,
    /// Declared metadata dependencies, including zero-code attribution references.
    #[cfg_attr(not(test), allow(dead_code))]
    pub dependencies: Vec<ArtifactId>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BlockOrigin {
    Entry,
    Selected(SelectedBlockId),
    Forward { block: SelectedBlockId, slot: usize },
}
pub(super) struct Block {
    #[cfg_attr(not(test), allow(dead_code))]
    pub id: BlockId,
    #[cfg_attr(not(test), allow(dead_code))]
    pub origin: BlockOrigin,
    pub groups: Vec<Group>,
}
pub(in crate::backend) struct PhysicalDraft<'a, 'f, 's, 'p> {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) frame: &'a FramePlan<'f, 's, 'p, SelectedInstruction>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) entry: BlockId,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) blocks: Vec<Block>,
}
#[derive(Debug, Eq, PartialEq)]
pub(in crate::backend) enum RealizeError {
    WrongSelected,
    Frame(FrameError),
    MissingOperand(Site, usize),
    InvalidResource,
    UnsupportedRecipe(Site),
    RecipeBound(Site),
    Overflow,
}
impl From<FrameError> for RealizeError {
    fn from(value: FrameError) -> Self {
        Self::Frame(value)
    }
}
