//! Primitive realizations selected through exact canonical generic bounds.

use super::{ResolvedPrimitiveOperatorOperation, ResolvedPrimitiveType};

/// One compiler-provided operation selected by a bound-member call.
///
/// This is compile-time evidence only. It does not imply primitive interface
/// conformance and never reaches executable IR as a protocol operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedPrimitiveBoundOperation {
    Operator(ResolvedPrimitiveOperatorOperation),
    Successor(ResolvedPrimitiveType),
}

impl ResolvedPrimitiveBoundOperation {
    pub(crate) fn semantic_name(self) -> String {
        match self {
            Self::Operator(operation) => operation.semantic_name(),
            Self::Successor(primitive) => format!("AddOne{}", primitive_suffix(primitive)),
        }
    }
}

const fn primitive_suffix(primitive: ResolvedPrimitiveType) -> &'static str {
    match primitive {
        ResolvedPrimitiveType::I64 => "I64",
        ResolvedPrimitiveType::U64 => "U64",
        ResolvedPrimitiveType::U8 => "U8",
        ResolvedPrimitiveType::F64 => "F64",
        ResolvedPrimitiveType::Bool => "Bool",
    }
}
