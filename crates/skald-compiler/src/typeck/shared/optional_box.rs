//! Optional-box construction and exact-wrapper copy selection.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirOptionalBoxAllocation, HirOptionalBoxEvaluationOrder, HirOwnerTransfer, HirSharedTarget,
        HirStoredValueInitialization, Type,
    },
    identity::{OptionalBoxTypeId, OptionalTypeId},
    resolve::{
        ResolvedDereferenceExpr, ResolvedExpression, ResolvedOptionalBoxAllocationExpr,
        ResolvedOptionalBoxInitializer, ResolvedSharedTarget,
    },
};

use super::super::{
    function::CallableChecker,
    program::{COPY_OPERATION_UNAVAILABLE, INVALID_SHARED_CONVERSION},
};

impl CallableChecker<'_, '_> {
    pub(super) fn check_optional_box_allocation(
        &mut self,
        allocation: &ResolvedOptionalBoxAllocationExpr,
    ) -> Option<HirOptionalBoxAllocation> {
        let metadata = self
            .program
            .optional_box_types
            .get(allocation.target)
            .expect("resolved optional-box allocation must name metadata");
        debug_assert_eq!(metadata.optional, Some(allocation.exact_optional));

        let initialization = match &allocation.initializer {
            ResolvedOptionalBoxInitializer::Absent {
                left_paren_span, ..
            } => {
                let absent = ResolvedExpression::Absent(crate::resolve::ResolvedAbsentExpr {
                    span: *left_paren_span,
                });
                self.check_stored_value_initialization(
                    Type::Optional(allocation.exact_optional),
                    &absent,
                    "optional-box initialization",
                )?
            }
            ResolvedOptionalBoxInitializer::Value { value, .. } => {
                if let ResolvedExpression::Dereference(dereference) = &**value {
                    if matches!(dereference.target, ResolvedSharedTarget::OptionalBox(_)) {
                        self.check_optional_box_pointee_copy(
                            allocation.exact_optional,
                            allocation.target,
                            dereference,
                        )?
                    } else {
                        self.check_stored_value_initialization(
                            Type::Optional(allocation.exact_optional),
                            value,
                            "optional-box initialization",
                        )?
                    }
                } else {
                    self.check_stored_value_initialization(
                        Type::Optional(allocation.exact_optional),
                        value,
                        "optional-box initialization",
                    )?
                }
            }
        };
        Some(HirOptionalBoxAllocation {
            exact_optional: allocation.exact_optional,
            exact_target: allocation.target,
            exact_dynamic_class: self
                .program
                .optional_box_types
                .get(allocation.target)
                .and_then(|target| match target.object_leaf {
                    Some(crate::resolve::ResolvedObjectTarget::Class(class)) => Some(class),
                    Some(
                        crate::resolve::ResolvedObjectTarget::Interface(_)
                        | crate::resolve::ResolvedObjectTarget::Obj,
                    )
                    | None => None,
                }),
            static_target: allocation.target,
            initialization,
            evaluation: HirOptionalBoxEvaluationOrder::SourceThenAllocateThenInitializeThenPublish,
            produced_owner: HirOwnerTransfer::Adopt,
            new_span: allocation.new_span,
            target_span: allocation.target_span,
            publication_span: allocation.span,
            span: allocation.span,
        })
    }

    fn check_optional_box_pointee_copy(
        &mut self,
        exact_optional: OptionalTypeId,
        exact_target: OptionalBoxTypeId,
        dereference: &ResolvedDereferenceExpr,
    ) -> Option<HirStoredValueInitialization> {
        let source = self.check_shared_source(&dereference.source, false)?;
        if source.target() != HirSharedTarget::OptionalBox(exact_target) {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_SHARED_CONVERSION,
                    "optional-box pointee copying requires an exact box target",
                )
                .with_primary_label(
                    dereference.span,
                    "this static box view does not identify one exact wrapper layout",
                ),
            );
            return None;
        }
        let Some(operation) = super::super::optional_types::selected_copy_plan(
            self.program,
            self.copy_capabilities,
            exact_optional,
        ) else {
            self.diagnostics.push(
                Diagnostic::error(
                    COPY_OPERATION_UNAVAILABLE,
                    "the boxed optional wrapper cannot be copied",
                )
                .with_primary_label(
                    dereference.span,
                    "this independent box construction requires optional copy capability",
                ),
            );
            return None;
        };
        Some(HirStoredValueInitialization::OptionalBoxPointeeCopy {
            source,
            optional: exact_optional,
            operation,
            span: dereference.span,
        })
    }
}
