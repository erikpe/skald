//! Eligibility and typed planning for immediate primitive range iteration.

use crate::{
    hir::{
        HirBinaryOperation, HirCallArgument, HirComparisonOperand, HirComparisonPredicate,
        HirConstructionMode, HirForIn, HirIntegerType, HirIterationItemPlan, HirIterationSpans,
        HirIterationStoredValuePlan, HirIterationValueCopy, HirIterationValueDestruction,
        HirPrimitiveComparison, HirPrimitiveRangeIterationPlan, HirStatement,
    },
    resolve::{
        ResolvedConstructionOrigin, ResolvedExpression, ResolvedForIn, ResolvedPrimitiveType,
        ResolvedRangeProtocolRealization, ResolvedTypeKind,
    },
};

use super::{CallableChecker, CheckedStatement};

impl CallableChecker<'_, '_> {
    pub(super) fn is_primitive_range_iteration_candidate(&self, statement: &ResolvedForIn) -> bool {
        let in_specialized_generic_body = self.class_owner.is_some_and(|class| {
            self.program
                .generic_specializations
                .for_class(class)
                .is_some()
        });
        !in_specialized_generic_body && primitive_range_construction(&statement.iterable).is_some()
    }

    pub(super) fn check_primitive_range_for_in_statement(
        &mut self,
        statement: &ResolvedForIn,
    ) -> CheckedStatement {
        let plan = self.check_primitive_range_iteration(statement);

        let inserted = self.read_only_locals.insert(statement.binding);
        debug_assert!(
            inserted,
            "an iteration binding is active only in its own body"
        );
        let body = self.check_block(&statement.body);
        let removed = self.read_only_locals.remove(&statement.binding);
        debug_assert!(removed);

        let effects = body.effects.clone().through_loop(statement.loop_id);
        let hir = plan.map(|plan| {
            HirStatement::ForIn(Box::new(HirForIn::new_primitive_range(
                statement.loop_id,
                statement.binding,
                plan,
                body,
                HirIterationSpans {
                    for_span: statement.for_span,
                    binding_span: statement.binding_span,
                    annotation_span: statement.annotation_span,
                    in_span: statement.in_span,
                    iterable_span: statement.iterable.span(),
                    span: statement.span,
                },
            )))
        });
        CheckedStatement::with_effects(hir, effects)
    }

    pub(super) fn check_primitive_range_iteration(
        &mut self,
        statement: &ResolvedForIn,
    ) -> Option<HirPrimitiveRangeIterationPlan> {
        let construction = primitive_range_construction(&statement.iterable)?;
        let checked = self.check_construction_arguments(construction)?;
        let origin = checked.canonical_range_origin()?.clone();
        let integer = integer_type(origin.endpoint_type)?;

        if origin.iterable != statement.selection.interface
            || statement.selection.item != origin_type(&origin)
            || statement.selection.state != origin_type(&origin)
        {
            self.report_invalid_range_origin(
                origin.operator_span,
                "range loop selection does not match its canonical construction provenance",
            );
            return None;
        }

        let HirConstructionMode::Initialize { arguments, .. } = checked.mode else {
            unreachable!("canonical range syntax always uses initializer construction")
        };
        let mut arguments = arguments.into_iter();
        let lower = primitive_value_argument(arguments.next(), origin.operator_span, self)?;
        let upper = primitive_value_argument(arguments.next(), origin.operator_span, self)?;
        debug_assert!(arguments.next().is_none());

        let ty = integer.operand_type();
        let item = HirIterationItemPlan {
            binding: statement.binding,
            access: crate::hir::HirAccess::ReadOnly,
            value: HirIterationStoredValuePlan {
                ty,
                copy: Some(HirIterationValueCopy::Trivial),
                destruction: HirIterationValueDestruction::Trivial,
            },
        };
        Some(HirPrimitiveRangeIterationPlan {
            origin,
            lower,
            upper,
            integer,
            comparison: HirPrimitiveComparison {
                predicate: HirComparisonPredicate::LessThan,
                operand: HirComparisonOperand::Integer(integer),
            },
            increment: match integer {
                HirIntegerType::I64 => HirBinaryOperation::AddI64,
                HirIntegerType::U64 => HirBinaryOperation::AddU64,
                HirIntegerType::U8 => HirBinaryOperation::AddU8,
            },
            item,
        })
    }
}

fn primitive_range_construction(
    expression: &ResolvedExpression,
) -> Option<&crate::resolve::ResolvedConstructExpr> {
    match expression {
        ResolvedExpression::Grouped(grouped) => primitive_range_construction(&grouped.expression),
        ResolvedExpression::Construct(construction) => match &construction.origin {
            ResolvedConstructionOrigin::CanonicalRangeSyntax(origin)
                if primitive_origin_type(origin).is_some() =>
            {
                Some(construction)
            }
            _ => None,
        },
        _ => None,
    }
}

fn primitive_origin_type(
    origin: &crate::resolve::ResolvedCanonicalRangeOrigin,
) -> Option<ResolvedPrimitiveType> {
    let primitive = match origin.endpoint_type {
        ResolvedTypeKind::I64 => ResolvedPrimitiveType::I64,
        ResolvedTypeKind::U64 => ResolvedPrimitiveType::U64,
        ResolvedTypeKind::U8 => ResolvedPrimitiveType::U8,
        _ => return None,
    };
    let expected = ResolvedRangeProtocolRealization::PrimitiveIntrinsic(primitive);
    (origin.ordering.realization == expected && origin.successor.realization == expected)
        .then_some(primitive)
}

fn integer_type(ty: crate::hir::Type) -> Option<HirIntegerType> {
    match ty {
        crate::hir::Type::I64 => Some(HirIntegerType::I64),
        crate::hir::Type::U64 => Some(HirIntegerType::U64),
        crate::hir::Type::U8 => Some(HirIntegerType::U8),
        _ => None,
    }
}

fn origin_type(origin: &crate::hir::HirCanonicalRangeOrigin) -> ResolvedTypeKind {
    match origin.endpoint_type {
        crate::hir::Type::I64 => ResolvedTypeKind::I64,
        crate::hir::Type::U64 => ResolvedTypeKind::U64,
        crate::hir::Type::U8 => ResolvedTypeKind::U8,
        _ => unreachable!("primitive range plan requires an integer endpoint"),
    }
}

fn primitive_value_argument(
    argument: Option<HirCallArgument>,
    span: crate::source::Span,
    checker: &mut CallableChecker<'_, '_>,
) -> Option<crate::hir::HirExpression> {
    match argument {
        Some(HirCallArgument::Value(value)) => Some(value),
        _ => {
            checker.report_invalid_range_origin(
                span,
                "primitive range endpoints must lower as ordinary scalar values",
            );
            None
        }
    }
}
