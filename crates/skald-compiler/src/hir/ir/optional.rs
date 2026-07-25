//! Typed primitive-optional storage operations.

use crate::{identity::BindingId, source::Span};

use super::HirExpression;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirPrimitiveType {
    I64,
    U64,
    U8,
    F64,
    Bool,
}

impl HirPrimitiveType {
    pub const fn payload_type(self) -> super::Type {
        match self {
            Self::I64 => super::Type::I64,
            Self::U64 => super::Type::U64,
            Self::U8 => super::Type::U8,
            Self::F64 => super::Type::F64,
            Self::Bool => super::Type::Bool,
        }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirOptionalPlace {
    pub binding: BindingId,
    pub payload: HirPrimitiveType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirOptionalSource {
    Absent { span: Span },
    Present(HirExpression),
    Copy(HirOptionalPlace),
}

impl HirOptionalSource {
    pub const fn span(&self) -> Span {
        match self {
            Self::Absent { span } => *span,
            Self::Present(expression) => expression.span,
            Self::Copy(place) => place.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirOptionalAssignment {
    pub destination: BindingId,
    pub payload: HirPrimitiveType,
    pub source: HirOptionalSource,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirPresenceTestKind {
    Some,
    None,
}
