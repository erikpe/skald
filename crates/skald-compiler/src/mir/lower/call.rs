//! Scalar calls and source-ordered call argument lowering.

use crate::{
    hir::{HirCallArgument, HirExpression},
    identity::FunctionId,
};

use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_direct_call(
        &mut self,
        expression: &HirExpression,
        function: FunctionId,
        arguments: &[HirCallArgument],
    ) -> Option<ValueId> {
        // Argument evaluation is fixed left-to-right.
        let arguments = self.lower_call_arguments(arguments);
        self.emit_scalar_call(MirCallTarget::Direct(function), None, arguments, expression)
    }

    pub(super) fn lower_method_call(
        &mut self,
        expression: &HirExpression,
        receiver: &crate::hir::HirObjectPlace,
        method: crate::identity::MethodId,
        arguments: &[HirCallArgument],
    ) -> Option<ValueId> {
        // Receiver selection precedes all explicit argument effects.
        let receiver = self.lower_object_place(receiver);
        let arguments = self.lower_call_arguments(arguments);
        self.emit_scalar_call(
            MirCallTarget::Method(method),
            Some(receiver),
            arguments,
            expression,
        )
    }

    fn emit_scalar_call(
        &mut self,
        target: MirCallTarget,
        receiver: Option<MirPlace>,
        arguments: Vec<MirArgument>,
        expression: &HirExpression,
    ) -> Option<ValueId> {
        let result = (expression.ty != Type::Unit)
            .then(|| self.new_value(lower_type(expression.ty), expression.span));
        self.emit(MirInstruction::Call(MirCall {
            target,
            receiver,
            arguments,
            result,
            destination: None,
            span: expression.span,
        }));
        result
    }

    pub(super) fn lower_call_arguments(
        &mut self,
        arguments: &[HirCallArgument],
    ) -> Vec<MirArgument> {
        arguments
            .iter()
            .map(|argument| match argument {
                HirCallArgument::Value(expression) => MirArgument::Value(
                    self.lower_expression(expression)
                        .expect("typed value argument must produce a scalar value"),
                ),
                HirCallArgument::Place(place) => MirArgument::Place(self.lower_object_place(place)),
                HirCallArgument::Copy(copy) => {
                    let source = self.lower_object_source(&copy.source);
                    let destination = self.new_object_storage(
                        MirStorageKind::Argument,
                        "argument",
                        copy.source.class(),
                        copy.span,
                    );
                    self.emit(MirInstruction::CopyConstruct(MirCopyConstruction {
                        destination: MirPlace::base(destination),
                        source,
                        class: copy.source.class(),
                        operation: lower_selected_copy_operation(copy.operation),
                        span: copy.span,
                    }));
                    MirArgument::OwnedPlace(MirPlace::base(destination))
                }
            })
            .collect()
    }
}
