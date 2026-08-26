//! Scalar calls and source-ordered call argument lowering.

use crate::{
    hir::{
        HirAccess, HirCallArgument, HirExpression, HirIndirectCall, HirInterfaceCallTarget,
        HirInterfaceReceiver, HirMethodCallTarget, HirMethodReceiver, HirObjectOrigin,
        HirObjectReceiver, HirObjectView, HirSharedPlace, HirSharedSource, HirViewSource,
        HirViewTarget,
    },
    identity::{FunctionId, MethodId},
};

use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_indirect_call(
        &mut self,
        expression: &HirExpression,
        call: &HirIndirectCall,
    ) -> Option<ValueId> {
        let optional_mark = self.optional_view_mark();
        let (target, arguments) = self.lower_indirect_target_and_arguments(call);
        let result = self.emit_scalar_call(target, None, arguments, expression);
        self.end_optional_views_from(optional_mark, expression.span);
        result
    }

    /// Selects an indirect callee exactly once before any explicit argument.
    ///
    /// MIR values are block-local. When argument lowering may split control
    /// flow, the selected address is secured in ordinary scalar storage and
    /// reloaded in the call block without reevaluating the HIR callee.
    pub(super) fn lower_indirect_target_and_arguments(
        &mut self,
        call: &HirIndirectCall,
    ) -> (MirCallTarget, Vec<MirArgument>) {
        let callee = self
            .lower_expression(&call.callee)
            .expect("typed indirect callee must produce a scalar value");
        let ty = MirType::Function(call.function_type);
        let secured = call
            .arguments
            .iter()
            .any(super::control_effect::call_argument_contains_control_effect)
            .then(|| self.spill_scalar(callee, ty, call.callee.span));
        let arguments = self.lower_call_arguments(&call.arguments);
        let callee = secured
            .map(|(storage, ty)| {
                self.assign(MirRvalueKind::Load(storage.into()), ty, call.callee.span)
            })
            .unwrap_or(callee);
        (
            MirCallTarget::Indirect(MirIndirectCallTarget {
                callee,
                function_type: call.function_type,
            }),
            arguments,
        )
    }

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

    /// Lowers the common target/receiver/argument prefix shared by every
    /// ordinary call result carrier. Result ownership remains with the caller
    /// so scalar, aggregate, optional, and shared destinations keep their
    /// established specialized completion paths.
    fn lower_call_parts(
        &mut self,
        expression: &HirExpression,
    ) -> (MirCallTarget, Option<MirCallReceiver>, Vec<MirArgument>) {
        match &expression.kind {
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
            crate::hir::HirExpressionKind::IndirectCall(call) => {
                let (target, arguments) = self.lower_indirect_target_and_arguments(call);
                (target, None, arguments)
            }
            _ => unreachable!("call producer must contain a call expression"),
        }
    }

    pub(super) fn lower_shared_call(&mut self, expression: &HirExpression, destination: StorageId) {
        let optional_mark = self.optional_view_mark();
        let (target, receiver, arguments) = self.lower_call_parts(expression);
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
        let (target, receiver, arguments) = self.lower_call_parts(expression);
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
        if let crate::hir::HirExpressionKind::NestedOptionalUnwrap(unwrap) = &expression.kind {
            self.lower_nested_optional_unwrap_at(destination, unwrap);
            self.end_optional_views_from(optional_mark, expression.span);
            return;
        }
        if let crate::hir::HirExpressionKind::Grouped(inner) = &expression.kind {
            self.lower_optional_call(inner, destination);
            return;
        }
        let (target, receiver, arguments) = self.lower_call_parts(expression);
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
        if let crate::hir::HirExpressionKind::NestedOptionalUnwrap(unwrap) = &expression.kind {
            self.lower_nested_optional_unwrap_at(MirPlace::base(destination), unwrap);
            self.end_optional_views_from(optional_mark, expression.span);
            return;
        }
        if let crate::hir::HirExpressionKind::Grouped(inner) = &expression.kind {
            self.lower_optional_shared_call(inner, destination);
            return;
        }
        let (target, receiver, arguments) = self.lower_call_parts(expression);
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
                    let ty = MirType::Optional(optional_types::scalar_id(
                        self.input.optional_types,
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
                        MirType::Optional(optional_types::class_id(
                            self.input.optional_types,
                            value.class,
                        )),
                        value.span,
                    );
                    self.lower_class_optional_initialize(storage, value);
                    LoweredArgument::Ready(MirArgument::OwnedPlace(MirPlace::base(storage)))
                }
                HirCallArgument::OptionalShared(value) => {
                    let storage = self.new_optional_storage(
                        MirStorageKind::Argument,
                        "optional-shared-argument",
                        MirType::Optional(optional_types::shared_id(
                            self.input.optional_types,
                            value.target,
                        )),
                        value.span,
                    );
                    self.lower_optional_shared_initialize(storage, value);
                    LoweredArgument::Ready(MirArgument::SharedOwner(storage))
                }
                HirCallArgument::AggregateOptional(value) => {
                    let storage = self.new_optional_storage(
                        MirStorageKind::Argument,
                        "aggregate-optional-argument",
                        MirType::Optional(value.optional),
                        value.span,
                    );
                    self.lower_aggregate_optional_initialize_at(MirPlace::base(storage), value);
                    LoweredArgument::Ready(MirArgument::OwnedPlace(MirPlace::base(storage)))
                }
                HirCallArgument::OptionalPlace(place) => {
                    let place = match place {
                        crate::hir::HirOptionalAliasPlace::Primitive(place) => {
                            self.lower_optional_place(place)
                        }
                        crate::hir::HirOptionalAliasPlace::Class(place) => {
                            self.lower_class_optional_place(place)
                        }
                        crate::hir::HirOptionalAliasPlace::Nested(place) => {
                            self.lower_aggregate_optional_place(place)
                        }
                    };
                    LoweredArgument::Ready(MirArgument::Place(place))
                }
                HirCallArgument::OptionalSharedPlace(place) => LoweredArgument::Ready(
                    MirArgument::Place(self.lower_optional_shared_place(place)),
                ),
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
                HirCallArgument::ProducedPrimitiveAlias(expression) => {
                    let value = self
                        .lower_expression(expression)
                        .expect("produced primitive alias must produce a scalar value");
                    let ty = self.lower_type(expression.ty);
                    debug_assert!(ty.is_primitive());
                    let storage = StorageId::new(self.input.callable, self.storage.len());
                    self.storage.push(MirStorage {
                        id: storage,
                        source: None,
                        name: format!("primitive-alias-{}", storage.index()),
                        kind: MirStorageKind::PrimitiveAlias,
                        ty,
                        span: expression.span,
                    });
                    self.track_full_expression_storage(storage, expression.span);
                    self.emit(MirInstruction::Store(MirStore {
                        destination: MirPlace::base(storage),
                        value,
                        authorization: None,
                        final_authorization: None,
                        span: expression.span,
                    }));
                    LoweredArgument::Ready(MirArgument::Place(MirPlace::base(storage)))
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
                HirCallArgument::SharedPlace(place) => {
                    LoweredArgument::Ready(MirArgument::Place(self.lower_shared_place(place)))
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
                        crate::hir::HirArrayAliasSource::OptionalPayload {
                            source,
                            optional,
                            array,
                        } => {
                            self.lower_optional_array_alias_place_with_anchor(
                                source, *optional, *array, alias.span,
                            )
                            .0
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
        self.lower_object_view_with_retention(view, false)
    }

    pub(super) fn lower_iteration_object_view(&mut self, view: &HirObjectView) -> MirObjectView {
        self.lower_object_view_with_retention(view, true)
    }

    fn lower_object_view_with_retention(
        &mut self,
        view: &HirObjectView,
        retain_replaceable_owner: bool,
    ) -> MirObjectView {
        let produced_class = match &view.source {
            HirViewSource::Produced { .. } => {
                let crate::hir::HirObjectOrigin::Produced { dynamic_class, .. } =
                    view.origin.as_ref()
                else {
                    unreachable!("produced HIR view must retain produced exact origin")
                };
                Some(*dynamic_class)
            }
            HirViewSource::ArrayElement(element) => {
                let Type::Class(class) = element.element else {
                    unreachable!("object view array source must have exact class type")
                };
                Some(class)
            }
            HirViewSource::OptionalPayload { view, .. } => Some(optional_types::class_payload(
                self.input.optional_types,
                &view.source,
            )),
            _ => None,
        };
        let source = match &view.source {
            HirViewSource::Place(place) => self.lower_object_place(place),
            HirViewSource::Static { place, projections } => projections
                .iter()
                .fold(MirPlace::static_field(place.field), lower_projection),
            HirViewSource::ArrayElement(element) => {
                self.lower_array_alias_element_place(element, element.receiver.access)
            }
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
            HirViewSource::OptionalBoxPayload {
                view: optional_box,
                projections,
            } => {
                let owner = match &optional_box.source {
                    HirSharedSource::Place(HirSharedPlace::Binding { binding, .. })
                        if !retain_replaceable_owner =>
                    {
                        self.storage_for_binding(*binding)
                    }
                    source => self.new_shared_anchor(source, optional_box.span),
                };
                projections.iter().fold(
                    self.begin_optional_box_view(owner, optional_box.box_target, optional_box.span),
                    lower_projection,
                )
            }
        };
        let produced_complete = match &view.source {
            HirViewSource::Produced { projections, .. } => {
                let complete_projection_count = projections
                    .iter()
                    .rposition(|projection| {
                        matches!(projection, crate::object_path::ObjectProjection::Field(_))
                    })
                    .map_or(0, |index| index + 1);
                Some(projections[..complete_projection_count].iter().fold(
                    MirPlace::base(source.base.expect_local_storage()),
                    lower_projection,
                ))
            }
            HirViewSource::ArrayElement(_) => Some(source.clone()),
            _ => None,
        };
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
                HirViewSource::OptionalBoxPayload {
                    view: optional_box, ..
                } => MirObjectOrigin::Shared {
                    owner: source.base.expect_local_storage(),
                    static_target: type_operations::lower_view_target(optional_box.target),
                    access: type_operations::lower_access(optional_box.access),
                    exact_dynamic_class: optional_box.source.exact_dynamic_class(),
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
            provenance: if matches!(view.source, HirViewSource::Produced { .. }) {
                MirViewProvenance::Produced
            } else {
                MirViewProvenance::Ordinary
            },
            span: view.span,
        }
    }

    pub(super) fn lower_method_receiver(
        &mut self,
        receiver: &HirMethodReceiver,
    ) -> MirMethodReceiver {
        match receiver {
            HirObjectReceiver::Place { place, origin } => MirMethodReceiver {
                place: self.lower_object_place(place),
                origin: Box::new(self.lower_object_origin(origin)),
                access: type_operations::lower_access(place.access),
                provenance: MirViewProvenance::Ordinary,
            },
            HirObjectReceiver::Checked { view, .. } => {
                let view = self.lower_checked_object_view(view);
                MirMethodReceiver {
                    place: view.source,
                    origin: view.origin,
                    access: view.access,
                    provenance: MirViewProvenance::Ordinary,
                }
            }
            HirObjectReceiver::View { view, .. } => {
                let view = self.lower_object_view(view);
                MirMethodReceiver {
                    place: view.source,
                    origin: view.origin,
                    access: view.access,
                    provenance: view.provenance,
                }
            }
            HirObjectReceiver::ArrayElement { element, .. } => {
                let place = self.lower_array_alias_element_place(element, element.receiver.access);
                let Type::Class(dynamic_class) = element.element else {
                    unreachable!("object method array receiver must have exact class type")
                };
                MirMethodReceiver::exact(
                    place,
                    dynamic_class,
                    type_operations::lower_access(element.receiver.access),
                )
            }
        }
    }

    pub(super) fn lower_checked_object_view(
        &mut self,
        checked: &crate::hir::HirCheckedObjectView,
    ) -> MirObjectView {
        self.lower_checked_object_view_with_retention(checked, false)
    }

    pub(super) fn lower_iteration_checked_object_view(
        &mut self,
        checked: &crate::hir::HirCheckedObjectView,
    ) -> MirObjectView {
        self.lower_checked_object_view_with_retention(checked, true)
    }

    fn lower_checked_object_view_with_retention(
        &mut self,
        checked: &crate::hir::HirCheckedObjectView,
        retain_replaceable_owner: bool,
    ) -> MirObjectView {
        let source = self.lower_object_view_with_retention(&checked.view, retain_replaceable_owner);
        let direct_static_source = matches!(
            checked.view.source,
            HirViewSource::Place(_)
                | HirViewSource::Static { .. }
                | HirViewSource::ArrayElement(_)
                | HirViewSource::Produced { .. }
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
                provenance: MirViewProvenance::Ordinary,
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
            provenance: MirViewProvenance::Ordinary,
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
            HirObjectOrigin::Static {
                place,
                dynamic_class,
            } => MirObjectOrigin::Exact {
                complete: MirPlace::static_field(place.field),
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
