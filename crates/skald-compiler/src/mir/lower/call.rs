//! Scalar calls and source-ordered call argument lowering.

use crate::{
    hir::{
        HirAccess, HirCallArgument, HirExpression, HirInterfaceCallTarget, HirInterfaceReceiver,
        HirMethodCallTarget, HirMethodReceiver, HirObjectOrigin, HirObjectView, HirViewSource,
        HirViewTarget,
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
        receiver: &HirInterfaceReceiver,
        target: HirInterfaceCallTarget,
        arguments: &[HirCallArgument],
    ) -> Option<ValueId> {
        // Receiver selection precedes all explicit argument effects.
        let receiver = match receiver {
            HirInterfaceReceiver::View(view) => self.lower_object_view(view),
            HirInterfaceReceiver::Checked(view) => self.lower_checked_object_view(view),
        };
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
        enum LoweredArgument {
            Ready(MirArgument),
            Spilled {
                storage: StorageId,
                ty: MirType,
                span: crate::source::Span,
            },
        }

        let mut lowered = Vec::with_capacity(arguments.len());
        for (index, argument) in arguments.iter().enumerate() {
            let later_branch = arguments[index + 1..]
                .iter()
                .any(super::control_effect::call_argument_contains_runtime_cast);
            let argument = match argument {
                HirCallArgument::Value(expression) => {
                    let value = self
                        .lower_expression(expression)
                        .expect("typed value argument must produce a scalar value");
                    if later_branch {
                        let (storage, ty) =
                            self.spill_scalar(value, lower_type(expression.ty), expression.span);
                        LoweredArgument::Spilled {
                            storage,
                            ty,
                            span: expression.span,
                        }
                    } else {
                        LoweredArgument::Ready(MirArgument::Value(value))
                    }
                }
                HirCallArgument::Place(place) => {
                    LoweredArgument::Ready(MirArgument::Place(self.lower_object_place(place)))
                }
                HirCallArgument::View(view) => {
                    LoweredArgument::Ready(MirArgument::View(self.lower_object_view(view)))
                }
                HirCallArgument::CheckedView(view) => {
                    LoweredArgument::Ready(MirArgument::View(self.lower_checked_object_view(view)))
                }
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
                    LoweredArgument::Ready(MirArgument::OwnedPlace(MirPlace::base(destination)))
                }
            };
            lowered.push(argument);
        }
        lowered
            .into_iter()
            .map(|argument| match argument {
                LoweredArgument::Ready(argument) => argument,
                LoweredArgument::Spilled { storage, ty, span } => {
                    MirArgument::Value(self.assign(MirRvalueKind::Load(storage.into()), ty, span))
                }
            })
            .collect()
    }

    pub(super) fn lower_object_view(&mut self, view: &HirObjectView) -> MirObjectView {
        let produced_class = match &view.source {
            HirViewSource::Produced(producer) => Some(producer.class()),
            _ => None,
        };
        let source = match &view.source {
            HirViewSource::Place(place) => self.lower_object_place(place),
            HirViewSource::Produced(producer) => self.lower_object_source(
                &crate::hir::HirObjectSource::Produced(producer.as_ref().clone()),
            ),
            HirViewSource::Forwarded { binding, .. } => {
                let storage = self.storage_for_binding(*binding);
                match self.storage[storage.index()].kind {
                    MirStorageKind::AliasParameter(_) => MirPlace::alias_parameter(storage),
                    MirStorageKind::NarrowedAlias(_) => MirPlace::narrowed_alias(storage),
                    MirStorageKind::CheckedView(_) => MirPlace::checked_view(storage),
                    _ => unreachable!("forwarded HIR views require indirect storage"),
                }
            }
        };
        let origin = produced_class.map_or_else(
            || self.lower_object_origin(&view.origin),
            |dynamic_class| MirObjectOrigin::Exact {
                complete: source.clone(),
                dynamic_class,
            },
        );
        MirObjectView {
            source,
            origin: Box::new(origin),
            target: type_operations::lower_view_target(view.target),
            access: type_operations::lower_access(view.access),
            span: view.span,
        }
    }

    pub(super) fn lower_method_receiver(
        &mut self,
        receiver: &HirMethodReceiver,
    ) -> MirMethodReceiver {
        if let Some(cast) = &receiver.checked_cast {
            let view = self.lower_checked_object_view(cast);
            return MirMethodReceiver {
                place: view.source,
                origin: view.origin,
            };
        }
        MirMethodReceiver {
            place: self.lower_object_place(&receiver.place),
            origin: Box::new(self.lower_object_origin(&receiver.origin)),
        }
    }

    pub(super) fn lower_checked_object_view(
        &mut self,
        checked: &crate::hir::HirCheckedObjectView,
    ) -> MirObjectView {
        let source = self.lower_object_view(&checked.view);
        let direct_static_source = matches!(
            checked.view.source,
            HirViewSource::Place(_) | HirViewSource::Produced(_)
        );
        if checked.kind == crate::hir::HirCheckedObjectViewKind::Static && direct_static_source {
            let projected = checked
                .projections
                .iter()
                .fold(source.source, |place, projection| match projection {
                    crate::object_path::ObjectProjection::Base(base) => place.project_base(*base),
                    crate::object_path::ObjectProjection::Field(field) => {
                        place.project_field(*field)
                    }
                });
            return MirObjectView {
                source: projected,
                origin: source.origin,
                target: type_operations::lower_view_target(checked.consumer_target),
                access: type_operations::lower_access(checked.consumer_access),
                span: checked.span,
            };
        }
        let destination = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id: destination,
            source: None,
            name: format!("cast#{}", destination.index()),
            kind: MirStorageKind::CheckedView(type_operations::lower_access(checked.view.access)),
            ty: type_operations::lower_view_target(checked.view.target).ty(),
            span: checked.span,
        });
        let binding = MirCheckedViewBinding {
            destination,
            view: source.clone(),
            span: checked.span,
        };
        match checked.kind {
            crate::hir::HirCheckedObjectViewKind::Static => {
                self.emit(MirInstruction::BindCheckedView(binding));
            }
            crate::hir::HirCheckedObjectViewKind::RuntimeTerminate => {
                let success = self.body.allocate_block(checked.span);
                let failure = self.body.allocate_block(checked.span);
                self.terminate(MirTerminator::CheckedCast {
                    binding,
                    success_target: success,
                    failure_target: failure,
                    span: checked.span,
                });
                self.body
                    .select_block(failure)
                    .expect("allocated cast failure block must be selectable");
                self.terminate(MirTerminator::Terminate {
                    reason: MirTerminationReason::ObjectCastFailure,
                    span: checked.span,
                });
                self.body
                    .select_block(success)
                    .expect("allocated cast success block must be selectable");
            }
        }
        self.full_expression_checked_views.push(destination);
        let source = checked.projections.iter().fold(
            MirPlace::checked_view(destination),
            |place, projection| match projection {
                crate::object_path::ObjectProjection::Base(base) => place.project_base(*base),
                crate::object_path::ObjectProjection::Field(field) => place.project_field(*field),
            },
        );
        let origin = Box::new(MirObjectOrigin::Forwarded {
            carrier: destination,
            static_target: type_operations::lower_view_target(checked.view.target),
            access: type_operations::lower_access(checked.view.access),
            dispatch_limit: None,
            span: checked.span,
        });
        MirObjectView {
            source,
            origin,
            target: type_operations::lower_view_target(checked.consumer_target),
            access: type_operations::lower_access(checked.consumer_access),
            span: checked.span,
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
            HirObjectOrigin::Produced { .. } => {
                unreachable!("produced origins are replaced while lowering their source")
            }
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
