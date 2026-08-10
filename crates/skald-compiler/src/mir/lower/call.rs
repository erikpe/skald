//! Scalar calls and source-ordered call argument lowering.

use crate::{
    hir::{
        HirAccess, HirCallArgument, HirExpression, HirInterfaceCallTarget, HirInterfaceReceiver,
        HirMethodCallTarget, HirMethodReceiver, HirObjectOrigin, HirObjectView, HirViewSource,
        HirViewTarget,
    },
    identity::{FunctionId, MethodId},
};

use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_direct_call(
        &mut self,
        expression: &HirExpression,
        function: FunctionId,
        arguments: &[HirCallArgument],
    ) -> Option<ValueId> {
        let optional_mark = self.optional_view_mark();
        // Argument evaluation is fixed left-to-right.
        let arguments = self.lower_call_arguments(arguments);
        let result =
            self.emit_scalar_call(MirCallTarget::Direct(function), None, arguments, expression);
        self.end_optional_views_from(optional_mark, expression.span);
        result
    }

    pub(super) fn lower_static_call(
        &mut self,
        expression: &HirExpression,
        method: MethodId,
        arguments: &[HirCallArgument],
    ) -> Option<ValueId> {
        let optional_mark = self.optional_view_mark();
        // Static calls evaluate only their explicit arguments, left to right.
        let arguments = self.lower_call_arguments(arguments);
        let result =
            self.emit_scalar_call(MirCallTarget::Static(method), None, arguments, expression);
        self.end_optional_views_from(optional_mark, expression.span);
        result
    }

    pub(super) fn lower_method_call(
        &mut self,
        expression: &HirExpression,
        receiver: &HirMethodReceiver,
        target: HirMethodCallTarget,
        arguments: &[HirCallArgument],
    ) -> Option<ValueId> {
        let optional_mark = self.optional_view_mark();
        // Receiver selection precedes all explicit argument effects.
        let receiver = self.lower_method_receiver(receiver);
        let arguments = self.lower_call_arguments(arguments);
        let result = self.emit_scalar_call(
            MirCallTarget::Method(lower_method_target(target)),
            Some(receiver.into()),
            arguments,
            expression,
        );
        self.end_optional_views_from(optional_mark, expression.span);
        result
    }

    pub(super) fn lower_interface_call(
        &mut self,
        expression: &HirExpression,
        receiver: &HirInterfaceReceiver,
        target: HirInterfaceCallTarget,
        arguments: &[HirCallArgument],
    ) -> Option<ValueId> {
        let optional_mark = self.optional_view_mark();
        // Receiver selection precedes all explicit argument effects.
        let receiver = match receiver {
            HirInterfaceReceiver::View(view) => self.lower_object_view(view),
            HirInterfaceReceiver::Checked(view) => self.lower_checked_object_view(view),
        };
        let arguments = self.lower_call_arguments(arguments);
        let result = self.emit_scalar_call(
            MirCallTarget::Interface(MirInterfaceCallTarget {
                interface: target.interface,
                requirement: target.requirement,
            }),
            Some(receiver.into()),
            arguments,
            expression,
        );
        self.end_optional_views_from(optional_mark, expression.span);
        result
    }

    fn emit_scalar_call(
        &mut self,
        target: MirCallTarget,
        receiver: Option<MirCallReceiver>,
        arguments: Vec<MirArgument>,
        expression: &HirExpression,
    ) -> Option<ValueId> {
        let result = (expression.ty != Type::Unit)
            .then(|| self.new_value(self.lower_type(expression.ty), expression.span));
        self.emit(MirInstruction::Call(MirCall {
            target,
            receiver,
            arguments,
            result,
            shared_result: None,
            destination: None,
            span: expression.span,
        }));
        result
    }

    pub(super) fn lower_shared_call(&mut self, expression: &HirExpression, destination: StorageId) {
        let optional_mark = self.optional_view_mark();
        let (target, receiver, arguments) = match &expression.kind {
            crate::hir::HirExpressionKind::DirectCall {
                function,
                arguments,
            } => (
                MirCallTarget::Direct(*function),
                None,
                self.lower_call_arguments(arguments),
            ),
            crate::hir::HirExpressionKind::StaticCall { method, arguments } => (
                MirCallTarget::Static(*method),
                None,
                self.lower_call_arguments(arguments),
            ),
            crate::hir::HirExpressionKind::MethodCall {
                receiver,
                target,
                arguments,
            } => {
                let receiver = self.lower_method_receiver(receiver);
                (
                    MirCallTarget::Method(lower_method_target(*target)),
                    Some(receiver.into()),
                    self.lower_call_arguments(arguments),
                )
            }
            crate::hir::HirExpressionKind::InterfaceCall {
                receiver,
                target,
                arguments,
            } => {
                let receiver = match receiver {
                    HirInterfaceReceiver::View(view) => self.lower_object_view(view),
                    HirInterfaceReceiver::Checked(view) => self.lower_checked_object_view(view),
                };
                (
                    MirCallTarget::Interface(MirInterfaceCallTarget {
                        interface: target.interface,
                        requirement: target.requirement,
                    }),
                    Some(receiver.into()),
                    self.lower_call_arguments(arguments),
                )
            }
            _ => unreachable!("shared call producer must contain a call expression"),
        };
        self.emit(MirInstruction::Call(MirCall {
            target,
            receiver,
            arguments,
            result: None,
            shared_result: Some(destination),
            destination: None,
            span: expression.span,
        }));
        self.end_optional_views_from(optional_mark, expression.span);
        self.full_expression.mark_shared_effect();
    }

    pub(super) fn lower_array_call(&mut self, expression: &HirExpression, destination: StorageId) {
        let optional_mark = self.optional_view_mark();
        let (target, receiver, arguments) = match &expression.kind {
            crate::hir::HirExpressionKind::DirectCall {
                function,
                arguments,
            } => (
                MirCallTarget::Direct(*function),
                None,
                self.lower_call_arguments(arguments),
            ),
            crate::hir::HirExpressionKind::StaticCall { method, arguments } => (
                MirCallTarget::Static(*method),
                None,
                self.lower_call_arguments(arguments),
            ),
            crate::hir::HirExpressionKind::MethodCall {
                receiver,
                target,
                arguments,
            } => (
                MirCallTarget::Method(lower_method_target(*target)),
                Some(self.lower_method_receiver(receiver).into()),
                self.lower_call_arguments(arguments),
            ),
            crate::hir::HirExpressionKind::InterfaceCall {
                receiver,
                target,
                arguments,
            } => {
                let receiver = match receiver {
                    HirInterfaceReceiver::View(view) => self.lower_object_view(view),
                    HirInterfaceReceiver::Checked(view) => self.lower_checked_object_view(view),
                };
                (
                    MirCallTarget::Interface(MirInterfaceCallTarget {
                        interface: target.interface,
                        requirement: target.requirement,
                    }),
                    Some(receiver.into()),
                    self.lower_call_arguments(arguments),
                )
            }
            _ => unreachable!("array call producer must contain a call expression"),
        };
        self.emit(MirInstruction::Call(MirCall {
            target,
            receiver,
            arguments,
            result: None,
            shared_result: None,
            destination: Some(MirPlace::base(destination)),
            span: expression.span,
        }));
        self.end_optional_views_from(optional_mark, expression.span);
    }

    pub(super) fn lower_optional_call(
        &mut self,
        expression: &HirExpression,
        destination: MirPlace,
    ) {
        let optional_mark = self.optional_view_mark();
        let (target, receiver, arguments) = match &expression.kind {
            crate::hir::HirExpressionKind::DirectCall {
                function,
                arguments,
            } => (
                MirCallTarget::Direct(*function),
                None,
                self.lower_call_arguments(arguments),
            ),
            crate::hir::HirExpressionKind::StaticCall { method, arguments } => (
                MirCallTarget::Static(*method),
                None,
                self.lower_call_arguments(arguments),
            ),
            crate::hir::HirExpressionKind::MethodCall {
                receiver,
                target,
                arguments,
            } => (
                MirCallTarget::Method(lower_method_target(*target)),
                Some(self.lower_method_receiver(receiver).into()),
                self.lower_call_arguments(arguments),
            ),
            crate::hir::HirExpressionKind::InterfaceCall {
                receiver,
                target,
                arguments,
            } => {
                let receiver = match receiver {
                    HirInterfaceReceiver::View(view) => self.lower_object_view(view),
                    HirInterfaceReceiver::Checked(view) => self.lower_checked_object_view(view),
                };
                (
                    MirCallTarget::Interface(MirInterfaceCallTarget {
                        interface: target.interface,
                        requirement: target.requirement,
                    }),
                    Some(receiver.into()),
                    self.lower_call_arguments(arguments),
                )
            }
            crate::hir::HirExpressionKind::Grouped(inner) => {
                self.lower_optional_call(inner, destination);
                return;
            }
            _ => unreachable!("optional producer must contain a call expression"),
        };
        self.emit(MirInstruction::Call(MirCall {
            target,
            receiver,
            arguments,
            result: None,
            shared_result: None,
            destination: Some(destination),
            span: expression.span,
        }));
        self.end_optional_views_from(optional_mark, expression.span);
    }

    pub(super) fn lower_optional_shared_call(
        &mut self,
        expression: &HirExpression,
        destination: StorageId,
    ) {
        let optional_mark = self.optional_view_mark();
        let (target, receiver, arguments) = match &expression.kind {
            crate::hir::HirExpressionKind::DirectCall {
                function,
                arguments,
            } => (
                MirCallTarget::Direct(*function),
                None,
                self.lower_call_arguments(arguments),
            ),
            crate::hir::HirExpressionKind::StaticCall { method, arguments } => (
                MirCallTarget::Static(*method),
                None,
                self.lower_call_arguments(arguments),
            ),
            crate::hir::HirExpressionKind::MethodCall {
                receiver,
                target,
                arguments,
            } => (
                MirCallTarget::Method(lower_method_target(*target)),
                Some(self.lower_method_receiver(receiver).into()),
                self.lower_call_arguments(arguments),
            ),
            crate::hir::HirExpressionKind::InterfaceCall {
                receiver,
                target,
                arguments,
            } => {
                let receiver = match receiver {
                    HirInterfaceReceiver::View(view) => self.lower_object_view(view),
                    HirInterfaceReceiver::Checked(view) => self.lower_checked_object_view(view),
                };
                (
                    MirCallTarget::Interface(MirInterfaceCallTarget {
                        interface: target.interface,
                        requirement: target.requirement,
                    }),
                    Some(receiver.into()),
                    self.lower_call_arguments(arguments),
                )
            }
            crate::hir::HirExpressionKind::Grouped(inner) => {
                self.lower_optional_shared_call(inner, destination);
                return;
            }
            _ => unreachable!("optional shared producer must contain a call expression"),
        };
        self.emit(MirInstruction::Call(MirCall {
            target,
            receiver,
            arguments,
            result: None,
            shared_result: Some(destination),
            destination: None,
            span: expression.span,
        }));
        self.end_optional_views_from(optional_mark, expression.span);
        self.full_expression.mark_shared_effect();
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
                .any(super::control_effect::call_argument_contains_control_effect);
            let argument = match argument {
                HirCallArgument::Value(expression) => {
                    let value = self
                        .lower_expression(expression)
                        .expect("typed value argument must produce a scalar value");
                    if later_branch {
                        let (storage, ty) = self.spill_scalar(
                            value,
                            self.lower_type(expression.ty),
                            expression.span,
                        );
                        LoweredArgument::Spilled {
                            storage,
                            ty,
                            span: expression.span,
                        }
                    } else {
                        LoweredArgument::Ready(MirArgument::Value(value))
                    }
                }
                HirCallArgument::Optional { source, payload } => {
                    let ty = MirType::OptionalPrimitive(super::primitive::lower_primitive_type(
                        *payload,
                    ));
                    let storage = self.new_optional_storage(
                        MirStorageKind::Argument,
                        "optional-argument",
                        ty,
                        source.span(),
                    );
                    let source = self.lower_optional_source(source);
                    self.emit(MirInstruction::OptionalInitialize(MirOptionalInitialize {
                        destination: MirPlace::base(storage),
                        source,
                        span: argument.span(),
                    }));
                    LoweredArgument::Ready(MirArgument::OwnedPlace(MirPlace::base(storage)))
                }
                HirCallArgument::ClassOptional(value) => {
                    let storage = self.new_optional_storage(
                        MirStorageKind::Argument,
                        "class-optional-argument",
                        MirType::OptionalClass(value.class),
                        value.span,
                    );
                    self.lower_class_optional_initialize(storage, value);
                    LoweredArgument::Ready(MirArgument::OwnedPlace(MirPlace::base(storage)))
                }
                HirCallArgument::OptionalShared(value) => {
                    let storage = self.new_optional_storage(
                        MirStorageKind::Argument,
                        "optional-shared-argument",
                        MirType::OptionalShared(super::lower_shared_target(value.target)),
                        value.span,
                    );
                    self.lower_optional_shared_initialize(storage, value);
                    LoweredArgument::Ready(MirArgument::SharedOwner(storage))
                }
                HirCallArgument::OptionalPlace(place) => {
                    let place = match place {
                        crate::hir::HirOptionalAliasPlace::Primitive(place) => {
                            self.lower_optional_place(place)
                        }
                        crate::hir::HirOptionalAliasPlace::Class(place) => {
                            self.lower_class_optional_place(place)
                        }
                    };
                    LoweredArgument::Ready(MirArgument::Place(place))
                }
                HirCallArgument::Place(place) => {
                    LoweredArgument::Ready(MirArgument::Place(self.lower_object_place(place)))
                }
                HirCallArgument::PrimitivePlace(place) => {
                    let place = match place.storage {
                        crate::hir::HirPrimitiveStorage::Binding(binding) => {
                            self.lower_binding_place(binding)
                        }
                        crate::hir::HirPrimitiveStorage::Static(place) => {
                            MirPlace::static_field(place.field)
                        }
                    };
                    LoweredArgument::Ready(MirArgument::Place(place))
                }
                HirCallArgument::View(view) => {
                    LoweredArgument::Ready(MirArgument::View(self.lower_object_view(view)))
                }
                HirCallArgument::CheckedView(view) => {
                    LoweredArgument::Ready(MirArgument::View(self.lower_checked_object_view(view)))
                }
                HirCallArgument::Copy(copy) => {
                    let optional_mark = self.optional_view_mark();
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
                    self.end_optional_views_from(optional_mark, copy.span);
                    LoweredArgument::Ready(MirArgument::OwnedPlace(MirPlace::base(destination)))
                }
                HirCallArgument::Shared(transfer) => {
                    let storage = StorageId::new(self.input.callable, self.storage.len());
                    self.storage.push(MirStorage {
                        id: storage,
                        source: None,
                        name: format!("shared-argument-{}", storage.index()),
                        kind: MirStorageKind::Argument,
                        ty: self.lower_type(Type::Shared(transfer.target)),
                        span: transfer.span,
                    });
                    self.track_full_expression_storage(storage, transfer.span);
                    self.lower_shared_transfer(storage, transfer);
                    LoweredArgument::Ready(MirArgument::SharedOwner(storage))
                }
                HirCallArgument::Array(initialization) => {
                    let storage = self.new_array_storage(
                        initialization.source.array,
                        MirStorageKind::Argument,
                        "argument",
                        initialization.span,
                    );
                    self.lower_array_initialize(MirPlace::base(storage), initialization, false);
                    LoweredArgument::Ready(MirArgument::OwnedPlace(MirPlace::base(storage)))
                }
                HirCallArgument::ArrayAlias(alias) => {
                    let place = match &alias.source {
                        crate::hir::HirArrayAliasSource::Whole(receiver) => {
                            self.lower_array_alias_receiver_place(receiver)
                        }
                        crate::hir::HirArrayAliasSource::Element(element) => {
                            self.lower_array_alias_element_place(element, alias.access)
                        }
                    };
                    LoweredArgument::Ready(MirArgument::Place(place))
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
            HirViewSource::Produced { producer, .. } => Some(producer.class()),
            HirViewSource::OptionalPayload { view, .. } => {
                Some(self.input.optional_adapter.operand_class(&view.source))
            }
            _ => None,
        };
        let source = match &view.source {
            HirViewSource::Place(place) => self.lower_object_place(place),
            HirViewSource::Produced {
                producer,
                projections,
            } => projections.iter().fold(
                self.lower_object_producer_temporary(producer),
                lower_projection,
            ),
            HirViewSource::Forwarded { binding, .. } => {
                let storage = self.storage_for_binding(*binding);
                match self.storage[storage.index()].kind {
                    MirStorageKind::AliasParameter(_) => MirPlace::alias_parameter(storage),
                    MirStorageKind::CheckedView(_) => MirPlace::checked_view(storage),
                    _ => unreachable!("forwarded HIR views require indirect storage"),
                }
            }
            HirViewSource::Shared {
                binding,
                projections,
                ..
            } => projections.iter().fold(
                MirPlace::shared_pointee(self.storage_for_binding(*binding)),
                lower_projection,
            ),
            HirViewSource::AnchoredShared {
                source,
                projections,
                ..
            } => projections.iter().fold(
                MirPlace::shared_pointee(self.new_shared_anchor(source, view.span)),
                lower_projection,
            ),
            HirViewSource::OptionalPayload {
                view: optional,
                projections,
            } => projections
                .iter()
                .fold(self.begin_optional_view(optional), lower_projection),
        };
        let produced_complete = matches!(view.source, HirViewSource::Produced { .. })
            .then(|| MirPlace::base(source.base.expect_local_storage()));
        let origin = produced_class.map_or_else(
            || match &view.source {
                HirViewSource::AnchoredShared {
                    source: shared_source,
                    target,
                    ..
                } => MirObjectOrigin::Shared {
                    owner: source.base.expect_local_storage(),
                    static_target: type_operations::lower_view_target(*target),
                    access: MirAliasAccess::Mutable,
                    exact_dynamic_class: shared_source.exact_dynamic_class(),
                    span: view.span,
                },
                _ => self.lower_object_origin(&view.origin),
            },
            |dynamic_class| MirObjectOrigin::Exact {
                complete: produced_complete.unwrap_or_else(|| source.clone()),
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
        if let Some(element) = &receiver.array_element {
            let place = self.lower_array_alias_element_place(element, element.receiver.access);
            let Type::Class(dynamic_class) = element.element else {
                unreachable!("object method array receiver must have exact class type")
            };
            return MirMethodReceiver::exact(place, dynamic_class);
        }
        if let Some(cast) = &receiver.checked_cast {
            let view = self.lower_checked_object_view(cast);
            return MirMethodReceiver {
                place: view.source,
                origin: view.origin,
            };
        }
        if let Some(view) = &receiver.shared_view {
            let view = self.lower_object_view(view);
            return MirMethodReceiver {
                place: view.source,
                origin: view.origin,
            };
        }
        if let Some(view) = &receiver.optional_view {
            let view = self.lower_object_view(view);
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
            HirViewSource::Place(_) | HirViewSource::Produced { .. }
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
        self.track_full_expression_storage(destination, checked.span);
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
        self.full_expression.register_checked_view(destination);
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
            HirObjectOrigin::Shared {
                binding,
                static_target,
                access,
                span,
            } => MirObjectOrigin::Shared {
                owner: self.storage_for_binding(*binding),
                static_target: lower_view_target(*static_target),
                access: lower_access(*access),
                exact_dynamic_class: None,
                span: *span,
            },
            HirObjectOrigin::AnchoredShared { .. } => {
                unreachable!("anchored origins are bound while lowering their view source")
            }
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

fn lower_projection(
    place: MirPlace,
    projection: &crate::object_path::ObjectProjection,
) -> MirPlace {
    match *projection {
        crate::object_path::ObjectProjection::Base(base) => place.project_base(base),
        crate::object_path::ObjectProjection::Field(field) => place.project_field(field),
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
