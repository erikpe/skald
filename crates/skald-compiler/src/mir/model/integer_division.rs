//! Target-independent checked integer division and remainder semantics.

use super::{MirIntegerType, MirTerminationReason, MirType, StorageId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MirIntegerDivisionOperation {
    pub kind: MirIntegerDivisionKind,
    pub operand: MirIntegerType,
}

impl MirIntegerDivisionOperation {
    pub const fn operand_type(self) -> MirType {
        self.operand.operand_type()
    }

    pub const fn result_type(self) -> MirType {
        self.operand_type()
    }

    pub const fn failure_reason(self) -> MirTerminationReason {
        match self.kind {
            MirIntegerDivisionKind::Quotient => MirTerminationReason::IntegerDivisionByZero,
            MirIntegerDivisionKind::Remainder => MirTerminationReason::IntegerRemainderByZero,
        }
    }

    pub const fn mnemonic(self) -> &'static str {
        match self.kind {
            MirIntegerDivisionKind::Quotient => "div",
            MirIntegerDivisionKind::Remainder => "rem",
        }
    }

    pub const fn signed_semantics(self) -> Option<MirSignedIntegerDivisionSemantics> {
        match self.operand {
            MirIntegerType::I64 => Some(MirSignedIntegerDivisionSemantics {
                quotient_rounding: MirSignedQuotientRounding::TowardNegativeInfinity,
                remainder_sign: MirSignedRemainderSign::Divisor,
                minimum_pair_result: match self.kind {
                    MirIntegerDivisionKind::Quotient => MirSignedMinimumPairResult::Minimum,
                    MirIntegerDivisionKind::Remainder => MirSignedMinimumPairResult::Zero,
                },
            }),
            MirIntegerType::U64 | MirIntegerType::U8 => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirIntegerDivisionKind {
    Quotient,
    Remainder,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MirSignedIntegerDivisionSemantics {
    pub quotient_rounding: MirSignedQuotientRounding,
    pub remainder_sign: MirSignedRemainderSign,
    pub minimum_pair_result: MirSignedMinimumPairResult,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirSignedQuotientRounding {
    TowardNegativeInfinity,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirSignedRemainderSign {
    Divisor,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirSignedMinimumPairResult {
    Minimum,
    Zero,
}

/// Exact scalar carriers participating in one checked division/remainder
/// diamond.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirIntegerDivisorCheck {
    pub operation: MirIntegerDivisionOperation,
    pub dividend: StorageId,
    pub divisor: StorageId,
    pub result: StorageId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_matrix_preserves_exact_types_failures_and_signed_semantics() {
        for operand in [MirIntegerType::I64, MirIntegerType::U64, MirIntegerType::U8] {
            for kind in [
                MirIntegerDivisionKind::Quotient,
                MirIntegerDivisionKind::Remainder,
            ] {
                let operation = MirIntegerDivisionOperation { kind, operand };
                assert_eq!(operation.operand_type(), operand.operand_type());
                assert_eq!(operation.result_type(), operand.operand_type());
                assert!(matches!(operation.mnemonic(), "div" | "rem"));
                assert_eq!(
                    operation.failure_reason(),
                    match kind {
                        MirIntegerDivisionKind::Quotient => {
                            MirTerminationReason::IntegerDivisionByZero
                        }
                        MirIntegerDivisionKind::Remainder => {
                            MirTerminationReason::IntegerRemainderByZero
                        }
                    }
                );

                match operand {
                    MirIntegerType::I64 => {
                        let semantics = operation.signed_semantics().unwrap();
                        assert_eq!(
                            semantics.quotient_rounding,
                            MirSignedQuotientRounding::TowardNegativeInfinity
                        );
                        assert_eq!(semantics.remainder_sign, MirSignedRemainderSign::Divisor);
                        assert_eq!(
                            semantics.minimum_pair_result,
                            match kind {
                                MirIntegerDivisionKind::Quotient => {
                                    MirSignedMinimumPairResult::Minimum
                                }
                                MirIntegerDivisionKind::Remainder => {
                                    MirSignedMinimumPairResult::Zero
                                }
                            }
                        );
                    }
                    MirIntegerType::U64 | MirIntegerType::U8 => {
                        assert_eq!(operation.signed_semantics(), None);
                    }
                }
            }
        }
    }
}
