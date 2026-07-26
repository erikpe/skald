//! Typed array construction, source provenance, and owning initialization.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirArrayConstruction, HirArrayConstructionMode, HirArrayInitialize, HirArrayOwnership,
        HirArrayProvenance, HirArraySource, HirArrayTransfer, HirExpression, HirExpressionKind,
        HirSharedTarget, Type,
    },
    resolve::{
        ResolvedArrayConstructionArguments, ResolvedArrayConstructionExpr, ResolvedExpression,
        ResolvedTypeKind,
    },
};

use super::super::{expression::require_type, function::CallableChecker};

pub const ARRAY_CAPABILITY_UNAVAILABLE: &str = "TYP037";
pub const ARRAY_LENGTH_OUT_OF_RANGE: &str = "TYP038";

impl CallableChecker<'_, '_> {
    pub(crate) fn check_array_construction(
        &mut self,
        construction: &ResolvedArrayConstructionExpr,
    ) -> Option<HirExpression> {
        let ResolvedTypeKind::Array(array) = construction.array_type.kind else {
            unreachable!("resolved array construction must retain an array identity")
        };
        let lifecycle = self.copy_capabilities.array(array).lifecycle.clone();
        let mode = match &construction.arguments {
            ResolvedArrayConstructionArguments::Empty { .. } => HirArrayConstructionMode::Empty,
            ResolvedArrayConstructionArguments::Length { length, .. } => {
                let length = self.check_expression(length)?;
                if !require_type(
                    length.ty,
                    Type::U64,
                    length.span,
                    "array length",
                    self.diagnostics,
                ) {
                    return None;
                }
                if matches!(
                    length.kind,
                    HirExpressionKind::U64(value) if value > i64::MAX as u64
                ) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            ARRAY_LENGTH_OUT_OF_RANGE,
                            "array length exceeds the supported maximum",
                        )
                        .with_primary_label(
                            length.span,
                            format!("length must not exceed {}", i64::MAX),
                        ),
                    );
                    return None;
                }
                let Some(element) = lifecycle.default else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            ARRAY_CAPABILITY_UNAVAILABLE,
                            "array element type is not default-constructible",
                        )
                        .with_primary_label(
                            construction.array_type.span,
                            "default-length construction requires an element default plan",
                        ),
                    );
                    return None;
                };
                HirArrayConstructionMode::DefaultLength {
                    length: Box::new(length),
                    element,
                }
            }
            ResolvedArrayConstructionArguments::Copy {
                copy_span, source, ..
            } => {
                let source = self.check_array_copy_source(source, array)?;
                let Some(element) = lifecycle.copy else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            ARRAY_CAPABILITY_UNAVAILABLE,
                            "array element type is not copy-constructible",
                        )
                        .with_primary_label(
                            *copy_span,
                            "explicit array copying requires an element copy plan",
                        ),
                    );
                    return None;
                };
                HirArrayConstructionMode::Copy { source, element }
            }
        };
        let ownership = if construction.new_span.is_some() {
            HirArrayOwnership::Shared
        } else {
            HirArrayOwnership::Inline
        };
        Some(HirExpression {
            kind: HirExpressionKind::ArrayConstruction(Box::new(HirArrayConstruction {
                array,
                ownership,
                mode,
                span: construction.span,
            })),
            ty: match ownership {
                HirArrayOwnership::Inline => Type::Array(array),
                HirArrayOwnership::Shared => Type::Shared(HirSharedTarget::Array(array)),
            },
            span: construction.span,
        })
    }

    pub(crate) fn check_array_initialize(
        &mut self,
        array: crate::identity::ArrayTypeId,
        expression: &ResolvedExpression,
        context: &'static str,
    ) -> Option<HirArrayInitialize> {
        let source = self.check_array_source(expression, array)?;
        let operation = match source.provenance {
            HirArrayProvenance::Produced => HirArrayTransfer::Adopt,
            HirArrayProvenance::Named => {
                let Some(element) = self.copy_capabilities.array(array).lifecycle.copy else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            ARRAY_CAPABILITY_UNAVAILABLE,
                            "array element type is not copy-constructible",
                        )
                        .with_primary_label(
                            expression.span(),
                            format!("{context} requires a deep array copy"),
                        ),
                    );
                    return None;
                };
                HirArrayTransfer::DeepCopy(element)
            }
        };
        Some(HirArrayInitialize {
            source,
            operation,
            span: expression.span(),
        })
    }

    pub(super) fn check_array_source(
        &mut self,
        expression: &ResolvedExpression,
        expected: crate::identity::ArrayTypeId,
    ) -> Option<HirArraySource> {
        let checked = self.check_expression(expression)?;
        if !require_type(
            checked.ty,
            Type::Array(expected),
            checked.span,
            "array source",
            self.diagnostics,
        ) {
            return None;
        }
        let provenance = array_provenance(&checked);
        let receiver = crate::hir::HirArrayReceiver {
            source: crate::hir::HirArrayReceiverSource::Inline(Box::new(checked)),
            array: expected,
            access: crate::hir::HirAccess::ReadOnly,
            ownership: crate::hir::HirArrayReceiverOwnership::Inline,
            anchor: match provenance {
                HirArrayProvenance::Named => crate::hir::HirArrayAnchor::InlineOwner,
                HirArrayProvenance::Produced => crate::hir::HirArrayAnchor::InlineBacking,
            },
            span: expression.span(),
        };
        Some(HirArraySource {
            span: expression.span(),
            receiver,
            provenance,
            array: expected,
        })
    }

    fn check_array_copy_source(
        &mut self,
        expression: &ResolvedExpression,
        expected: crate::identity::ArrayTypeId,
    ) -> Option<HirArraySource> {
        let receiver =
            self.check_array_receiver(expression, super::place::ArrayReceiverSyntax::Ordinary)?;
        if !require_type(
            Type::Array(receiver.array),
            Type::Array(expected),
            expression.span(),
            "array source",
            self.diagnostics,
        ) {
            return None;
        }
        let provenance = match &receiver.source {
            crate::hir::HirArrayReceiverSource::Inline(expression) => array_provenance(expression),
            crate::hir::HirArrayReceiverSource::Shared(_) => HirArrayProvenance::Named,
        };
        Some(HirArraySource {
            span: expression.span(),
            receiver,
            provenance,
            array: expected,
        })
    }
}

fn array_provenance(expression: &HirExpression) -> HirArrayProvenance {
    match &expression.kind {
        HirExpressionKind::Binding(_) | HirExpressionKind::FieldRead(_) => {
            HirArrayProvenance::Named
        }
        HirExpressionKind::Grouped(inner) => array_provenance(inner),
        HirExpressionKind::ArrayConstruction(_)
        | HirExpressionKind::ArraySlice(_)
        | HirExpressionKind::DirectCall { .. }
        | HirExpressionKind::MethodCall { .. }
        | HirExpressionKind::InterfaceCall { .. } => HirArrayProvenance::Produced,
        _ => HirArrayProvenance::Named,
    }
}
