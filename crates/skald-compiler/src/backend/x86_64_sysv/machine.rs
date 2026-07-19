//! Small target-specific assembly model.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Register {
    Rax,
    Rcx,
    Rdx,
    Rsi,
    Rdi,
    R8,
    R9,
    Rbp,
    Rsp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ByteRegister {
    Al,
}

impl ByteRegister {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Al => "%al",
        }
    }
}

impl Register {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Rax => "%rax",
            Self::Rcx => "%rcx",
            Self::Rdx => "%rdx",
            Self::Rsi => "%rsi",
            Self::Rdi => "%rdi",
            Self::R8 => "%r8",
            Self::R9 => "%r9",
            Self::Rbp => "%rbp",
            Self::Rsp => "%rsp",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Operand {
    Register(Register),
    Memory { base: Register, displacement: i32 },
}

impl From<Register> for Operand {
    fn from(register: Register) -> Self {
        Self::Register(register)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Instruction {
    Push(Register),
    Move {
        source: Operand,
        destination: Operand,
    },
    MoveImmediate64 {
        value: i64,
        destination: Register,
    },
    ZeroExtendByte {
        source: ByteRegister,
        destination: Register,
    },
    Add {
        source: Register,
        destination: Register,
    },
    Subtract {
        source: Register,
        destination: Register,
    },
    Multiply {
        source: Register,
        destination: Register,
    },
    Negate(Register),
    ReserveStack(u32),
    ReleaseStack(u32),
    Call(String),
    Leave,
    Return,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AssemblyFunction {
    pub symbol: String,
    pub exported: bool,
    pub instructions: Vec<Instruction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AssemblyProgram {
    pub functions: Vec<AssemblyFunction>,
}
