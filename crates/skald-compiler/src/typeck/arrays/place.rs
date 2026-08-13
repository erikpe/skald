//! Checked array receivers, indices, slices, and the intrinsic length query.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirAccess, HirArrayAnchor, HirArrayElementPlace, HirArrayEvaluationOrder, HirArrayIndex,
        HirArrayIndexNormalization, HirArrayLength, HirArrayReceiver, HirArrayReceiverOwnership,
        HirArrayReceiverSource, HirArrayRuntimeFailure, HirArraySlice, HirArraySliceBounds,
        HirArraySource, HirExpression, HirExpressionKind, HirSharedPlace, HirSharedProducer,
        HirSharedSource, HirSharedTarget, Type,
    },
    resolve::{
        ResolvedArrayLengthExpr, ResolvedArrayLengthOperator, ResolvedArrayProjectionBounds,
        ResolvedArrayProjectionExpr, ResolvedArrayProjectionOperator, ResolvedExpression,
    },
};

use super::super::{
    expression::require_type,
    function::CallableChecker,
    program::{IMPLICIT_SHARED_DEREFERENCE, WRONG_ARGUMENT_COUNT},
};
use super::ARRAY_CAPABILITY_UNAVAILABLE;

pub const ARRAY_PROJECTION_REQUIRES_ARRAY: &str = "TYP039";

impl CallableChecker<'_, '_> {
    pub(in crate::typeck) fn check_array_length(
        &mut self,
        length: &ResolvedArrayLengthExpr,
    ) -> Option<HirExpression> {
        if !length.arguments.is_empty() {
            self.diagnostics.push(
                Diagnostic::error(WRONG_ARGUMENT_COUNT, "array `len()` takes no arguments")
                    .with_primary_label(length.member_span, "remove these arguments"),
            );
            return None;
        }
        let receiver = self.check_array_receiver(
            &length.receiver,
            match length.operator {
                ResolvedArrayLengthOperator::Ordinary { .. } => ArrayReceiverSyntax::Ordinary,
                ResolvedArrayLengthOperator::Shared { .. } => ArrayReceiverSyntax::Shared,
            },
        )?;
        Some(HirExpression {
            kind: HirExpressionKind::ArrayLength(Box::new(HirArrayLength {
                receiver,
                span: length.span,
            })),
            ty: Type::U64,
            span: length.span,
        })
    }

    pub(in crate::typeck) fn check_array_projection(
        &mut self,
        projection: &ResolvedArrayProjectionExpr,
    ) -> Option<HirExpression> {
        let receiver = self.check_array_receiver(
            &projection.receiver,
            match projection.operator {
                ResolvedArrayProjectionOperator::Ordinary { .. } => ArrayReceiverSyntax::Ordinary,
                ResolvedArrayProjectionOperator::Shared { .. } => ArrayReceiverSyntax::Shared,
            },
        )?;
        let array = receiver.array;
        match &projection.bounds {
            ResolvedArrayProjectionBounds::Index(index) => {
                let index = self.check_array_index(index)?;
                let element = self.copy_capabilities.array(array).element;
                let place = HirArrayElementPlace {
                    receiver,
                    index,
                    element,
                    evaluation: HirArrayEvaluationOrder::ReceiverThenIndex,
                    span: projection.span,
                };
                Some(HirExpression {
                    kind: HirExpressionKind::ArrayElement(Box::new(place)),
                    ty: element,
                    span: projection.span,
                })
            }
            ResolvedArrayProjectionBounds::Slice {
                start,
                colon_span,
                end,
            } => {
                let start = self.check_array_bound(start.as_deref(), "slice start")?;
                let end = self.check_array_bound(end.as_deref(), "slice end")?;
                let Some(element_copy) = self.copy_capabilities.array(array).lifecycle.copy else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            ARRAY_CAPABILITY_UNAVAILABLE,
                            "array element type is not copy-constructible",
                        )
                        .with_primary_label(
                            projection.span,
                            "a slice read creates a copied inline array",
                        ),
                    );
                    return None;
                };
                let slice = HirArraySlice {
                    receiver,
                    bounds: HirArraySliceBounds {
                        start,
                        end,
                        normalization: HirArrayIndexNormalization::SignedFromEndOnce,
                        failure: HirArrayRuntimeFailure::InvalidSliceBoundsTerminate,
                        span: *colon_span,
                    },
                    array,
                    element_copy: Some(element_copy),
                    evaluation: HirArrayEvaluationOrder::ReceiverThenBounds,
                    span: projection.span,
                };
                Some(HirExpression {
                    kind: HirExpressionKind::ArraySlice(Box::new(slice)),
                    ty: Type::Array(array),
                    span: projection.span,
                })
            }
        }
    }

    pub(super) fn check_array_index(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<HirArrayIndex> {
        let value = self.check_expression(expression)?;
        if !require_type(
            value.ty,
            Type::I64,
            value.span,
            "array index",
            self.diagnostics,
        ) {
            return None;
        }
        Some(HirArrayIndex {
            span: value.span,
            value: Box::new(value),
            normalization: HirArrayIndexNormalization::SignedFromEndOnce,
            failure: HirArrayRuntimeFailure::IndexOutOfBoundsTerminate,
        })
    }

    pub(super) fn check_array_bound(
        &mut self,
        expression: Option<&ResolvedExpression>,
        context: &'static str,
    ) -> Option<Option<Box<HirExpression>>> {
        let Some(expression) = expression else {
            return Some(None);
        };
        let bound = self.check_expression(expression)?;
        if !require_type(bound.ty, Type::I64, bound.span, context, self.diagnostics) {
            return None;
        }
        Some(Some(Box::new(bound)))
    }

    pub(super) fn check_array_receiver(
        &mut self,
        expression: &ResolvedExpression,
        syntax: ArrayReceiverSyntax,
    ) -> Option<HirArrayReceiver> {
        match syntax {
            ArrayReceiverSyntax::Shared => {
                let source = self.check_shared_source(expression, false)?;
                self.finish_shared_array_receiver(source, expression.span())
            }
            ArrayReceiverSyntax::Ordinary => {
                if let ResolvedExpression::Dereference(dereference) =
                    expression_through_groups(expression)
                {
                    let source = self.check_shared_source(&dereference.source, false)?;
                    return self.finish_shared_array_receiver(source, expression.span());
                }
                let checked = self.check_expression(expression)?;
                let Type::Array(array) = checked.ty else {
                    if matches!(checked.ty, Type::Shared(HirSharedTarget::Array(_)))
                        || matches!(
                            self.optional_kind(checked.ty),
                            Some(super::super::optional_types::OptionalPayloadKind::Shared(
                                HirSharedTarget::Array(_)
                            ))
                        )
                    {
                        self.diagnostics.push(
                            Diagnostic::error(
                                IMPLICIT_SHARED_DEREFERENCE,
                                "shared arrays require explicit pointee projection",
                            )
                            .with_primary_label(
                                expression.span(),
                                "use `owner->[...]`, `owner->len()`, or `(*owner)[...]`",
                            ),
                        );
                    } else {
                        self.report_non_array_receiver(checked.ty, expression.span());
                    }
                    return None;
                };
                let access = self.array_expression_access(&checked);
                let anchor = if matches!(
                    checked.kind,
                    HirExpressionKind::Binding(_) | HirExpressionKind::FieldRead(_)
                ) {
                    HirArrayAnchor::InlineOwner
                } else {
                    HirArrayAnchor::InlineBacking
                };
                Some(HirArrayReceiver {
                    source: HirArrayReceiverSource::Inline(Box::new(checked)),
                    array,
                    access,
                    ownership: HirArrayReceiverOwnership::Inline,
                    anchor,
                    span: expression.span(),
                })
            }
        }
    }

    fn finish_shared_array_receiver(
        &mut self,
        source: HirSharedSource,
        span: crate::source::Span,
    ) -> Option<HirArrayReceiver> {
        let HirSharedTarget::Array(array) = source.target() else {
            self.report_non_array_receiver(Type::Shared(source.target()), span);
            return None;
        };
        let access = match &source {
            HirSharedSource::Place(HirSharedPlace::Binding { binding, .. }) => {
                self.binding_access(*binding, false, span)?
            }
            HirSharedSource::Place(HirSharedPlace::Field { place, .. }) => place.receiver.access(),
            HirSharedSource::Place(HirSharedPlace::ArrayElement { .. }) => {
                // Array access is shallow across an owning shared edge: the
                // element handle may be read-only while its separate pointee
                // remains mutable after explicit dereference.
                HirAccess::Mutable
            }
            HirSharedSource::Place(HirSharedPlace::Static { .. }) => HirAccess::Mutable,
            HirSharedSource::Produced(_) => HirAccess::Mutable,
        };
        let anchor = match &source {
            HirSharedSource::Place(HirSharedPlace::Binding { .. }) => {
                HirArrayAnchor::StableSharedOwner
            }
            HirSharedSource::Place(HirSharedPlace::Field { .. }) => {
                HirArrayAnchor::CopiedSharedOwner
            }
            HirSharedSource::Place(HirSharedPlace::ArrayElement { .. }) => {
                HirArrayAnchor::CopiedSharedOwner
            }
            HirSharedSource::Place(HirSharedPlace::Static { .. }) => {
                HirArrayAnchor::CopiedSharedOwner
            }
            HirSharedSource::Produced(HirSharedProducer::OptionalUnwrap { .. }) => {
                HirArrayAnchor::SecuredOptionalSharedOwner
            }
            HirSharedSource::Produced(_) => HirArrayAnchor::AdoptedSharedOwner,
        };
        Some(HirArrayReceiver {
            source: HirArrayReceiverSource::Shared(Box::new(source)),
            array,
            access,
            ownership: HirArrayReceiverOwnership::ExplicitSharedPointee,
            anchor,
            span,
        })
    }

    fn array_expression_access(&mut self, expression: &HirExpression) -> HirAccess {
        match &expression.kind {
            HirExpressionKind::Binding(binding) => self
                .binding_access(*binding, false, expression.span)
                .unwrap_or(HirAccess::ReadOnly),
            HirExpressionKind::FieldRead(place) => place.receiver.access(),
            HirExpressionKind::StaticRead(_) => HirAccess::Mutable,
            HirExpressionKind::ArrayElement(place) => place.receiver.access,
            HirExpressionKind::Grouped(inner) => self.array_expression_access(inner),
            _ => HirAccess::Mutable,
        }
    }

    fn report_non_array_receiver(&mut self, actual: Type, span: crate::source::Span) {
        self.diagnostics.push(
            Diagnostic::error(
                ARRAY_PROJECTION_REQUIRES_ARRAY,
                format!(
                    "array operation requires an array, found `{}`",
                    actual.name()
                ),
            )
            .with_primary_label(span, "this expression is not an array owner"),
        );
    }

    pub(super) fn array_source_from_slice_or_value(
        &mut self,
        expression: &ResolvedExpression,
        array: crate::identity::ArrayTypeId,
    ) -> Option<HirArraySource> {
        self.check_array_source(expression, array)
    }
}

fn expression_through_groups(mut expression: &ResolvedExpression) -> &ResolvedExpression {
    while let ResolvedExpression::Grouped(grouped) = expression {
        expression = &grouped.expression;
    }
    expression
}

#[derive(Clone, Copy)]
pub(super) enum ArrayReceiverSyntax {
    Ordinary,
    Shared,
}
