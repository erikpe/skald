//! Checked integer division and remainder selection.

use super::{HirExpression, HirIntegerType, Type};

/// One typed integer division-family expression.
///
/// The operation records Skald semantics rather than a target divide
/// instruction. In particular, signed quotient rounding and the
/// signed-minimum pair remain explicit properties of the selected operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCheckedIntegerDivision {
    pub operation: HirIntegerDivisionOperation,
    pub dividend: Box<HirExpression>,
    pub divisor: Box<HirExpression>,
}

impl HirCheckedIntegerDivision {
    pub fn new(
        operation: HirIntegerDivisionOperation,
        dividend: HirExpression,
        divisor: HirExpression,
    ) -> Self {
        assert_eq!(
            dividend.ty,
            operation.operand_type(),
            "typed integer dividend must match its selected integer kind"
        );
        assert_eq!(
            divisor.ty,
            operation.operand_type(),
            "typed integer divisor must match its selected integer kind"
        );
        Self {
            operation,
            dividend: Box::new(dividend),
            divisor: Box::new(divisor),
        }
    }

    pub fn validate(&self, result_type: Type) {
        assert_eq!(self.dividend.ty, self.operation.operand_type());
        assert_eq!(self.divisor.ty, self.operation.operand_type());
        assert_eq!(result_type, self.operation.result_type());
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HirIntegerDivisionOperation {
    pub kind: HirIntegerDivisionKind,
    pub operand: HirIntegerType,
}

impl HirIntegerDivisionOperation {
    pub const fn operand_type(self) -> Type {
        self.operand.operand_type()
    }

    pub const fn result_type(self) -> Type {
        self.operand_type()
    }

    pub const fn failure(self) -> HirIntegerDivisionFailure {
        match self.kind {
            HirIntegerDivisionKind::Quotient => HirIntegerDivisionFailure::DivisionByZero,
            HirIntegerDivisionKind::Remainder => HirIntegerDivisionFailure::RemainderByZero,
        }
    }

    pub const fn mnemonic(self) -> &'static str {
        match self.kind {
            HirIntegerDivisionKind::Quotient => "div",
            HirIntegerDivisionKind::Remainder => "rem",
        }
    }

    pub const fn signed_semantics(self) -> Option<HirSignedIntegerDivisionSemantics> {
        match self.operand {
            HirIntegerType::I64 => Some(HirSignedIntegerDivisionSemantics {
                quotient_rounding: HirSignedQuotientRounding::TowardNegativeInfinity,
                remainder_sign: HirSignedRemainderSign::Divisor,
                minimum_pair_result: match self.kind {
                    HirIntegerDivisionKind::Quotient => HirSignedMinimumPairResult::Minimum,
                    HirIntegerDivisionKind::Remainder => HirSignedMinimumPairResult::Zero,
                },
            }),
            HirIntegerType::U64 | HirIntegerType::U8 => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirIntegerDivisionKind {
    Quotient,
    Remainder,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirIntegerDivisionFailure {
    DivisionByZero,
    RemainderByZero,
}

impl HirIntegerDivisionFailure {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::DivisionByZero => "integer-division-by-zero",
            Self::RemainderByZero => "integer-remainder-by-zero",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HirSignedIntegerDivisionSemantics {
    pub quotient_rounding: HirSignedQuotientRounding,
    pub remainder_sign: HirSignedRemainderSign,
    pub minimum_pair_result: HirSignedMinimumPairResult,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirSignedQuotientRounding {
    TowardNegativeInfinity,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirSignedRemainderSign {
    Divisor,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirSignedMinimumPairResult {
    Minimum,
    Zero,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_matrix_preserves_exact_types_failures_and_signed_semantics() {
        for operand in [HirIntegerType::I64, HirIntegerType::U64, HirIntegerType::U8] {
            for kind in [
                HirIntegerDivisionKind::Quotient,
                HirIntegerDivisionKind::Remainder,
            ] {
                let operation = HirIntegerDivisionOperation { kind, operand };
                assert_eq!(operation.operand_type(), operand.operand_type());
                assert_eq!(operation.result_type(), operand.operand_type());
                assert!(matches!(operation.mnemonic(), "div" | "rem"));
                assert!(matches!(
                    operation.failure(),
                    HirIntegerDivisionFailure::DivisionByZero
                        | HirIntegerDivisionFailure::RemainderByZero
                ));

                match operand {
                    HirIntegerType::I64 => {
                        let semantics = operation.signed_semantics().unwrap();
                        assert_eq!(
                            semantics.quotient_rounding,
                            HirSignedQuotientRounding::TowardNegativeInfinity
                        );
                        assert_eq!(semantics.remainder_sign, HirSignedRemainderSign::Divisor);
                        assert_eq!(
                            semantics.minimum_pair_result,
                            match kind {
                                HirIntegerDivisionKind::Quotient => {
                                    HirSignedMinimumPairResult::Minimum
                                }
                                HirIntegerDivisionKind::Remainder => {
                                    HirSignedMinimumPairResult::Zero
                                }
                            }
                        );
                    }
                    HirIntegerType::U64 | HirIntegerType::U8 => {
                        assert_eq!(operation.signed_semantics(), None);
                    }
                }
            }
        }
    }

    #[test]
    fn failures_have_distinct_deterministic_vocabulary() {
        assert_eq!(
            HirIntegerDivisionFailure::DivisionByZero.mnemonic(),
            "integer-division-by-zero"
        );
        assert_eq!(
            HirIntegerDivisionFailure::RemainderByZero.mnemonic(),
            "integer-remainder-by-zero"
        );
    }
}
