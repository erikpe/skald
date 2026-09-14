//! Phase-neutral primitive comparison semantics shared by typed IRs.

/// The relation tested by a primitive comparison.
///
/// HIR and MIR retain their own operand and operation types. This descriptor is
/// shared because its variants, stable dump mnemonic, and equality
/// classification have the same meaning in both representations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PrimitiveComparisonPredicate {
    Equal,
    NotEqual,
    LessThan,
    LessEqual,
    GreaterThan,
    GreaterEqual,
}

impl PrimitiveComparisonPredicate {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::Equal => "eq",
            Self::NotEqual => "ne",
            Self::LessThan => "lt",
            Self::LessEqual => "le",
            Self::GreaterThan => "gt",
            Self::GreaterEqual => "ge",
        }
    }

    pub const fn is_equality(self) -> bool {
        matches!(self, Self::Equal | Self::NotEqual)
    }
}
