//! Receiverless calls through canonical capture-free function signatures.

use super::*;
use crate::{hir::HirIndirectCall, typeck::program::lower_type};

impl CallableChecker<'_, '_> {
    pub(super) fn check_indirect_call(
        &mut self,
        call: &crate::resolve::ResolvedIndirectCallExpr,
    ) -> Option<HirExpression> {
        // Checking the callee first makes the runtime evaluation contract
        // structural before MIR introduces its stabilizing temporary.
        let callee = self.check_expression(&call.callee)?;
        if !require_type(
            callee.ty,
            Type::Function(call.function_type),
            callee.span,
            "indirect call target",
            self.diagnostics,
        ) {
            return None;
        }

        let signature = self
            .program
            .function_types
            .get(call.function_type)
            .expect("resolved indirect call must reference a canonical function type");
        let arguments = self.check_arguments(
            &call.arguments,
            &signature.parameters,
            call.callee.span(),
            "function value",
            None,
            Some(signature.span),
        )?;
        let result = lower_type(self.program, &signature.result);
        Some(
            HirIndirectCall::new(callee, call.function_type, arguments, result, call.span)
                .into_expression(),
        )
    }
}
