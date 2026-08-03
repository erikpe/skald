//! Primitive type and pure-cast lowering.

use crate::{
    hir::{HirExpression, HirPrimitiveCast, HirPrimitiveCastKind, HirPrimitiveType},
    mir::{
        MirF64ToIntegerRange, MirInstruction, MirPlace, MirPrimitiveCast, MirPrimitiveCastKind,
        MirPrimitiveCastRangeCheck, MirPrimitiveType, MirRvalueKind, MirStore, MirTerminator,
        StorageId, ValueId,
    },
};

use super::BodyLowerer;

impl BodyLowerer<'_> {
    pub(super) fn lower_primitive_cast(
        &mut self,
        expression: &HirExpression,
        operation: HirPrimitiveCast,
        operand: &HirExpression,
    ) -> Option<ValueId> {
        if operation.may_terminate() {
            return Some(self.lower_checked_f64_to_integer(expression, operation, operand));
        }
        let operand = self
            .lower_expression(operand)
            .expect("typed primitive-cast operand must produce a value");
        let expected_kind = lower_primitive_cast_kind(operation.kind());
        let source = lower_primitive_type(operation.source);
        let target = lower_primitive_type(operation.target);
        let operation = if expected_kind == MirPrimitiveCastKind::BitReinterpretation {
            MirPrimitiveCast::bit_reinterpretation(source, target)
        } else {
            MirPrimitiveCast::new(source, target)
        };
        debug_assert_eq!(operation.kind(), expected_kind);
        Some(self.assign(
            MirRvalueKind::PrimitiveCast { operation, operand },
            operation.result_type(),
            expression.span,
        ))
    }

    fn lower_checked_f64_to_integer(
        &mut self,
        expression: &HirExpression,
        operation: HirPrimitiveCast,
        operand: &HirExpression,
    ) -> ValueId {
        debug_assert_eq!(operation.kind(), HirPrimitiveCastKind::CheckedF64ToInteger);
        let relation = MirF64ToIntegerRange {
            target: lower_primitive_type(operation.target)
                .integer_type()
                .expect("checked primitive cast target must be an integer"),
        };

        // The effectful operand completes exactly once before its scalar value
        // is secured across the range-check diamond.
        let source = self
            .lower_expression(operand)
            .expect("typed primitive-cast operand must produce a value");
        let (source, _) = self.spill_scalar(source, relation.source_type(), operand.span);
        let result = self.new_primitive_cast_result(relation.result_type(), expression.span);

        let success = self.body.allocate_block(expression.span);
        let failure = self.body.allocate_block(expression.span);
        let join = self.body.allocate_block(expression.span);
        self.terminate(MirTerminator::PrimitiveCastRangeCheck {
            check: MirPrimitiveCastRangeCheck {
                relation,
                source,
                result,
            },
            success_target: success,
            failure_target: failure,
            span: expression.span,
        });

        self.body
            .select_block(success)
            .expect("allocated primitive-cast success block must be selectable");
        let source_value = self.assign(
            MirRvalueKind::Load(MirPlace::base(source)),
            relation.source_type(),
            operand.span,
        );
        let converted = self.assign(
            MirRvalueKind::CheckedF64ToInteger {
                relation,
                operand: source_value,
            },
            relation.result_type(),
            expression.span,
        );
        self.emit(MirInstruction::Store(MirStore {
            destination: MirPlace::base(result),
            value: converted,
            span: expression.span,
        }));
        self.terminate(MirTerminator::Goto {
            target: join,
            span: expression.span,
        });

        self.body
            .select_block(failure)
            .expect("allocated primitive-cast failure block must be selectable");
        self.terminate(MirTerminator::Terminate {
            reason: relation.failure_reason(),
            span: expression.span,
        });

        self.body
            .select_block(join)
            .expect("allocated primitive-cast result join must be selectable");
        self.assign(
            MirRvalueKind::Load(MirPlace::base(result)),
            relation.result_type(),
            expression.span,
        )
    }

    fn new_primitive_cast_result(
        &mut self,
        ty: crate::mir::MirType,
        span: crate::source::Span,
    ) -> StorageId {
        self.new_scalar_spill_storage("primitive-cast-result", ty, span)
    }
}

pub(super) const fn lower_primitive_type(ty: HirPrimitiveType) -> MirPrimitiveType {
    match ty {
        HirPrimitiveType::I64 => MirPrimitiveType::I64,
        HirPrimitiveType::U64 => MirPrimitiveType::U64,
        HirPrimitiveType::U8 => MirPrimitiveType::U8,
        HirPrimitiveType::F64 => MirPrimitiveType::F64,
        HirPrimitiveType::Bool => MirPrimitiveType::Bool,
    }
}

const fn lower_primitive_cast_kind(kind: HirPrimitiveCastKind) -> MirPrimitiveCastKind {
    match kind {
        HirPrimitiveCastKind::Identity => MirPrimitiveCastKind::Identity,
        HirPrimitiveCastKind::IntegerBits => MirPrimitiveCastKind::IntegerBits,
        HirPrimitiveCastKind::ToBool => MirPrimitiveCastKind::ToBool,
        HirPrimitiveCastKind::ToF64 => MirPrimitiveCastKind::ToF64,
        HirPrimitiveCastKind::FromBool => MirPrimitiveCastKind::FromBool,
        HirPrimitiveCastKind::BitReinterpretation => MirPrimitiveCastKind::BitReinterpretation,
        HirPrimitiveCastKind::CheckedF64ToInteger => MirPrimitiveCastKind::CheckedF64ToInteger,
    }
}
