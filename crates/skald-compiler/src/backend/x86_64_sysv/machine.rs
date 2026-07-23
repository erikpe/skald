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
    R11,
    Rbp,
    Rsp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ByteRegister {
    Al,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum XmmRegister {
    Xmm0,
    Xmm1,
    Xmm2,
    Xmm3,
    Xmm4,
    Xmm5,
    Xmm6,
    Xmm7,
    /// Caller-saved instruction-selection scratch registers.
    Xmm14,
    Xmm15,
}

impl XmmRegister {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Xmm0 => "%xmm0",
            Self::Xmm1 => "%xmm1",
            Self::Xmm2 => "%xmm2",
            Self::Xmm3 => "%xmm3",
            Self::Xmm4 => "%xmm4",
            Self::Xmm5 => "%xmm5",
            Self::Xmm6 => "%xmm6",
            Self::Xmm7 => "%xmm7",
            Self::Xmm14 => "%xmm14",
            Self::Xmm15 => "%xmm15",
        }
    }
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
            Self::R11 => "%r11",
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FloatOperand {
    Register(XmmRegister),
    Memory { base: Register, displacement: i32 },
}

impl From<XmmRegister> for FloatOperand {
    fn from(register: XmmRegister) -> Self {
        Self::Register(register)
    }
}

impl From<Register> for Operand {
    fn from(register: Register) -> Self {
        Self::Register(register)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Label(String);

impl Label {
    pub(super) fn new(name: String) -> Self {
        Self(name)
    }

    pub(super) fn name(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Instruction {
    Label(Label),
    Push(Register),
    Move {
        source: Operand,
        destination: Operand,
    },
    MoveByte {
        source: ByteRegister,
        destination: Operand,
    },
    LoadEffectiveAddress {
        source: Operand,
        destination: Register,
    },
    LoadSymbolAddress {
        symbol: String,
        destination: Register,
    },
    MoveImmediate64 {
        bits: u64,
        destination: Register,
    },
    MoveBitsToFloat {
        source: Register,
        destination: XmmRegister,
    },
    MoveFloat64 {
        source: FloatOperand,
        destination: FloatOperand,
    },
    ZeroExtendByte {
        source: ByteRegister,
        destination: Register,
    },
    LoadZeroExtendByte {
        source: Operand,
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
    AddFloat64 {
        source: XmmRegister,
        destination: XmmRegister,
    },
    SubtractFloat64 {
        source: XmmRegister,
        destination: XmmRegister,
    },
    MultiplyFloat64 {
        source: XmmRegister,
        destination: XmmRegister,
    },
    XorFloat128 {
        source: XmmRegister,
        destination: XmmRegister,
    },
    Test(Register),
    Compare {
        left: Register,
        right: Register,
    },
    ReserveStack(u32),
    ReleaseStack(u32),
    Call(String),
    CallIndirect(Register),
    Jump(Label),
    JumpIfNotZero(Label),
    JumpIfEqual(Label),
    Trap,
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
    pub dispatch_tables: Vec<AssemblyDispatchTable>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AssemblyDispatchTable {
    pub symbol: String,
    pub entries: Vec<Option<String>>,
}
