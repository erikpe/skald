//! Definition-site selection for canonical operators over bounded parameters.

use super::*;

impl TemplateBodyResolver<'_, '_, '_> {
    pub(super) fn select_unary_operator(&mut self, expression: &syntax::UnaryExpr) {
        let operator = match expression.operator {
            syntax::UnaryOperator::Negate => ResolvedUnaryOperator::Negate,
            syntax::UnaryOperator::BitwiseComplement => ResolvedUnaryOperator::BitwiseComplement,
            syntax::UnaryOperator::LogicalNot | syntax::UnaryOperator::Dereference => return,
        };
        let Some(parameter) = self.parameter_of_expression(&expression.operand) else {
            return;
        };
        self.select_operator(
            parameter,
            operator
                .protocol()
                .expect("selected unary operator is overloadable"),
            None,
            ResolvedTemplateOperatorSyntax::Unary {
                operator,
                operator_span: expression.operator_span,
                operand_span: expression.operand.span(),
            },
            expression.span,
        );
    }

    pub(super) fn select_binary_operator(&mut self, expression: &syntax::BinaryExpr) {
        let Some(parameter) = self.parameter_of_expression(&expression.left) else {
            return;
        };
        let operator = resolved_binary_operator(expression.operator);
        let right = self.type_of_expression(&expression.right);
        self.select_operator(
            parameter,
            operator.protocol(),
            right.as_ref(),
            ResolvedTemplateOperatorSyntax::Binary {
                operator,
                operator_span: expression.operator_span,
                left_span: expression.left.span(),
                right_span: expression.right.span(),
            },
            expression.span,
        );
    }

    fn select_operator(
        &mut self,
        parameter: TypeParameterId,
        protocol: CanonicalOperatorProtocol,
        right: Option<&ResolvedTemplateType>,
        syntax: ResolvedTemplateOperatorSyntax,
        span: Span,
    ) {
        let Some(language_item) = self.operator_language_item else {
            self.report_unsupported_operator(parameter, syntax);
            return;
        };
        let canonical = language_item.get(protocol);
        let mut candidates = self
            .bounds
            .iter()
            .enumerate()
            .filter_map(|(bound, candidate)| {
                if candidate.parameter != parameter {
                    return None;
                }
                let ResolvedInterfaceType::TemplateApplication {
                    template,
                    arguments,
                } = &candidate.interface
                else {
                    return None;
                };
                if *template != canonical.template {
                    return None;
                }
                let (rhs, output) = structural_arguments(canonical.kind, arguments, span)?;
                if let Some(expected) = rhs.as_ref() {
                    let actual = right?;
                    if !readonly_alias_compatible(actual, expected) {
                        return None;
                    }
                }
                Some(ResolvedTemplateOperatorSelection {
                    syntax,
                    parameter,
                    bound,
                    protocol,
                    requirement: canonical.requirement,
                    rhs,
                    output,
                    origin_span: candidate.interface_span,
                    span,
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|candidate| candidate.bound);
        match candidates.as_slice() {
            [selection] => self
                .selections
                .push(ResolvedTemplateSelection::Operator(selection.clone())),
            [] => self.report_unsupported_operator(parameter, syntax),
            candidates => self.report_ambiguous_operator(syntax, candidates),
        }
    }

    pub(super) fn operator_output(&self, span: Span) -> Option<ResolvedTemplateType> {
        self.selections
            .iter()
            .rev()
            .find_map(|selection| match selection {
                ResolvedTemplateSelection::Operator(selection) if selection.span == span => {
                    Some(selection.output.clone())
                }
                _ => None,
            })
    }

    fn report_unsupported_operator(
        &mut self,
        parameter: TypeParameterId,
        syntax: ResolvedTemplateOperatorSyntax,
    ) {
        let declaration = self.parameter(parameter);
        self.diagnostics.push(
            Diagnostic::error(
                super::super::super::super::UNSUPPORTED_GENERIC_OPERATOR_APPLICATION,
                format!(
                    "operator `{}` is not authorized for type parameter `{}`",
                    operator_spelling(syntax),
                    declaration.name,
                ),
            )
            .with_primary_label(
                operator_span(syntax),
                "no exact declared operator bound applies",
            )
            .with_secondary_label(declaration.name_span, "type parameter declared here"),
        );
    }

    fn report_ambiguous_operator(
        &mut self,
        syntax: ResolvedTemplateOperatorSyntax,
        candidates: &[ResolvedTemplateOperatorSelection],
    ) {
        let mut diagnostic = Diagnostic::error(
            super::super::super::super::AMBIGUOUS_GENERIC_OPERATOR_APPLICATION,
            format!(
                "operator `{}` has multiple applicable bounds",
                operator_spelling(syntax)
            ),
        )
        .with_primary_label(
            operator_span(syntax),
            "generic operator selection is ambiguous",
        );
        for candidate in candidates {
            diagnostic = diagnostic.with_secondary_label(
                candidate.origin_span,
                "candidate operator bound declared here",
            );
        }
        self.diagnostics.push(diagnostic);
    }

    fn parameter(&self, parameter: TypeParameterId) -> &ResolvedTypeParameter {
        self.parameters
            .iter()
            .find(|candidate| candidate.id == parameter)
            .expect("operator receiver parameter belongs to its template")
    }
}

fn structural_arguments(
    protocol: CanonicalOperatorProtocol,
    arguments: &[ResolvedTemplateType],
    span: Span,
) -> Option<(Option<ResolvedTemplateType>, ResolvedTemplateType)> {
    match protocol.shape() {
        CanonicalOperatorProtocolShape::Unary => {
            let [output] = arguments else { return None };
            Some((None, output.clone()))
        }
        CanonicalOperatorProtocolShape::Binary => {
            let [rhs, output] = arguments else {
                return None;
            };
            Some((Some(rhs.clone()), output.clone()))
        }
        CanonicalOperatorProtocolShape::Predicate => {
            let [rhs] = arguments else { return None };
            Some((
                Some(rhs.clone()),
                ResolvedTemplateType {
                    kind: ResolvedTemplateTypeKind::Bool,
                    span,
                },
            ))
        }
    }
}

fn readonly_alias_compatible(
    actual: &ResolvedTemplateType,
    expected: &ResolvedTemplateType,
) -> bool {
    actual.semantically_eq(expected)
        || matches!(
            (&actual.kind, &expected.kind),
            (
                ResolvedTemplateTypeKind::Class(_)
                    | ResolvedTemplateTypeKind::ClassTemplate { .. }
                    | ResolvedTemplateTypeKind::Interface(_)
                    | ResolvedTemplateTypeKind::InterfaceTemplate { .. },
                ResolvedTemplateTypeKind::Obj
            )
        )
}

fn resolved_binary_operator(operator: syntax::BinaryOperator) -> ResolvedBinaryOperator {
    match operator {
        syntax::BinaryOperator::Add => ResolvedBinaryOperator::Add,
        syntax::BinaryOperator::Subtract => ResolvedBinaryOperator::Subtract,
        syntax::BinaryOperator::Multiply => ResolvedBinaryOperator::Multiply,
        syntax::BinaryOperator::Divide => ResolvedBinaryOperator::Divide,
        syntax::BinaryOperator::Remainder => ResolvedBinaryOperator::Remainder,
        syntax::BinaryOperator::ShiftLeft => ResolvedBinaryOperator::ShiftLeft,
        syntax::BinaryOperator::ShiftRight => ResolvedBinaryOperator::ShiftRight,
        syntax::BinaryOperator::BitwiseAnd => ResolvedBinaryOperator::BitwiseAnd,
        syntax::BinaryOperator::BitwiseOr => ResolvedBinaryOperator::BitwiseOr,
        syntax::BinaryOperator::BitwiseXor => ResolvedBinaryOperator::BitwiseXor,
        syntax::BinaryOperator::Equal => ResolvedBinaryOperator::Equal,
        syntax::BinaryOperator::NotEqual => ResolvedBinaryOperator::NotEqual,
        syntax::BinaryOperator::LessThan => ResolvedBinaryOperator::LessThan,
        syntax::BinaryOperator::LessEqual => ResolvedBinaryOperator::LessEqual,
        syntax::BinaryOperator::GreaterThan => ResolvedBinaryOperator::GreaterThan,
        syntax::BinaryOperator::GreaterEqual => ResolvedBinaryOperator::GreaterEqual,
    }
}

const fn operator_span(syntax: ResolvedTemplateOperatorSyntax) -> Span {
    match syntax {
        ResolvedTemplateOperatorSyntax::Unary { operator_span, .. }
        | ResolvedTemplateOperatorSyntax::Binary { operator_span, .. } => operator_span,
    }
}

const fn operator_spelling(syntax: ResolvedTemplateOperatorSyntax) -> &'static str {
    match syntax {
        ResolvedTemplateOperatorSyntax::Unary { operator, .. } => operator.spelling(),
        ResolvedTemplateOperatorSyntax::Binary { operator, .. } => operator.spelling(),
    }
}
