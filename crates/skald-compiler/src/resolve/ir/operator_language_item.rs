//! Validated canonical operator-protocol identities.

use crate::{
    identity::{InterfaceTemplateId, InterfaceTemplateRequirementId, TypeParameterId},
    source::Span,
};

/// One protocol in the compiler-recognized `std::ops` bundle.
///
/// The declaration order is the canonical diagnostic, dump, and table order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CanonicalOperatorProtocol {
    Neg,
    BitNot,
    Eq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
}

impl CanonicalOperatorProtocol {
    pub const ALL: [Self; 17] = [
        Self::Neg,
        Self::BitNot,
        Self::Eq,
        Self::Less,
        Self::LessEq,
        Self::Greater,
        Self::GreaterEq,
        Self::Add,
        Self::Sub,
        Self::Mul,
        Self::Div,
        Self::Rem,
        Self::BitAnd,
        Self::BitOr,
        Self::BitXor,
        Self::ShiftLeft,
        Self::ShiftRight,
    ];

    pub const COUNT: usize = Self::ALL.len();

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn interface_name(self) -> &'static str {
        match self {
            Self::Neg => "OpNeg",
            Self::BitNot => "OpBitNot",
            Self::Eq => "OpEq",
            Self::Less => "OpLess",
            Self::LessEq => "OpLessEq",
            Self::Greater => "OpGreater",
            Self::GreaterEq => "OpGreaterEq",
            Self::Add => "OpAdd",
            Self::Sub => "OpSub",
            Self::Mul => "OpMul",
            Self::Div => "OpDiv",
            Self::Rem => "OpRem",
            Self::BitAnd => "OpBitAnd",
            Self::BitOr => "OpBitOr",
            Self::BitXor => "OpBitXor",
            Self::ShiftLeft => "OpShiftLeft",
            Self::ShiftRight => "OpShiftRight",
        }
    }

    pub const fn requirement_name(self) -> &'static str {
        match self {
            Self::Neg => "op_neg",
            Self::BitNot => "op_bit_not",
            Self::Eq => "op_eq",
            Self::Less => "op_less",
            Self::LessEq => "op_less_eq",
            Self::Greater => "op_greater",
            Self::GreaterEq => "op_greater_eq",
            Self::Add => "op_add",
            Self::Sub => "op_sub",
            Self::Mul => "op_mul",
            Self::Div => "op_div",
            Self::Rem => "op_rem",
            Self::BitAnd => "op_bit_and",
            Self::BitOr => "op_bit_or",
            Self::BitXor => "op_bit_xor",
            Self::ShiftLeft => "op_shift_left",
            Self::ShiftRight => "op_shift_right",
        }
    }

    pub const fn shape(self) -> CanonicalOperatorProtocolShape {
        match self {
            Self::Neg | Self::BitNot => CanonicalOperatorProtocolShape::Unary,
            Self::Eq | Self::Less | Self::LessEq | Self::Greater | Self::GreaterEq => {
                CanonicalOperatorProtocolShape::Predicate
            }
            Self::Add
            | Self::Sub
            | Self::Mul
            | Self::Div
            | Self::Rem
            | Self::BitAnd
            | Self::BitOr
            | Self::BitXor
            | Self::ShiftLeft
            | Self::ShiftRight => CanonicalOperatorProtocolShape::Binary,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalOperatorProtocolShape {
    Unary,
    Predicate,
    Binary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedOperatorProtocolParameters {
    Unary {
        output: TypeParameterId,
    },
    Predicate {
        rhs: TypeParameterId,
    },
    Binary {
        rhs: TypeParameterId,
        output: TypeParameterId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedOperatorProtocol {
    pub kind: CanonicalOperatorProtocol,
    pub template: InterfaceTemplateId,
    pub parameters: ResolvedOperatorProtocolParameters,
    pub requirement: InterfaceTemplateRequirementId,
    pub declaration_span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedOperatorLanguageItem {
    protocols: [ResolvedOperatorProtocol; CanonicalOperatorProtocol::COUNT],
    pub requiring_spans: Vec<Span>,
}

impl ResolvedOperatorLanguageItem {
    pub(crate) fn new(
        protocols: Vec<ResolvedOperatorProtocol>,
        requiring_spans: Vec<Span>,
    ) -> Self {
        let protocols: [ResolvedOperatorProtocol; CanonicalOperatorProtocol::COUNT] =
            protocols.try_into().unwrap_or_else(|protocols: Vec<_>| {
                panic!(
                    "canonical operator bundle has {} protocols instead of {}",
                    protocols.len(),
                    CanonicalOperatorProtocol::COUNT
                )
            });
        assert!(
            protocols
                .iter()
                .zip(CanonicalOperatorProtocol::ALL)
                .all(|(protocol, kind)| protocol.kind == kind),
            "canonical operator protocols are out of order"
        );
        Self {
            protocols,
            requiring_spans,
        }
    }

    pub fn get(&self, kind: CanonicalOperatorProtocol) -> &ResolvedOperatorProtocol {
        &self.protocols[kind.index()]
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedOperatorProtocol> {
        self.protocols.iter()
    }
}
