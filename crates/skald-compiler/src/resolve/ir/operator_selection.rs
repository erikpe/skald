//! Definition-site selections for value-producing operator punctuation.

use crate::{
    identity::{InterfaceId, InterfaceRequirementId},
    source::Span,
};

use super::{
    CanonicalOperatorProtocol, ResolvedBinaryOperator, ResolvedTypeKind, ResolvedUnaryOperator,
};

impl ResolvedUnaryOperator {
    /// Returns the canonical value-producing protocol selected by this syntax.
    ///
    /// Logical negation is deliberately absent: prefix `!` is never
    /// overloadable.
    pub const fn value_protocol(self) -> Option<CanonicalOperatorProtocol> {
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
    /// Returns the canonical value-producing protocol selected by this syntax.
    ///
    /// Predicate protocols are introduced by the following roadmap slice and
    /// therefore remain absent here.
    pub const fn value_protocol(self) -> Option<CanonicalOperatorProtocol> {
        match self {
            Self::Add => Some(CanonicalOperatorProtocol::Add),
            Self::Subtract => Some(CanonicalOperatorProtocol::Sub),
            Self::Multiply => Some(CanonicalOperatorProtocol::Mul),
            Self::Divide => Some(CanonicalOperatorProtocol::Div),
            Self::Remainder => Some(CanonicalOperatorProtocol::Rem),
            Self::ShiftLeft => Some(CanonicalOperatorProtocol::ShiftLeft),
            Self::ShiftRight => Some(CanonicalOperatorProtocol::ShiftRight),
            Self::BitwiseAnd => Some(CanonicalOperatorProtocol::BitAnd),
            Self::BitwiseOr => Some(CanonicalOperatorProtocol::BitOr),
            Self::BitwiseXor => Some(CanonicalOperatorProtocol::BitXor),
            Self::Equal
            | Self::NotEqual
            | Self::LessThan
            | Self::LessEqual
            | Self::GreaterThan
            | Self::GreaterEqual => None,
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
pub struct ResolvedValueOperatorSelection {
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
pub struct ResolvedValueOperatorResolution {
    pub protocol: CanonicalOperatorProtocol,
    pub candidates: Vec<ResolvedValueOperatorSelection>,
}

impl ResolvedValueOperatorResolution {
    pub fn selected(&self) -> Option<ResolvedValueOperatorSelection> {
        let [selection] = self.candidates.as_slice() else {
            return None;
        };
        Some(*selection)
    }
}
