//! Source-level literal classifications shared by the frontend phases.
//!
//! A literal kind records syntax, not a converted semantic value. Keeping the
//! classification beside the original spelling lets later phases select
//! behavior without rediscovering suffixes or decimal forms from text.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericLiteralKind {
    I64,
    U64,
    U8,
    F64,
}
