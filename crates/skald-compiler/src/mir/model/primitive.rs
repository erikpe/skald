//! Primitive value types and explicit cast semantics.

use super::{MirIntegerType, MirType};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirPrimitiveType {
    I64,
    U64,
    U8,
    F64,
    Bool,
}

impl MirPrimitiveType {
    pub const fn from_type(ty: MirType) -> Option<Self> {
        match ty {
            MirType::I64 => Some(Self::I64),
            MirType::U64 => Some(Self::U64),
            MirType::U8 => Some(Self::U8),
            MirType::F64 => Some(Self::F64),
            MirType::Bool => Some(Self::Bool),
            _ => None,
        }
    }

    pub const fn value_type(self) -> MirType {
        match self {
            Self::I64 => MirType::I64,
            Self::U64 => MirType::U64,
            Self::U8 => MirType::U8,
            Self::F64 => MirType::F64,
            Self::Bool => MirType::Bool,
        }
    }

    pub const fn payload_type(self) -> MirType {
        self.value_type()
    }

    pub const fn integer_type(self) -> Option<MirIntegerType> {
        match self {
            Self::I64 => Some(MirIntegerType::I64),
            Self::U64 => Some(MirIntegerType::U64),
            Self::U8 => Some(MirIntegerType::U8),
            Self::F64 | Self::Bool => None,
        }
    }

    pub const fn is_integer(self) -> bool {
        self.integer_type().is_some()
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::U8 => "u8",
            Self::F64 => "f64",
            Self::Bool => "bool",
        }
    }
}

impl From<MirIntegerType> for MirPrimitiveType {
    fn from(integer: MirIntegerType) -> Self {
        match integer {
            MirIntegerType::I64 => Self::I64,
            MirIntegerType::U64 => Self::U64,
            MirIntegerType::U8 => Self::U8,
        }
    }
}

impl std::fmt::Display for MirPrimitiveType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.name())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirPrimitiveCastKind {
    Identity,
    IntegerBits,
    ToBool,
    ToF64,
    FromBool,
    CheckedF64ToInteger,
}

impl MirPrimitiveCastKind {
    pub const fn may_terminate(self) -> bool {
        matches!(self, Self::CheckedF64ToInteger)
    }

    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::IntegerBits => "integer_bits",
            Self::ToBool => "to_bool",
            Self::ToF64 => "to_f64",
            Self::FromBool => "from_bool",
            Self::CheckedF64ToInteger => "checked_f64_to_integer",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MirPrimitiveCast {
    pub source: MirPrimitiveType,
    pub target: MirPrimitiveType,
    kind: MirPrimitiveCastKind,
}

impl MirPrimitiveCast {
    pub fn new(source: MirPrimitiveType, target: MirPrimitiveType) -> Self {
        let kind = if source == target {
            MirPrimitiveCastKind::Identity
        } else if source.is_integer() && target.is_integer() {
            MirPrimitiveCastKind::IntegerBits
        } else if target == MirPrimitiveType::Bool {
            MirPrimitiveCastKind::ToBool
        } else if target == MirPrimitiveType::F64 {
            MirPrimitiveCastKind::ToF64
        } else if source == MirPrimitiveType::Bool {
            MirPrimitiveCastKind::FromBool
        } else {
            assert!(
                source == MirPrimitiveType::F64 && target.is_integer(),
                "unclassified primitive cast pair"
            );
            MirPrimitiveCastKind::CheckedF64ToInteger
        };
        Self {
            source,
            target,
            kind,
        }
    }

    pub const fn source_type(self) -> MirType {
        self.source.value_type()
    }

    pub const fn kind(self) -> MirPrimitiveCastKind {
        self.kind
    }

    pub const fn result_type(self) -> MirType {
        self.target.value_type()
    }

    pub const fn may_terminate(self) -> bool {
        self.kind.may_terminate()
    }

    pub fn is_semantically_consistent(self) -> bool {
        Self::new(self.source, self.target).kind == self.kind
    }

    #[cfg(test)]
    pub(crate) fn set_kind_for_test(&mut self, kind: MirPrimitiveCastKind) {
        self.kind = kind;
    }
}
