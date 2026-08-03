//! Primitive value types and explicit cast selection.

use super::{HirIntegerType, Type};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirPrimitiveType {
    I64,
    U64,
    U8,
    F64,
    Bool,
}

impl HirPrimitiveType {
    pub const fn from_type(ty: Type) -> Option<Self> {
        match ty {
            Type::I64 => Some(Self::I64),
            Type::U64 => Some(Self::U64),
            Type::U8 => Some(Self::U8),
            Type::F64 => Some(Self::F64),
            Type::Bool => Some(Self::Bool),
            _ => None,
        }
    }

    pub const fn value_type(self) -> Type {
        match self {
            Self::I64 => Type::I64,
            Self::U64 => Type::U64,
            Self::U8 => Type::U8,
            Self::F64 => Type::F64,
            Self::Bool => Type::Bool,
        }
    }

    pub const fn payload_type(self) -> Type {
        self.value_type()
    }

    pub const fn integer_type(self) -> Option<HirIntegerType> {
        match self {
            Self::I64 => Some(HirIntegerType::I64),
            Self::U64 => Some(HirIntegerType::U64),
            Self::U8 => Some(HirIntegerType::U8),
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

impl From<HirIntegerType> for HirPrimitiveType {
    fn from(integer: HirIntegerType) -> Self {
        match integer {
            HirIntegerType::I64 => Self::I64,
            HirIntegerType::U64 => Self::U64,
            HirIntegerType::U8 => Self::U8,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirPrimitiveCastKind {
    Identity,
    IntegerBits,
    ToBool,
    ToF64,
    FromBool,
    BitReinterpretation,
    CheckedF64ToInteger,
}

impl HirPrimitiveCastKind {
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
            Self::BitReinterpretation => "bit_reinterpretation",
            Self::CheckedF64ToInteger => "checked_f64_to_integer",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HirPrimitiveCast {
    pub source: HirPrimitiveType,
    pub target: HirPrimitiveType,
    kind: HirPrimitiveCastKind,
}

impl HirPrimitiveCast {
    pub fn new(source: HirPrimitiveType, target: HirPrimitiveType) -> Self {
        let kind = if source == target {
            HirPrimitiveCastKind::Identity
        } else if source.is_integer() && target.is_integer() {
            HirPrimitiveCastKind::IntegerBits
        } else if target == HirPrimitiveType::Bool {
            HirPrimitiveCastKind::ToBool
        } else if target == HirPrimitiveType::F64 {
            HirPrimitiveCastKind::ToF64
        } else if source == HirPrimitiveType::Bool {
            HirPrimitiveCastKind::FromBool
        } else {
            assert!(
                source == HirPrimitiveType::F64 && target.is_integer(),
                "unclassified primitive cast pair"
            );
            HirPrimitiveCastKind::CheckedF64ToInteger
        };
        Self {
            source,
            target,
            kind,
        }
    }

    pub const fn source_type(self) -> Type {
        self.source.value_type()
    }

    pub fn bit_reinterpretation(source: HirPrimitiveType, target: HirPrimitiveType) -> Self {
        assert!(
            matches!(
                (source, target),
                (HirPrimitiveType::F64, HirPrimitiveType::U64)
                    | (HirPrimitiveType::U64, HirPrimitiveType::F64)
            ),
            "bit reinterpretation is defined only between f64 and u64"
        );
        Self {
            source,
            target,
            kind: HirPrimitiveCastKind::BitReinterpretation,
        }
    }

    pub const fn kind(self) -> HirPrimitiveCastKind {
        self.kind
    }

    pub const fn result_type(self) -> Type {
        self.target.value_type()
    }

    pub const fn may_terminate(self) -> bool {
        self.kind.may_terminate()
    }
}
