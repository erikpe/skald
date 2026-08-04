//! Source-level literal classifications shared by the frontend phases.
//!
//! A literal kind records syntax, not a converted semantic value. Keeping the
//! classification beside the original spelling lets later phases select
//! behavior without rediscovering suffixes or radix from text.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegerRadix {
    Decimal,
    Hexadecimal,
}

impl IntegerRadix {
    pub(crate) const fn base(self) -> u32 {
        match self {
            Self::Decimal => 10,
            Self::Hexadecimal => 16,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericLiteralKind {
    I64(IntegerRadix),
    U64(IntegerRadix),
    U8(IntegerRadix),
    F64,
}
