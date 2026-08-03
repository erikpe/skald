//! Type-checked construction of exact f64/u64 bit reinterpretation.

use super::*;
use crate::{
    hir::{HirCallArgument, HirExpressionKind, HirPrimitiveCast, HirPrimitiveType},
    intrinsic::Intrinsic,
    resolve::{ResolvedDirectCallExpr, ResolvedFunctionDeclaration},
};

impl CallableChecker<'_, '_> {
    pub(super) fn check_bit_reinterpretation_intrinsic_call(
        &mut self,
        call: &ResolvedDirectCallExpr,
        target: &ResolvedFunctionDeclaration,
        intrinsic: Intrinsic,
    ) -> Option<HirExpression> {
        let arguments = self.check_arguments(
            &call.arguments,
            &target.parameters,
            call.callee_span,
            "f64 bit intrinsic",
            Some(&target.name),
            Some(target.name_span),
        )?;
        let mut arguments = arguments.into_iter();
        let operand = match arguments.next() {
            Some(HirCallArgument::Value(value)) => value,
            _ => unreachable!("validated bit intrinsic must receive one value argument"),
        };
        debug_assert!(arguments.next().is_none());

        let (source, target_type) = match intrinsic {
            Intrinsic::F64ToBits => (HirPrimitiveType::F64, HirPrimitiveType::U64),
            Intrinsic::F64FromBits => (HirPrimitiveType::U64, HirPrimitiveType::F64),
            _ => unreachable!("only f64 bit intrinsics reach bit reinterpretation checking"),
        };
        let operation = HirPrimitiveCast::bit_reinterpretation(source, target_type);
        Some(HirExpression {
            kind: HirExpressionKind::PrimitiveCast {
                operation,
                operand: Box::new(operand),
            },
            ty: operation.result_type(),
            span: call.span,
        })
    }
}
