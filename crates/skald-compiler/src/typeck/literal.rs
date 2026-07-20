//! Numeric literal conversion and range diagnostics.

use crate::{
    diagnostics::Diagnostic,
    hir::{HirExpression, HirExpressionKind, Type},
    literal::NumericLiteralKind,
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
            NumericLiteralKind::I64 => self.check_positive_i64_literal(literal),
            NumericLiteralKind::U64 => self.check_u64_literal(literal),
            NumericLiteralKind::U8 => self.check_u8_literal(literal),
            NumericLiteralKind::F64 => self.check_f64_literal(literal),
        }
    }

    fn check_positive_i64_literal(
        &mut self,
        literal: &ResolvedNumericLiteralExpr,
    ) -> Option<HirExpression> {
        match literal.spelling.parse::<i64>() {
            Ok(value) => Some(HirExpression {
                kind: HirExpressionKind::I64(value),
                ty: Type::I64,
                span: literal.span,
            }),
            Err(_) => {
                self.report_integer_out_of_range(literal.span, literal.spelling.clone());
                None
            }
        }
    }

    fn check_u64_literal(&mut self, literal: &ResolvedNumericLiteralExpr) -> Option<HirExpression> {
        let digits = literal
            .spelling
            .strip_suffix('u')
            .expect("validated u64 literal must have a `u` suffix");
        match digits.parse::<u64>() {
            Ok(value) => Some(HirExpression {
                kind: HirExpressionKind::U64(value),
                ty: Type::U64,
                span: literal.span,
            }),
            Err(_) => {
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
        let digits = literal
            .spelling
            .strip_suffix("u8")
            .expect("validated u8 literal must have a `u8` suffix");
        match digits.parse::<u8>() {
            Ok(value) => Some(HirExpression {
                kind: HirExpressionKind::U8(value),
                ty: Type::U8,
                span: literal.span,
            }),
            Err(_) => {
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

pub(super) fn classify_i64_magnitude(spelling: &str) -> Magnitude {
    let Ok(magnitude) = spelling.parse::<u128>() else {
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

pub(super) fn i64_literal_through_groups(
    expression: &ResolvedExpression,
) -> Option<&ResolvedNumericLiteralExpr> {
    match expression {
        ResolvedExpression::NumericLiteral(literal) if literal.kind == NumericLiteralKind::I64 => {
            Some(literal)
        }
        ResolvedExpression::Grouped(grouped) => i64_literal_through_groups(&grouped.expression),
        _ => None,
    }
}
