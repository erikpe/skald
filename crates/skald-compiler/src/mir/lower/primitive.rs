//! Primitive type and pure-cast lowering.

use crate::{
    hir::{HirExpression, HirPrimitiveCast, HirPrimitiveCastKind, HirPrimitiveType},
    mir::{MirPrimitiveCast, MirPrimitiveCastKind, MirPrimitiveType, MirRvalueKind, ValueId},
};

use super::BodyLowerer;

impl BodyLowerer<'_> {
    pub(super) fn lower_primitive_cast(
        &mut self,
        expression: &HirExpression,
        operation: HirPrimitiveCast,
        operand: &HirExpression,
    ) -> Option<ValueId> {
        assert!(
            !operation.may_terminate(),
            "checked primitive casts require explicit MIR control flow"
        );
        let operand = self
            .lower_expression(operand)
            .expect("typed primitive-cast operand must produce a value");
        let expected_kind = lower_primitive_cast_kind(operation.kind());
        let operation = MirPrimitiveCast::new(
            lower_primitive_type(operation.source),
            lower_primitive_type(operation.target),
        );
        debug_assert_eq!(operation.kind(), expected_kind);
        Some(self.assign(
            MirRvalueKind::PrimitiveCast { operation, operand },
            operation.result_type(),
            expression.span,
        ))
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
        HirPrimitiveCastKind::CheckedF64ToInteger => MirPrimitiveCastKind::CheckedF64ToInteger,
    }
}
