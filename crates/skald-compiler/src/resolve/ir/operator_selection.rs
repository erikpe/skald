//! Canonical selections for overloadable operator punctuation.

use crate::{
    identity::{InterfaceId, InterfaceRequirementId},
    source::Span,
};

use super::{
    CanonicalOperatorProtocol, ResolvedBinaryOperator, ResolvedTypeKind, ResolvedUnaryOperator,
};

impl ResolvedUnaryOperator {
    /// Returns the canonical protocol selected by this syntax.
    ///
    /// Logical negation is deliberately absent: prefix `!` is never
    /// overloadable.
    pub const fn protocol(self) -> Option<CanonicalOperatorProtocol> {
        match self {
            Self::Negate => Some(CanonicalOperatorProtocol::Neg),
            Self::BitwiseComplement => Some(CanonicalOperatorProtocol::BitNot),
            Self::LogicalNot => None,
        }
    }

    pub const fn spelling(self) -> &'static str {
        match self {
            Self::Negate => "-",
            Self::LogicalNot => "!",
            Self::BitwiseComplement => "~",
        }
    }
}

impl ResolvedBinaryOperator {
    /// Returns the canonical protocol selected by this syntax.
    pub const fn protocol(self) -> CanonicalOperatorProtocol {
        match self {
            Self::Add => CanonicalOperatorProtocol::Add,
            Self::Subtract => CanonicalOperatorProtocol::Sub,
            Self::Multiply => CanonicalOperatorProtocol::Mul,
            Self::Divide => CanonicalOperatorProtocol::Div,
            Self::Remainder => CanonicalOperatorProtocol::Rem,
            Self::ShiftLeft => CanonicalOperatorProtocol::ShiftLeft,
            Self::ShiftRight => CanonicalOperatorProtocol::ShiftRight,
            Self::BitwiseAnd => CanonicalOperatorProtocol::BitAnd,
            Self::BitwiseOr => CanonicalOperatorProtocol::BitOr,
            Self::BitwiseXor => CanonicalOperatorProtocol::BitXor,
            Self::Equal | Self::NotEqual => CanonicalOperatorProtocol::Eq,
            Self::LessThan => CanonicalOperatorProtocol::Less,
            Self::LessEqual => CanonicalOperatorProtocol::LessEq,
            Self::GreaterThan => CanonicalOperatorProtocol::Greater,
            Self::GreaterEqual => CanonicalOperatorProtocol::GreaterEq,
        }
    }

    pub const fn spelling(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::Remainder => "%",
            Self::ShiftLeft => "<<",
            Self::ShiftRight => ">>",
            Self::BitwiseAnd => "&",
            Self::BitwiseOr => "|",
            Self::BitwiseXor => "^",
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::LessThan => "<",
            Self::LessEqual => "<=",
            Self::GreaterThan => ">",
            Self::GreaterEqual => ">=",
        }
    }
}

/// One exact canonical application selected for source punctuation.
///
/// This is resolution evidence only. Successful type checking erases it to an
/// ordinary interface call and no operator-specific node survives in HIR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedOperatorSelection {
    pub protocol: CanonicalOperatorProtocol,
    pub interface: InterfaceId,
    pub requirement: InterfaceRequirementId,
    pub rhs: Option<ResolvedTypeKind>,
    pub output: ResolvedTypeKind,
    pub origin_span: Span,
}

/// Candidate resolution for one overloadable class/interface expression.
///
/// Keeping zero and multiple candidates in resolved IR lets type checking
/// report operator failures alongside independent primitive type errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedOperatorResolution {
    pub protocol: CanonicalOperatorProtocol,
    pub candidates: Vec<ResolvedOperatorSelection>,
    /// Canonical applications rejected only by read-only RHS alias binding.
    ///
    /// These are not selectable candidates. Retaining their stable origins
    /// lets type checking distinguish an incompatible RHS from an operand
    /// type with no canonical application at all.
    pub incompatible_rhs: Vec<ResolvedOperatorSelection>,
}

impl ResolvedOperatorResolution {
    pub fn selected(&self) -> Option<ResolvedOperatorSelection> {
        let [selection] = self.candidates.as_slice() else {
            return None;
        };
        Some(*selection)
    }
}
