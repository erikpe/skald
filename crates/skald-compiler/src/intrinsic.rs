//! Compiler-known function identities that have source declarations but no
//! Skald body or foreign linkage.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Intrinsic {
    Panic,
    IoStandardHandle,
    IoOpen,
    IoRead,
    IoWrite,
    IoClose,
    F64ToBits,
    F64FromBits,
}
