//! Eligibility and typed planning for direct primitive range iteration.

use crate::{
    hir::{
        HirBinaryOperation, HirCallArgument, HirComparisonOperand, HirComparisonPredicate,
        HirConstructionMode, HirForIn, HirIntegerType, HirIterationItemPlan, HirIterationSpans,
        HirIterationStoredValuePlan, HirIterationValueCopy, HirIterationValueDestruction,
        HirPrimitiveComparison, HirPrimitiveRangeIterationPlan, HirStatement,
    },
    resolve::{
        ResolvedForIn, ResolvedForInSource, ResolvedPrimitiveType, ResolvedRangeForSource,
        ResolvedRangeProtocolRealization, ResolvedTypeKind,
    },
};

use super::{range_source::resolved_range_construction, CallableChecker, CheckedStatement};

impl CallableChecker<'_, '_> {
    pub(super) fn is_primitive_range_iteration_candidate(&self, statement: &ResolvedForIn) -> bool {
        primitive_range_source(&statement.source).is_some_and(|source| {
            source.endpoint_provenance.iter().all(|provenance| {
                matches!(
                    provenance,
                    crate::resolve::ResolvedRangeEndpointProvenance::SpecializationIndependent
                )
            })
        })
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
                    iterable_span: statement.source.span(),
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
        let source = primitive_range_source(&statement.source)?;
        let evidence = self.validate_primitive_range_source(source)?;
        let integer = integer_type(source.endpoint_type)?;
        self.validate_range_selection(source, &statement.selection)?;

        let construction = resolved_range_construction(source);
        let checked = self.check_construction_arguments(&construction)?;
        if checked.initializer() != Some(source.initializer) {
            return None;
        }
        let HirConstructionMode::Initialize { arguments, .. } = checked.mode else {
            unreachable!("direct range sources always use initializer construction")
        };
        let mut arguments = arguments.into_iter();
        let lower = primitive_value_argument(arguments.next())?;
        let upper = primitive_value_argument(arguments.next())?;
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
            evidence,
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

fn primitive_range_source(source: &ResolvedForInSource) -> Option<&ResolvedRangeForSource> {
    let ResolvedForInSource::Range(source) = source else {
        return None;
    };
    primitive_source_type(source).is_some().then_some(source)
}

fn primitive_source_type(source: &ResolvedRangeForSource) -> Option<ResolvedPrimitiveType> {
    let primitive = match source.endpoint_type {
        ResolvedTypeKind::I64 => ResolvedPrimitiveType::I64,
        ResolvedTypeKind::U64 => ResolvedPrimitiveType::U64,
        ResolvedTypeKind::U8 => ResolvedPrimitiveType::U8,
        _ => return None,
    };
    let expected = ResolvedRangeProtocolRealization::PrimitiveIntrinsic(primitive);
    (source.ordering.realization == expected && source.successor.realization == expected)
        .then_some(primitive)
}

fn integer_type(ty: ResolvedTypeKind) -> Option<HirIntegerType> {
    match ty {
        ResolvedTypeKind::I64 => Some(HirIntegerType::I64),
        ResolvedTypeKind::U64 => Some(HirIntegerType::U64),
        ResolvedTypeKind::U8 => Some(HirIntegerType::U8),
        _ => None,
    }
}

fn primitive_value_argument(
    argument: Option<HirCallArgument>,
) -> Option<crate::hir::HirExpression> {
    match argument {
        Some(HirCallArgument::Value(value)) => Some(value),
        _ => None,
    }
}
