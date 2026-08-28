//! Erasure of resolved operator selections to ordinary interface calls.

use super::*;
use crate::{
    identity::ClassId,
    resolve::{
        CanonicalOperatorProtocol, CanonicalOperatorProtocolShape, ResolvedBinaryExpr,
        ResolvedExpression, ResolvedInterfaceCallExpr, ResolvedInterfaceReceiver,
        ResolvedObjectReceiver, ResolvedOperatorResolution, ResolvedOperatorSelection,
        ResolvedUnaryExpr,
    },
    typeck::program::lower_type_kind,
};

/// Whether this source expression is known to erase to one ordinary
/// interface call. Result-capability owners use this instead of duplicating
/// operator protocol selection rules.
pub(in crate::typeck) fn is_selected_operator_expression(expression: &ResolvedExpression) -> bool {
    match expression {
        ResolvedExpression::Unary(unary) => unary
            .selection
            .as_ref()
            .and_then(ResolvedOperatorResolution::selected)
            .is_some(),
        ResolvedExpression::Binary(binary) => binary
            .selection
            .as_ref()
            .and_then(ResolvedOperatorResolution::selected)
            .is_some(),
        ResolvedExpression::Grouped(grouped) => {
            is_selected_operator_expression(&grouped.expression)
        }
        _ => false,
    }
}

impl CallableChecker<'_, '_> {
    pub(super) fn check_selected_unary_operator(
        &mut self,
        unary: &ResolvedUnaryExpr,
        resolution: &ResolvedOperatorResolution,
    ) -> Option<HirExpression> {
        let selection = self.require_operator_selection(
            resolution,
            unary
                .operator
                .protocol()
                .expect("selected unary operator is overloadable"),
            unary.operator.spelling(),
            unary.operator_span,
            &unary.operand,
            None,
        )?;
        let receiver = self.operator_receiver(&unary.operand, selection)?;
        self.check_interface_call(&ResolvedInterfaceCallExpr {
            receiver,
            interface: selection.interface,
            requirement: selection.requirement,
            receiver_span: unary.operand.span(),
            member_span: unary.operator_span,
            arguments: Vec::new(),
            span: unary.span,
        })
    }

    pub(super) fn check_selected_binary_operator(
        &mut self,
        binary: &ResolvedBinaryExpr,
        resolution: &ResolvedOperatorResolution,
    ) -> Option<HirExpression> {
        let selection = self.require_operator_selection(
            resolution,
            binary.operator.protocol(),
            binary.operator.spelling(),
            binary.operator_span,
            &binary.left,
            Some(&binary.right),
        )?;
        let receiver = self.operator_receiver(&binary.left, selection)?;
        let call = self.check_interface_call(&ResolvedInterfaceCallExpr {
            receiver,
            interface: selection.interface,
            requirement: selection.requirement,
            receiver_span: binary.left.span(),
            member_span: binary.operator_span,
            arguments: vec![(*binary.right).clone()],
            span: binary.span,
        })?;
        if binary.operator != crate::resolve::ResolvedBinaryOperator::NotEqual {
            return Some(call);
        }
        debug_assert_eq!(call.ty, Type::Bool);
        Some(HirExpression {
            kind: HirExpressionKind::Unary {
                operation: crate::hir::HirUnaryOperation::LogicalNotBool,
                operand: Box::new(call),
            },
            ty: Type::Bool,
            span: binary.span,
        })
    }

    fn require_operator_selection(
        &mut self,
        resolution: &ResolvedOperatorResolution,
        expected_protocol: CanonicalOperatorProtocol,
        spelling: &'static str,
        operator_span: crate::source::Span,
        left: &ResolvedExpression,
        right: Option<&ResolvedExpression>,
    ) -> Option<ResolvedOperatorSelection> {
        if let Some(selection) = resolution.selected() {
            if self.operator_selection_is_valid(resolution, selection, expected_protocol) {
                return Some(selection);
            }
            self.diagnostics.push(
                Diagnostic::error(
                    crate::typeck::program::INVALID_OPERATOR_SELECTION,
                    "resolved operator selection has inconsistent canonical mapping evidence",
                )
                .with_primary_label(
                    operator_span,
                    "invalid operator selection reaches type checking",
                )
                .with_secondary_label(
                    selection.origin_span,
                    "inconsistent application selected here",
                ),
            );
            return None;
        }
        let left_type = self.static_expression_type(left);
        let mut diagnostic = if !resolution.incompatible_rhs.is_empty() {
            Diagnostic::error(
                crate::typeck::program::INCOMPATIBLE_OPERATOR_RHS,
                format!("operator `{spelling}` cannot bind its right operand to any canonical application"),
            )
            .with_primary_label(operator_span, "right operand is incompatible with every declared `Rhs`")
        } else if resolution.candidates.is_empty() {
            Diagnostic::error(
                crate::typeck::program::UNSUPPORTED_OPERATOR_APPLICATION,
                format!("operator `{spelling}` is unsupported for these operands"),
            )
            .with_primary_label(
                operator_span,
                "no canonical protocol application was selected",
            )
        } else {
            Diagnostic::error(
                crate::typeck::program::AMBIGUOUS_OPERATOR_APPLICATION,
                format!("operator `{spelling}` has multiple applicable protocol applications"),
            )
            .with_primary_label(operator_span, "operator selection is ambiguous")
        }
        .with_secondary_label(
            left.span(),
            format!(
                "left operand has static type `{}`",
                self.operator_type_name(left_type)
            ),
        );
        if let Some(right) = right {
            diagnostic = diagnostic.with_secondary_label(
                right.span(),
                format!(
                    "right operand has static type `{}`",
                    self.operator_type_name(self.static_expression_type(right))
                ),
            );
        }
        for candidate in &resolution.candidates {
            diagnostic = diagnostic.with_secondary_label(
                candidate.origin_span,
                format!(
                    "candidate `{}` application declared here",
                    candidate.protocol.interface_name()
                ),
            );
        }
        for candidate in &resolution.incompatible_rhs {
            let expected = candidate
                .rhs
                .map(lower_type_kind)
                .map(|ty| self.operator_type_name(ty))
                .unwrap_or_else(|| "<missing>".to_owned());
            diagnostic = diagnostic.with_secondary_label(
                candidate.origin_span,
                format!("candidate requires read-only `Rhs` `{expected}`"),
            );
        }
        if self.program.operator_language_item.is_none() {
            diagnostic = diagnostic.with_note(
                "the canonical `std::ops` bundle is not reachable through an explicit protocol reference",
            );
        }
        self.diagnostics.push(diagnostic);
        None
    }

    fn operator_selection_is_valid(
        &self,
        resolution: &ResolvedOperatorResolution,
        selection: ResolvedOperatorSelection,
        expected_protocol: CanonicalOperatorProtocol,
    ) -> bool {
        if resolution.protocol != expected_protocol || selection.protocol != expected_protocol {
            return false;
        }
        let Some(canonical) = self
            .program
            .operator_language_item
            .as_ref()
            .map(|language_item| language_item.get(expected_protocol))
        else {
            return false;
        };
        let Some(application) = self
            .program
            .generic_interface_specializations
            .for_interface(selection.interface)
        else {
            return false;
        };
        if application.key.template != canonical.template
            || !application.requirement_mappings.iter().any(|mapping| {
                mapping.template == canonical.requirement && mapping.closed == selection.requirement
            })
        {
            return false;
        }
        let Some(interface) = self.program.interface(selection.interface) else {
            return false;
        };
        let Some(requirement) = interface
            .requirements
            .get(selection.requirement.index())
            .filter(|requirement| requirement.id == selection.requirement)
        else {
            return false;
        };
        if requirement.return_type.kind != selection.output {
            return false;
        }
        match expected_protocol.shape() {
            CanonicalOperatorProtocolShape::Unary => {
                selection.rhs.is_none() && requirement.parameters.is_empty()
            }
            CanonicalOperatorProtocolShape::Predicate | CanonicalOperatorProtocolShape::Binary => {
                let [parameter] = requirement.parameters.as_slice() else {
                    return false;
                };
                selection.rhs == Some(parameter.type_syntax.kind)
            }
        }
    }

    fn operator_type_name(&self, ty: Type) -> String {
        match ty {
            Type::Class(class) => self
                .program
                .class(class)
                .map(|class| class.name.clone())
                .unwrap_or_else(|| ty.name().into_owned()),
            Type::Interface(interface) => self
                .program
                .interface(interface)
                .map(|interface| interface.name.clone())
                .unwrap_or_else(|| ty.name().into_owned()),
            _ => self.diagnostic_type_name(ty),
        }
    }

    fn operator_receiver(
        &mut self,
        expression: &ResolvedExpression,
        selection: ResolvedOperatorSelection,
    ) -> Option<ResolvedInterfaceReceiver> {
        match self.static_expression_type(expression) {
            Type::Class(class) => self
                .class_operator_receiver(expression.clone(), class)
                .map(|receiver| ResolvedInterfaceReceiver::Object(Box::new(receiver))),
            Type::Interface(interface) if interface == selection.interface => {
                self.interface_operator_receiver(expression.clone(), interface)
            }
            actual => {
                self.diagnostics.push(
                    Diagnostic::error(
                        crate::typeck::program::TYPE_MISMATCH,
                        "selected operator requires an exact class or canonical interface receiver",
                    )
                    .with_primary_label(
                        expression.span(),
                        format!(
                            "left operand has type `{}`",
                            self.diagnostic_type_name(actual)
                        ),
                    )
                    .with_secondary_label(
                        selection.origin_span,
                        "protocol application selected here",
                    ),
                );
                None
            }
        }
    }

    fn class_operator_receiver(
        &mut self,
        expression: ResolvedExpression,
        class: ClassId,
    ) -> Option<ResolvedObjectReceiver> {
        match ResolvedObjectReceiver::from_expression(expression, class) {
            Ok(receiver) => Some(receiver),
            Err(unsupported) => {
                self.report_operator_receiver_form(unsupported.span());
                None
            }
        }
    }

    fn interface_operator_receiver(
        &mut self,
        expression: ResolvedExpression,
        interface: crate::identity::InterfaceId,
    ) -> Option<ResolvedInterfaceReceiver> {
        match ResolvedInterfaceReceiver::from_expression(expression, interface) {
            Ok((receiver, _)) => Some(receiver),
            Err(unsupported) => {
                self.report_operator_receiver_form(unsupported.span());
                None
            }
        }
    }

    fn report_operator_receiver_form(&mut self, span: crate::source::Span) {
        self.diagnostics.push(
            Diagnostic::error(
                crate::typeck::program::INVALID_OBJECT_CONTEXT,
                "this object expression cannot be used as an overloaded operator receiver",
            )
            .with_primary_label(span, "unsupported receiver form"),
        );
    }
}
