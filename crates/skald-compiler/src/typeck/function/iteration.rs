//! Core structured-HIR planning for nominal general iteration.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirAccess, HirForIn, HirIterationCallTarget, HirIterationItemPlan,
        HirIterationNextCallPlan, HirIterationProtocol, HirIterationReceiver,
        HirIterationReceiverLifetime, HirIterationResultPlan, HirIterationSpans,
        HirIterationStateAlias, HirIterationStateCallPlan, HirIterationStatePlan,
        HirIterationStoredValuePlan, HirIterationValueDestruction, HirIterationValueInitialization,
        HirOptionalDestructionPlan, HirOptionalPresenceTestPlan, HirOptionalUnwrapPlan,
        HirStatement, HirViewTarget, Type,
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
        let receiver = self.check_iteration_receiver(statement);
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
        let (iterable, view) = match self.check_core_iteration_view(
            &statement.iterable,
            HirViewTarget::Interface(statement.selection.interface),
        )? {
            Ok(receiver) => receiver,
            Err(span) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        GENERAL_ITERATION_UNSUPPORTED,
                        "this iteration receiver family is not implemented yet",
                    )
                    .with_primary_label(
                        span,
                        "the current core supports named class and interface-view receivers",
                    ),
                );
                return None;
            }
        };
        Some(HirIterationReceiver {
            iterable,
            view,
            lifetime: HirIterationReceiverLifetime::LoopDuration,
        })
    }

    fn check_iteration_state(
        &mut self,
        state: Type,
        statement: &ResolvedForIn,
    ) -> Option<HirIterationStoredValuePlan> {
        if is_primitive_value(state) {
            return Some(trivial_value(state));
        }
        self.report_iteration_value_family(
            state,
            statement.selection.origin_span,
            "iteration state",
            "the current core requires primitive `State`",
        );
        None
    }

    fn check_iteration_item(
        &mut self,
        item: Type,
        statement: &ResolvedForIn,
    ) -> Option<HirIterationStoredValuePlan> {
        if is_primitive_value(item) {
            return Some(trivial_value(item));
        }
        let Type::Class(class) = item else {
            self.report_iteration_value_family(
                item,
                statement.binding_span,
                "iteration item",
                "the current core supports primitive or trivially copied exact-class `Item`",
            );
            return None;
        };
        let capability = self.copy_capabilities.constructor(class);
        let Some(operation) = capability.selected() else {
            self.report_unavailable_copy_operation(class, true, statement.binding_span);
            return None;
        };
        let trivially_copied = matches!(
            capability,
            crate::hir::HirCopyCapability::Synthesized(copy)
                if copy.base.is_none()
                    && copy.fields.iter().all(|field| matches!(
                        field,
                        crate::hir::HirSynthesizedFieldCopy::Scalar { .. }
                    ))
        );
        if !trivially_copied {
            self.report_iteration_value_family(
                item,
                statement.binding_span,
                "iteration item",
                "non-trivial class item copying is implemented in a later roadmap task",
            );
            return None;
        }
        Some(HirIterationStoredValuePlan {
            ty: item,
            initialization: HirIterationValueInitialization::CopyClass { class, operation },
            destruction: HirIterationValueDestruction::Class(class),
        })
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
        let (unwrap, destruction) = match item {
            Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool => (
                HirOptionalUnwrapPlan::ExtractScalar,
                HirOptionalDestructionPlan::Trivial,
            ),
            Type::Class(class) => (
                HirOptionalUnwrapPlan::CheckedInlineClass(class),
                HirOptionalDestructionPlan::Class(class),
            ),
            _ => return None,
        };
        Some(HirIterationResultPlan {
            optional,
            payload: item,
            presence: HirOptionalPresenceTestPlan::OuterTag,
            unwrap,
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

fn is_primitive_value(ty: Type) -> bool {
    matches!(
        ty,
        Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool
    )
}

fn trivial_value(ty: Type) -> HirIterationStoredValuePlan {
    HirIterationStoredValuePlan {
        ty,
        initialization: HirIterationValueInitialization::Trivial,
        destruction: HirIterationValueDestruction::Trivial,
    }
}
