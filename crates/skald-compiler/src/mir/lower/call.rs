//! Scalar calls and source-ordered call argument lowering.

use crate::{
    hir::{
        HirAccess, HirCallArgument, HirExpression, HirInterfaceCallTarget, HirMethodCallTarget,
        HirMethodReceiver, HirObjectOrigin, HirObjectView, HirViewSource, HirViewTarget,
    },
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
        receiver: &HirMethodReceiver,
        target: HirMethodCallTarget,
        arguments: &[HirCallArgument],
    ) -> Option<ValueId> {
        // Receiver selection precedes all explicit argument effects.
        let receiver = self.lower_method_receiver(receiver);
        let arguments = self.lower_call_arguments(arguments);
        self.emit_scalar_call(
            MirCallTarget::Method(lower_method_target(target)),
            Some(receiver.into()),
            arguments,
            expression,
        )
    }

    pub(super) fn lower_interface_call(
        &mut self,
        expression: &HirExpression,
        receiver: &HirObjectView,
        target: HirInterfaceCallTarget,
        arguments: &[HirCallArgument],
    ) -> Option<ValueId> {
        // Receiver selection precedes all explicit argument effects.
        let receiver = self.lower_object_view(receiver);
        let arguments = self.lower_call_arguments(arguments);
        self.emit_scalar_call(
            MirCallTarget::Interface(MirInterfaceCallTarget {
                interface: target.interface,
                requirement: target.requirement,
            }),
            Some(receiver.into()),
            arguments,
            expression,
        )
    }

    fn emit_scalar_call(
        &mut self,
        target: MirCallTarget,
        receiver: Option<MirCallReceiver>,
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
                HirCallArgument::View(view) => MirArgument::View(self.lower_object_view(view)),
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

    pub(super) fn lower_object_view(&self, view: &HirObjectView) -> MirObjectView {
        let source = match &view.source {
            HirViewSource::Place(place) => self.lower_object_place(place),
            HirViewSource::Forwarded { binding, .. } => {
                let storage = self.storage_for_binding(*binding);
                match self.storage[storage.index()].kind {
                    MirStorageKind::AliasParameter(_) => MirPlace::alias_parameter(storage),
                    MirStorageKind::NarrowedAlias(_) => MirPlace::narrowed_alias(storage),
                    _ => unreachable!("forwarded HIR views require indirect storage"),
                }
            }
        };
        MirObjectView {
            source,
            origin: Box::new(self.lower_object_origin(&view.origin)),
            target: type_operations::lower_view_target(view.target),
            access: type_operations::lower_access(view.access),
            span: view.span,
        }
    }

    pub(super) fn lower_method_receiver(&self, receiver: &HirMethodReceiver) -> MirMethodReceiver {
        MirMethodReceiver {
            place: self.lower_object_place(&receiver.place),
            origin: Box::new(self.lower_object_origin(&receiver.origin)),
        }
    }

    fn lower_object_origin(&self, origin: &HirObjectOrigin) -> MirObjectOrigin {
        match origin {
            HirObjectOrigin::Exact {
                complete,
                dynamic_class,
            } => MirObjectOrigin::Exact {
                complete: self.lower_object_place(complete),
                dynamic_class: *dynamic_class,
            },
            HirObjectOrigin::Forwarded {
                binding,
                static_target,
                access,
                dispatch_limit,
                span,
            } => MirObjectOrigin::Forwarded {
                carrier: self.storage_for_binding(*binding),
                static_target: lower_view_target(*static_target),
                access: lower_access(*access),
                dispatch_limit: *dispatch_limit,
                span: *span,
            },
        }
    }
}

pub(super) const fn lower_method_target(target: HirMethodCallTarget) -> MirMethodCallTarget {
    match target {
        HirMethodCallTarget::Direct(method) => MirMethodCallTarget::Direct(method),
        HirMethodCallTarget::Virtual {
            family,
            slot,
            selected,
        } => MirMethodCallTarget::Virtual {
            family,
            slot,
            selected,
        },
    }
}

const fn lower_view_target(target: HirViewTarget) -> MirViewTarget {
    match target {
        HirViewTarget::Class(class) => MirViewTarget::Class(class),
        HirViewTarget::Interface(interface) => MirViewTarget::Interface(interface),
        HirViewTarget::Obj => MirViewTarget::Obj,
    }
}

const fn lower_access(access: HirAccess) -> MirAliasAccess {
    match access {
        HirAccess::ReadOnly => MirAliasAccess::ReadOnly,
        HirAccess::Mutable => MirAliasAccess::Mutable,
    }
}
