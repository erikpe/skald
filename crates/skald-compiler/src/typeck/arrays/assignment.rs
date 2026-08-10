//! Whole-array replacement, element writes, and slice writes.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirAccess, HirArrayAssignment, HirArrayElementAssignment, HirArrayElementPlace,
        HirArrayElementValue, HirArrayEvaluationOrder, HirArrayPlace, HirArrayRuntimeFailure,
        HirArraySlice, HirArraySliceAssignment, HirArraySliceBounds, HirExpressionKind,
        HirStatement, Type,
    },
    identity::BindingId,
    resolve::{
        ResolvedArrayAssignment, ResolvedArrayProjectionBounds, ResolvedArrayProjectionExpr,
        ResolvedArrayProjectionOperator, ResolvedExpression, ResolvedParameterBindingMode,
    },
};

use super::{place::ArrayReceiverSyntax, ARRAY_CAPABILITY_UNAVAILABLE};
use crate::typeck::{
    expression::require_type,
    function::CallableChecker,
    program::{INVALID_ALIAS_ARGUMENT, READ_ONLY_RECEIVER},
};

impl CallableChecker<'_, '_> {
    pub(in crate::typeck) fn check_array_assignment(
        &mut self,
        assignment: &ResolvedArrayAssignment,
    ) -> Option<HirStatement> {
        match self.array_assignment_kind(&assignment.destination) {
            Some(ArrayAssignmentKind::Whole(place)) => {
                if !self.require_mutable_array_access(place.access(), place.span()) {
                    return None;
                }
                let value = self.check_array_initialize(
                    place.array(),
                    &assignment.source,
                    "whole-array replacement",
                );
                value.map(|value| {
                    HirStatement::ArrayAssignment(HirArrayAssignment {
                        destination: *place,
                        value,
                        evaluation: HirArrayEvaluationOrder::DestinationThenSourceThenReplace,
                        span: assignment.span,
                    })
                })
            }
            Some(ArrayAssignmentKind::Projection(projection)) => {
                self.check_array_projection_assignment(projection, assignment)
            }
            None => None,
        }
    }

    fn array_assignment_kind<'a>(
        &mut self,
        destination: &'a ResolvedExpression,
    ) -> Option<ArrayAssignmentKind<'a>> {
        match destination {
            ResolvedExpression::Binding(binding) => {
                let Type::Array(array) = self.binding_type(binding.binding) else {
                    return None;
                };
                if let BindingId::Parameter(parameter) = binding.binding {
                    if !matches!(
                        self.parameter(parameter).binding_mode,
                        ResolvedParameterBindingMode::Value
                    ) {
                        self.diagnostics.push(
                            Diagnostic::error(
                                INVALID_ALIAS_ARGUMENT,
                                "an array alias root cannot be rebound",
                            )
                            .with_primary_label(
                                binding.span,
                                "mutate elements or slices through this alias instead",
                            ),
                        );
                        return None;
                    }
                }
                let access = self.binding_access(binding.binding, false, binding.span)?;
                Some(ArrayAssignmentKind::Whole(Box::new(
                    HirArrayPlace::Binding {
                        binding: binding.binding,
                        array,
                        access,
                        span: binding.span,
                    },
                )))
            }
            ResolvedExpression::FieldAccess(access) => {
                let checked = self.check_field_read(access)?;
                let Type::Array(array) = checked.ty else {
                    return None;
                };
                let HirExpressionKind::FieldRead(place) = checked.kind else {
                    unreachable!("checked field access must remain a field read")
                };
                let access = place.receiver.access;
                Some(ArrayAssignmentKind::Whole(Box::new(HirArrayPlace::Field {
                    place,
                    array,
                    access,
                    span: checked.span,
                })))
            }
            ResolvedExpression::StaticFieldAccess(access) => {
                let (place, ty) = self.check_static_place(access.field, access.span)?;
                let Type::Array(array) = ty else {
                    return None;
                };
                Some(ArrayAssignmentKind::Whole(Box::new(
                    HirArrayPlace::Static {
                        place,
                        array,
                        span: access.span,
                    },
                )))
            }
            ResolvedExpression::ArrayProjection(projection) => {
                Some(ArrayAssignmentKind::Projection(projection))
            }
            ResolvedExpression::Grouped(grouped) => self.array_assignment_kind(&grouped.expression),
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(
                        super::ARRAY_PROJECTION_REQUIRES_ARRAY,
                        "array assignment destination must be an owning array place",
                    )
                    .with_primary_label(destination.span(), "this expression is not assignable"),
                );
                None
            }
        }
    }

    fn check_array_projection_assignment(
        &mut self,
        projection: &ResolvedArrayProjectionExpr,
        assignment: &ResolvedArrayAssignment,
    ) -> Option<HirStatement> {
        let receiver = self.check_array_receiver(
            &projection.receiver,
            match projection.operator {
                ResolvedArrayProjectionOperator::Ordinary { .. } => ArrayReceiverSyntax::Ordinary,
                ResolvedArrayProjectionOperator::Shared { .. } => ArrayReceiverSyntax::Shared,
            },
        );
        let mut receiver = receiver?;
        if receiver.ownership == crate::hir::HirArrayReceiverOwnership::Inline {
            receiver.anchor = crate::hir::HirArrayAnchor::InlineBacking;
        }
        if !self.require_mutable_array_access(receiver.access, projection.span) {
            return None;
        }
        let array = receiver.array;
        let lifecycle = self.copy_capabilities.array(array).lifecycle.clone();
        let Some(operation) = lifecycle.assignment else {
            self.diagnostics.push(
                Diagnostic::error(
                    ARRAY_CAPABILITY_UNAVAILABLE,
                    "array element type is not assignable",
                )
                .with_primary_label(projection.span, "this write requires element assignment"),
            );
            return None;
        };
        match &projection.bounds {
            ResolvedArrayProjectionBounds::Index(index) => {
                let index = self.check_array_index(index)?;
                let element = self.copy_capabilities.array(array).element;
                let destination = HirArrayElementPlace {
                    receiver,
                    index,
                    element,
                    evaluation: HirArrayEvaluationOrder::ReceiverThenIndex,
                    span: projection.span,
                };
                let value = self.check_array_element_value(element, &assignment.source, operation);
                value.map(|value| {
                    HirStatement::ArrayElementAssignment(Box::new(HirArrayElementAssignment {
                        destination,
                        value,
                        operation,
                        evaluation: HirArrayEvaluationOrder::DestinationThenSourceThenStore,
                        span: assignment.span,
                    }))
                })
            }
            ResolvedArrayProjectionBounds::Slice {
                start,
                colon_span,
                end,
            } => {
                let start = self.check_array_bound(start.as_deref(), "slice start")?;
                let end = self.check_array_bound(end.as_deref(), "slice end")?;
                let source = self.array_source_from_slice_or_value(&assignment.source, array)?;
                let destination = HirArraySlice {
                    receiver,
                    bounds: HirArraySliceBounds {
                        start,
                        end,
                        normalization: crate::hir::HirArrayIndexNormalization::SignedFromEndOnce,
                        failure: HirArrayRuntimeFailure::InvalidSliceBoundsTerminate,
                        span: *colon_span,
                    },
                    array,
                    element_copy: None,
                    evaluation: HirArrayEvaluationOrder::ReceiverThenBounds,
                    span: projection.span,
                };
                Some(HirStatement::ArraySliceAssignment(
                    HirArraySliceAssignment {
                        destination,
                        source,
                        operation,
                        failure: HirArrayRuntimeFailure::SliceLengthMismatchTerminate,
                        evaluation:
                            HirArrayEvaluationOrder::DestinationBoundsThenSourceThenLengthCheckThenCopy,
                        span: assignment.span,
                    },
                ))
            }
        }
    }

    fn check_array_element_value(
        &mut self,
        element: Type,
        source: &ResolvedExpression,
        operation: crate::hir::HirArrayAssignElement,
    ) -> Option<HirArrayElementValue> {
        match element {
            Type::Array(array) => self
                .check_array_initialize(array, source, "nested array element assignment")
                .map(HirArrayElementValue::Array),
            Type::Shared(target) => self
                .check_shared_transfer(source, target, "shared array element assignment")
                .map(HirArrayElementValue::Shared),
            Type::Optional(_) => match self
                .optional_kind(element)
                .expect("enabled optional array elements must have legacy metadata")
            {
                super::super::optional_types::OptionalPayloadKind::Primitive(payload) => self
                    .check_optional_source(source, payload, "optional array element assignment")
                    .map(|source| HirArrayElementValue::Optional { source, payload }),
                super::super::optional_types::OptionalPayloadKind::Class(class) => self
                    .check_class_optional_initialize(
                        class,
                        source,
                        "optional class element assignment",
                    )
                    .map(HirArrayElementValue::ClassOptional),
                super::super::optional_types::OptionalPayloadKind::Shared(target) => self
                    .check_optional_shared_initialize(
                        target,
                        source,
                        "optional shared array element assignment",
                    )
                    .map(HirArrayElementValue::OptionalShared),
                super::super::optional_types::OptionalPayloadKind::Nested(_)
                | super::super::optional_types::OptionalPayloadKind::Array(_) => {
                    let Type::Optional(optional) = element else {
                        unreachable!()
                    };
                    self.check_optional_value(
                        optional,
                        source,
                        "nested optional array element assignment",
                    )
                    .map(|value| HirArrayElementValue::NestedOptional(Box::new(value)))
                }
            },
            Type::Class(class) => {
                let source =
                    self.check_object_source(source, class, "class array element assignment")?;
                let crate::hir::HirArrayAssignElement::Class { operation, .. } = operation else {
                    unreachable!("class array assignment must retain its copy operation")
                };
                Some(HirArrayElementValue::Object { source, operation })
            }
            _ => {
                let value = self.check_expression(source)?;
                require_type(
                    value.ty,
                    element,
                    value.span,
                    "array element assignment",
                    self.diagnostics,
                )
                .then_some(HirArrayElementValue::Value(value))
            }
        }
    }

    fn require_mutable_array_access(
        &mut self,
        access: HirAccess,
        span: crate::source::Span,
    ) -> bool {
        if access == HirAccess::Mutable {
            return true;
        }
        self.diagnostics.push(
            Diagnostic::error(READ_ONLY_RECEIVER, "cannot mutate a read-only array")
                .with_primary_label(span, "array assignment requires mutable access"),
        );
        false
    }
}

enum ArrayAssignmentKind<'a> {
    Whole(Box<HirArrayPlace>),
    Projection(&'a ResolvedArrayProjectionExpr),
}
