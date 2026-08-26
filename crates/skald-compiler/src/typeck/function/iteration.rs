//! Core structured-HIR planning for nominal general iteration.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirAccess, HirForIn, HirIterationCallTarget, HirIterationItemPlan,
        HirIterationNextCallPlan, HirIterationProtocol, HirIterationReceiver,
        HirIterationReceiverCarrier, HirIterationReceiverLifetime, HirIterationResultPlan,
        HirIterationSpans, HirIterationStateAlias, HirIterationStateCallPlan,
        HirIterationStatePlan, HirIterationStoredValuePlan, HirIterationValueCopy,
        HirIterationValueDestruction, HirOptionalDestructionPlan, HirOptionalPresenceTestPlan,
        HirOptionalUnwrapPlan, HirStatement, HirViewTarget, Type,
    },
    resolve::ResolvedForIn,
};

use super::{
    super::program::{lower_type, lower_type_kind, GENERAL_ITERATION_UNSUPPORTED},
    CallableChecker, CheckedStatement,
};

impl CallableChecker<'_, '_> {
    pub(super) fn check_for_in_statement(&mut self, statement: &ResolvedForIn) -> CheckedStatement {
        let item_type = lower_type_kind(statement.selection.item);
        let state_type = lower_type_kind(statement.selection.state);
        let mut receiver = self.check_iteration_receiver(statement);
        let state_value = self.check_iteration_state(state_type, statement);
        let item_value = self.check_iteration_item(item_type, statement);
        let result = self.check_iteration_result(item_type, statement);

        // The binding is immutable only in its own body. Nested scopes retain
        // ordinary shadowing because they use distinct local identities.
        let inserted = self.read_only_locals.insert(statement.binding);
        debug_assert!(
            inserted,
            "an iteration binding is active only in its own body"
        );
        let body = self.check_block(&statement.body);
        let removed = self.read_only_locals.remove(&statement.binding);
        debug_assert!(removed);
        if let Some(binding) = receiver
            .as_ref()
            .and_then(iteration_guarded_optional_binding)
        {
            if let Some(span) = guarded_optional_write(&body, binding) {
                self.diagnostics.push(
                    Diagnostic::error(
                        GENERAL_ITERATION_UNSUPPORTED,
                        "loop body cannot replace a guarded optional iteration receiver",
                    )
                    .with_primary_label(span, "this write would invalidate the retained payload view")
                    .with_secondary_label(
                        statement.iterable.span(),
                        "the optional payload is retained for the whole loop",
                    )
                    .with_note("copy or move the iterable into an independent local before iterating if the original optional must be replaced"),
                );
                receiver = None;
            }
        }
        let effects = body.effects.clone().through_loop(statement.loop_id);

        let hir = receiver.zip(state_value).zip(item_value).zip(result).map(
            |(((receiver, state_value), item_value), result)| {
                let protocol = HirIterationProtocol {
                    interface: statement.selection.interface,
                    iter_state: statement.selection.iter_state,
                    iter_next: statement.selection.iter_next,
                    item: item_type,
                    state: state_type,
                    result: result.optional,
                };
                let target = |requirement| HirIterationCallTarget {
                    interface: protocol.interface,
                    requirement,
                };
                let state = HirIterationStatePlan {
                    value: state_value,
                    initialize: HirIterationStateCallPlan {
                        target: target(protocol.iter_state),
                        receiver_access: HirAccess::ReadOnly,
                        result: state_type,
                    },
                    advance: HirIterationNextCallPlan {
                        target: target(protocol.iter_next),
                        receiver_access: HirAccess::ReadOnly,
                        state_alias: HirIterationStateAlias {
                            ty: state_type,
                            access: HirAccess::Mutable,
                        },
                        result: Type::Optional(result.optional),
                    },
                };
                let item = HirIterationItemPlan {
                    binding: statement.binding,
                    access: HirAccess::ReadOnly,
                    value: item_value,
                };
                HirStatement::ForIn(Box::new(HirForIn::new(
                    statement.loop_id,
                    statement.binding,
                    protocol,
                    receiver,
                    state,
                    result,
                    item,
                    body,
                    HirIterationSpans {
                        for_span: statement.for_span,
                        binding_span: statement.binding_span,
                        annotation_span: statement.annotation_span,
                        in_span: statement.in_span,
                        iterable_span: statement.iterable.span(),
                        span: statement.span,
                    },
                )))
            },
        );
        CheckedStatement::with_effects(hir, effects)
    }

    fn check_iteration_receiver(
        &mut self,
        statement: &ResolvedForIn,
    ) -> Option<HirIterationReceiver> {
        let target = HirViewTarget::Interface(statement.selection.interface);
        let (iterable, carrier) = if let Some(cast) = iteration_cast(&statement.iterable) {
            let mut checked = self.check_object_cast(cast)?;
            let iterable = view_target_type(checked.view.target);
            anchor_checked_iteration_source(&mut checked.view);
            checked.consumer_target = target;
            checked.consumer_access = HirAccess::ReadOnly;
            (
                iterable,
                HirIterationReceiverCarrier::Checked(Box::new(checked)),
            )
        } else {
            let (iterable, view) = self.check_iteration_view(&statement.iterable, target)?;
            (iterable, HirIterationReceiverCarrier::View(view))
        };
        Some(HirIterationReceiver {
            iterable,
            carrier,
            lifetime: HirIterationReceiverLifetime::LoopDuration,
        })
    }

    fn check_iteration_state(
        &mut self,
        state: Type,
        statement: &ResolvedForIn,
    ) -> Option<HirIterationStoredValuePlan> {
        self.iteration_value_plan(
            state,
            false,
            statement.selection.origin_span,
            "iteration state",
        )
    }

    fn check_iteration_item(
        &mut self,
        item: Type,
        statement: &ResolvedForIn,
    ) -> Option<HirIterationStoredValuePlan> {
        self.iteration_value_plan(item, true, statement.binding_span, "iteration item")
    }

    fn check_iteration_result(
        &mut self,
        item: Type,
        statement: &ResolvedForIn,
    ) -> Option<HirIterationResultPlan> {
        let interface = self
            .program
            .interface(statement.selection.interface)
            .expect("selected iteration interface must exist");
        let requirement = interface
            .requirements
            .get(statement.selection.iter_next.index())
            .filter(|requirement| requirement.id == statement.selection.iter_next)
            .expect("selected iter_next requirement must exist");
        let Type::Optional(optional) = lower_type(self.program, &requirement.return_type) else {
            unreachable!("canonical iter_next must return the selected optional item")
        };
        let metadata = self
            .program
            .optional_types
            .get(optional)
            .expect("selected iteration result must have canonical optional metadata");
        debug_assert_eq!(lower_type(self.program, &metadata.payload), item);
        let (presence, unwrap, destruction) = match item {
            Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool => (
                HirOptionalPresenceTestPlan::OuterTag,
                HirOptionalUnwrapPlan::ExtractScalar,
                HirOptionalDestructionPlan::Trivial,
            ),
            Type::Class(class) => (
                HirOptionalPresenceTestPlan::OuterTag,
                HirOptionalUnwrapPlan::CheckedInlineClass(class),
                HirOptionalDestructionPlan::Class(class),
            ),
            Type::Array(array) => (
                HirOptionalPresenceTestPlan::OuterTag,
                HirOptionalUnwrapPlan::CheckedInlineArray(array),
                HirOptionalDestructionPlan::Array(array),
            ),
            Type::Shared(target) => (
                HirOptionalPresenceTestPlan::SharedOwnerNull,
                HirOptionalUnwrapPlan::SecureSharedOwner(target),
                HirOptionalDestructionPlan::Shared(target),
            ),
            Type::Optional(nested) => (
                HirOptionalPresenceTestPlan::OuterTag,
                HirOptionalUnwrapPlan::CheckedNested(nested),
                HirOptionalDestructionPlan::Optional(nested),
            ),
            Type::Unit | Type::Obj | Type::Interface(_) | Type::Function(_) => return None,
        };
        Some(HirIterationResultPlan {
            optional,
            payload: item,
            presence,
            unwrap,
            destruction,
        })
    }

    fn iteration_value_plan(
        &mut self,
        ty: Type,
        require_copy: bool,
        span: crate::source::Span,
        owner: &'static str,
    ) -> Option<HirIterationStoredValuePlan> {
        let (copy, destruction) = match ty {
            Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool => (
                Some(HirIterationValueCopy::Trivial),
                HirIterationValueDestruction::Trivial,
            ),
            Type::Class(class) => (
                self.copy_capabilities
                    .constructor(class)
                    .selected()
                    .map(|operation| HirIterationValueCopy::Class { class, operation }),
                HirIterationValueDestruction::Class(class),
            ),
            Type::Array(array) => (
                self.copy_capabilities
                    .array(array)
                    .lifecycle
                    .copy
                    .map(|operation| HirIterationValueCopy::Array { array, operation }),
                HirIterationValueDestruction::Array(array),
            ),
            Type::Shared(target) => (
                Some(HirIterationValueCopy::Shared(target)),
                HirIterationValueDestruction::Shared(target),
            ),
            Type::Optional(optional) => {
                let copy = super::super::optional_types::selected_copy_plan(
                    self.program,
                    self.copy_capabilities,
                    optional,
                )
                .map(|operation| HirIterationValueCopy::Optional {
                    optional,
                    operation,
                });
                let plan = iteration_optional_destruction(self.program, optional);
                (
                    copy,
                    HirIterationValueDestruction::Optional { optional, plan },
                )
            }
            Type::Unit | Type::Obj | Type::Interface(_) | Type::Function(_) => {
                self.report_iteration_value_family(
                    ty,
                    span,
                    owner,
                    "iteration requires an ordinary owning stored-value type",
                );
                return None;
            }
        };
        if require_copy && copy.is_none() {
            match ty {
                Type::Class(class) => self.report_unavailable_copy_operation(class, true, span),
                Type::Array(_) => self.diagnostics.push(
                    Diagnostic::error(
                        super::super::arrays::ARRAY_CAPABILITY_UNAVAILABLE,
                        "array element type is not copy-constructible",
                    )
                    .with_primary_label(span, "yielding this item requires a deep array copy"),
                ),
                Type::Optional(_) => self.diagnostics.push(
                    Diagnostic::error(
                        super::super::COPY_OPERATION_UNAVAILABLE,
                        "the optional iteration item cannot be copied",
                    )
                    .with_primary_label(
                        span,
                        "yielding this item requires optional copy capability",
                    ),
                ),
                _ => unreachable!("all other admitted iteration item families are copyable"),
            }
            return None;
        }
        Some(HirIterationStoredValuePlan {
            ty,
            copy,
            destruction,
        })
    }

    fn report_iteration_value_family(
        &mut self,
        ty: Type,
        span: crate::source::Span,
        owner: &'static str,
        note: &'static str,
    ) {
        self.diagnostics.push(
            Diagnostic::error(
                GENERAL_ITERATION_UNSUPPORTED,
                format!(
                    "{owner} type `{}` is not in the implemented core matrix",
                    ty.name()
                ),
            )
            .with_primary_label(span, "unsupported general-iteration value family")
            .with_note(note),
        );
    }
}

fn iteration_cast(
    expression: &crate::resolve::ResolvedExpression,
) -> Option<&crate::resolve::ResolvedObjectCastExpr> {
    match expression {
        crate::resolve::ResolvedExpression::ObjectCast(cast) => Some(cast),
        crate::resolve::ResolvedExpression::Grouped(grouped) => iteration_cast(&grouped.expression),
        _ => None,
    }
}

const fn view_target_type(target: HirViewTarget) -> Type {
    match target {
        HirViewTarget::Class(class) => Type::Class(class),
        HirViewTarget::Interface(interface) => Type::Interface(interface),
        HirViewTarget::Obj => Type::Obj,
    }
}

fn anchor_checked_iteration_source(view: &mut crate::hir::HirObjectView) {
    let crate::hir::HirViewSource::Shared {
        binding,
        target,
        access,
        projections,
        span,
    } = &view.source
    else {
        return;
    };
    let binding = *binding;
    let target = *target;
    let access = *access;
    let projections = projections.clone();
    let span = *span;
    view.source = crate::hir::HirViewSource::AnchoredShared {
        source: Box::new(crate::hir::HirSharedSource::Place(
            crate::hir::HirSharedPlace::Binding {
                binding,
                target: view_shared_target(target),
                span,
            },
        )),
        target,
        access,
        projections,
        span,
    };
    *view.origin = crate::hir::HirObjectOrigin::AnchoredShared {
        static_target: target,
        access,
        span,
    };
}

const fn view_shared_target(target: HirViewTarget) -> crate::hir::HirSharedTarget {
    match target {
        HirViewTarget::Class(class) => crate::hir::HirSharedTarget::Class(class),
        HirViewTarget::Interface(interface) => crate::hir::HirSharedTarget::Interface(interface),
        HirViewTarget::Obj => crate::hir::HirSharedTarget::Obj,
    }
}

fn iteration_guarded_optional_binding(
    receiver: &HirIterationReceiver,
) -> Option<crate::identity::BindingId> {
    let HirIterationReceiverCarrier::View(view) = &receiver.carrier else {
        return None;
    };
    let crate::hir::HirViewSource::OptionalPayload { view, .. } = &view.source else {
        return None;
    };
    let crate::hir::HirOptionalOperand::ClassPlace(place) = &view.source else {
        return None;
    };
    let crate::hir::HirOptionalStorage::Binding(binding) = &place.storage else {
        return None;
    };
    Some(*binding)
}

fn guarded_optional_write(
    block: &crate::hir::HirBlock,
    binding: crate::identity::BindingId,
) -> Option<crate::source::Span> {
    for statement in &block.statements {
        if let crate::hir::HirStatement::ClassOptionalAssignment(assignment) = statement {
            if matches!(
                assignment.destination.storage,
                crate::hir::HirOptionalStorage::Binding(candidate) if candidate == binding
            ) {
                return Some(assignment.span);
            }
        }
        let nested = match statement {
            crate::hir::HirStatement::Conditional(conditional) => conditional
                .arms
                .iter()
                .find_map(|arm| guarded_optional_write(&arm.body, binding))
                .or_else(|| {
                    conditional
                        .else_block
                        .as_ref()
                        .and_then(|body| guarded_optional_write(body, binding))
                }),
            crate::hir::HirStatement::While(loop_) => guarded_optional_write(&loop_.body, binding),
            crate::hir::HirStatement::ForIn(loop_) => guarded_optional_write(&loop_.body, binding),
            crate::hir::HirStatement::Block(block) => guarded_optional_write(block, binding),
            _ => None,
        };
        if nested.is_some() {
            return nested;
        }
    }
    None
}

fn iteration_optional_destruction(
    program: &crate::resolve::ResolvedProgram,
    optional: crate::identity::OptionalTypeId,
) -> HirOptionalDestructionPlan {
    match super::super::optional_types::classify_payload(program, optional)
        .expect("validated optional payload must be a stored value")
    {
        super::super::optional_types::OptionalPayloadKind::Primitive(_) => {
            HirOptionalDestructionPlan::Trivial
        }
        super::super::optional_types::OptionalPayloadKind::Class(class) => {
            HirOptionalDestructionPlan::Class(class)
        }
        super::super::optional_types::OptionalPayloadKind::Array(array) => {
            HirOptionalDestructionPlan::Array(array)
        }
        super::super::optional_types::OptionalPayloadKind::Shared(target) => {
            HirOptionalDestructionPlan::Shared(target)
        }
        super::super::optional_types::OptionalPayloadKind::Nested(nested) => {
            HirOptionalDestructionPlan::Optional(nested)
        }
    }
}
