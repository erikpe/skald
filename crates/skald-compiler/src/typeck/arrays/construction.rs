//! Typed array construction, source provenance, and owning initialization.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirArrayConstruction, HirArrayConstructionMode, HirArrayElementInitialization,
        HirArrayElementList, HirArrayInitialize, HirArrayOwnership, HirArrayProvenance,
        HirArraySource, HirArrayTransfer, HirExpression, HirExpressionKind,
        HirIndexedArrayInitialization, HirSharedTarget, Type,
    },
    resolve::{
        ResolvedArrayConstructionArguments, ResolvedArrayConstructionExpr, ResolvedExpression,
        ResolvedTypeKind,
    },
};

use super::super::{
    expression::require_type,
    function::{lower_local, CallableChecker},
};

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
                let length = self.check_array_construction_length(length)?;
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
                let initializer = match &element {
                    crate::hir::HirArrayDefaultElement::Class { initializer, .. }
                    | crate::hir::HirArrayDefaultElement::SharedClass { initializer, .. } => {
                        Some(*initializer)
                    }
                    _ => None,
                };
                if initializer.is_some_and(|initializer| {
                    !self.check_initializer_access(initializer, construction.array_type.span)
                }) {
                    return None;
                }
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
            ResolvedArrayConstructionArguments::Indexed(initializer) => {
                let length = self.check_array_construction_length(&initializer.length)?;
                debug_assert_eq!(initializer.binding.type_syntax.kind, ResolvedTypeKind::I64);
                let binding = lower_local(self.program, &initializer.binding);
                debug_assert_eq!(binding.ty, Type::I64);

                let inserted = self.read_only_locals.insert(binding.id);
                debug_assert!(inserted, "an indexed binding is active only in its element");
                let value = self.check_stored_value_initialization(
                    self.copy_capabilities.array(array).element,
                    &initializer.element,
                    "indexed array element initializer",
                );
                let removed = self.read_only_locals.remove(&binding.id);
                debug_assert!(removed);
                let value = value?;
                let element = HirArrayElementInitialization {
                    element: self.copy_capabilities.array(array).element,
                    span: initializer.element.span(),
                    value,
                };
                HirArrayConstructionMode::Indexed(Box::new(HirIndexedArrayInitialization {
                    left_paren_span: initializer.left_paren_span,
                    length: Box::new(length),
                    semicolon_span: initializer.semicolon_span,
                    binding,
                    arrow_span: initializer.arrow_span,
                    element,
                    right_paren_span: initializer.right_paren_span,
                }))
            }
            ResolvedArrayConstructionArguments::Elements(list) => {
                let element = self.copy_capabilities.array(array).element;
                let mut elements = Vec::with_capacity(list.elements.len());
                let mut valid = true;
                for source in &list.elements {
                    match self.check_stored_value_initialization(
                        element,
                        source,
                        "array element initializer",
                    ) {
                        Some(value) => elements.push(HirArrayElementInitialization {
                            element,
                            span: source.span(),
                            value,
                        }),
                        None => valid = false,
                    }
                }
                if !valid {
                    return None;
                }
                HirArrayConstructionMode::Elements(HirArrayElementList {
                    left_brace_span: list.left_brace_span,
                    elements,
                    comma_spans: list.comma_spans.clone(),
                    right_brace_span: list.right_brace_span,
                })
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

    fn check_array_construction_length(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<HirExpression> {
        let length = self.check_expression(expression)?;
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
                .with_primary_label(length.span, format!("length must not exceed {}", i64::MAX)),
            );
            return None;
        }
        Some(length)
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
        | HirExpressionKind::OptionalArrayUnwrap(_)
        | HirExpressionKind::DirectCall { .. }
        | HirExpressionKind::IndirectCall(_)
        | HirExpressionKind::StaticCall { .. }
        | HirExpressionKind::MethodCall { .. }
        | HirExpressionKind::InterfaceCall { .. } => HirArrayProvenance::Produced,
        _ => HirArrayProvenance::Named,
    }
}
