//! Numeric literal conversion and range diagnostics.

use crate::{
    diagnostics::Diagnostic,
    hir::{HirExpression, HirExpressionKind, Type},
    literal::{IntegerRadix, NumericLiteralKind},
    resolve::{ResolvedExpression, ResolvedNumericLiteralExpr},
    source::Span,
};

use super::{
    function::CallableChecker,
    program::{
        F64_LITERAL_OUT_OF_RANGE, INTEGER_LITERAL_OUT_OF_RANGE, U64_LITERAL_OUT_OF_RANGE,
        U8_LITERAL_OUT_OF_RANGE,
    },
};

impl CallableChecker<'_, '_> {
    pub(super) fn check_numeric_literal(
        &mut self,
        literal: &ResolvedNumericLiteralExpr,
    ) -> Option<HirExpression> {
        match literal.kind {
            NumericLiteralKind::I64(_) => self.check_positive_i64_literal(literal),
            NumericLiteralKind::U64(_) => self.check_u64_literal(literal),
            NumericLiteralKind::U8(_) => self.check_u8_literal(literal),
            NumericLiteralKind::F64 => self.check_f64_literal(literal),
        }
    }

    fn check_positive_i64_literal(
        &mut self,
        literal: &ResolvedNumericLiteralExpr,
    ) -> Option<HirExpression> {
        match parse_integer_magnitude(literal).and_then(|value| i64::try_from(value).ok()) {
            Some(value) => Some(HirExpression {
                kind: HirExpressionKind::I64(value),
                ty: Type::I64,
                span: literal.span,
            }),
            None => {
                self.report_integer_out_of_range(literal.span, literal.spelling.clone());
                None
            }
        }
    }

    fn check_u64_literal(&mut self, literal: &ResolvedNumericLiteralExpr) -> Option<HirExpression> {
        match parse_integer_magnitude(literal).and_then(|value| u64::try_from(value).ok()) {
            Some(value) => Some(HirExpression {
                kind: HirExpressionKind::U64(value),
                ty: Type::U64,
                span: literal.span,
            }),
            None => {
                self.diagnostics.push(
                    Diagnostic::error(
                        U64_LITERAL_OUT_OF_RANGE,
                        format!(
                            "integer literal `{}` is out of range for `u64`",
                            literal.spelling
                        ),
                    )
                    .with_primary_label(literal.span, "value is not representable as `u64`")
                    .with_note(format!(
                        "the inclusive `u64` range is 0 through {}",
                        u64::MAX
                    )),
                );
                None
            }
        }
    }

    fn check_u8_literal(&mut self, literal: &ResolvedNumericLiteralExpr) -> Option<HirExpression> {
        match parse_integer_magnitude(literal).and_then(|value| u8::try_from(value).ok()) {
            Some(value) => Some(HirExpression {
                kind: HirExpressionKind::U8(value),
                ty: Type::U8,
                span: literal.span,
            }),
            None => {
                self.diagnostics.push(
                    Diagnostic::error(
                        U8_LITERAL_OUT_OF_RANGE,
                        format!(
                            "integer literal `{}` is out of range for `u8`",
                            literal.spelling
                        ),
                    )
                    .with_primary_label(literal.span, "value is not representable as `u8`")
                    .with_note(format!("the inclusive `u8` range is 0 through {}", u8::MAX)),
                );
                None
            }
        }
    }

    fn check_f64_literal(&mut self, literal: &ResolvedNumericLiteralExpr) -> Option<HirExpression> {
        let value = literal
            .spelling
            .parse::<f64>()
            .expect("validated decimal f64 literal must parse");
        if value.is_finite() {
            Some(HirExpression {
                kind: HirExpressionKind::F64Bits(value.to_bits()),
                ty: Type::F64,
                span: literal.span,
            })
        } else {
            self.diagnostics.push(
                Diagnostic::error(
                    F64_LITERAL_OUT_OF_RANGE,
                    format!(
                        "floating literal `{}` is out of range for `f64`",
                        literal.spelling
                    ),
                )
                .with_primary_label(literal.span, "value rounds to infinity")
                .with_note("finite `f64` literals must round to a finite IEEE-754 binary64 value"),
            );
            None
        }
    }

    pub(super) fn report_integer_out_of_range(&mut self, span: Span, spelling: String) {
        self.diagnostics.push(
            Diagnostic::error(
                INTEGER_LITERAL_OUT_OF_RANGE,
                format!("integer literal `{spelling}` is out of range for `i64`"),
            )
            .with_primary_label(span, "value is not representable as `i64`")
            .with_note(format!(
                "the inclusive `i64` range is {} through {}",
                i64::MIN,
                i64::MAX
            )),
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Magnitude {
    PositiveI64,
    MinimumBoundary,
    TooLarge,
}

pub(super) fn classify_i64_magnitude(literal: &ResolvedNumericLiteralExpr) -> Magnitude {
    let NumericLiteralKind::I64(_) = literal.kind else {
        unreachable!("i64 magnitude classification requires an i64 literal");
    };
    let Some(magnitude) = parse_integer_magnitude(literal) else {
        return Magnitude::TooLarge;
    };
    let minimum_magnitude = (i64::MAX as u128) + 1;
    if magnitude <= i64::MAX as u128 {
        Magnitude::PositiveI64
    } else if magnitude == minimum_magnitude {
        Magnitude::MinimumBoundary
    } else {
        Magnitude::TooLarge
    }
}

fn parse_integer_magnitude(literal: &ResolvedNumericLiteralExpr) -> Option<u128> {
    let (radix, suffix) = match literal.kind {
        NumericLiteralKind::I64(radix) => (radix, ""),
        NumericLiteralKind::U64(radix) => (radix, "u"),
        NumericLiteralKind::U8(radix) => (radix, "u8"),
        NumericLiteralKind::F64 => {
            unreachable!("integer magnitude parsing requires an integer literal")
        }
    };
    let unsuffixed = if suffix.is_empty() {
        literal.spelling.as_str()
    } else {
        literal
            .spelling
            .strip_suffix(suffix)
            .expect("validated integer literal must have its classified suffix")
    };
    let digits = match radix {
        IntegerRadix::Decimal => unsuffixed,
        IntegerRadix::Hexadecimal => unsuffixed
            .strip_prefix("0x")
            .or_else(|| unsuffixed.strip_prefix("0X"))
            .expect("validated hexadecimal literal must have a radix prefix"),
    };
    u128::from_str_radix(digits, radix.base()).ok()
}

pub(super) fn i64_literal_through_groups(
    expression: &ResolvedExpression,
) -> Option<&ResolvedNumericLiteralExpr> {
    match expression {
        ResolvedExpression::NumericLiteral(literal)
            if matches!(literal.kind, NumericLiteralKind::I64(_)) =>
        {
            Some(literal)
        }
        ResolvedExpression::Grouped(grouped) => i64_literal_through_groups(&grouped.expression),
        _ => None,
    }
}
