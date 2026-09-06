//! Parameter-dependency provenance for expressions in generic template bodies.

use super::*;

impl TemplateBodyResolver<'_, '_, '_> {
    pub(super) fn range_endpoint_provenance(
        &self,
        expression: &syntax::Expression,
    ) -> ResolvedRangeEndpointProvenance {
        if self.expression_depends_on_parameter(expression) {
            ResolvedRangeEndpointProvenance::SpecializationDependent
        } else {
            ResolvedRangeEndpointProvenance::SpecializationIndependent
        }
    }

    pub(super) fn expression_depends_on_parameter(&self, expression: &syntax::Expression) -> bool {
        match expression {
            syntax::Expression::Absent(_)
            | syntax::Expression::NumericLiteral(_)
            | syntax::Expression::ByteLiteral(_)
            | syntax::Expression::StringLiteral(_)
            | syntax::Expression::Boolean(_) => false,
            syntax::Expression::SelfValue(_) => true,
            syntax::Expression::Identifier(identifier) => {
                !identifier.name.is_qualified()
                    && self
                        .lookup_binding(identifier.name.text.as_str())
                        .is_some_and(|binding| binding.depends_on_parameter)
            }
            syntax::Expression::Present(present) => {
                self.expression_depends_on_parameter(&present.value)
            }
            syntax::Expression::GenericTypeApplication(application) => {
                self.type_use_depends_on_parameter(application.target.span)
            }
            syntax::Expression::GenericStaticSelection(selection) => {
                self.type_use_depends_on_parameter(selection.target.span)
            }
            syntax::Expression::Unary(unary) => {
                self.expression_depends_on_parameter(&unary.operand)
            }
            syntax::Expression::Binary(binary) => {
                self.expression_depends_on_parameter(&binary.left)
                    || self.expression_depends_on_parameter(&binary.right)
            }
            syntax::Expression::Logical(logical) => {
                self.expression_depends_on_parameter(&logical.left)
                    || self.expression_depends_on_parameter(&logical.right)
            }
            syntax::Expression::TypeTest(test) => {
                self.expression_depends_on_parameter(&test.source)
                    || self.type_use_depends_on_parameter(test.target.span)
            }
            syntax::Expression::PresenceTest(test) => {
                self.expression_depends_on_parameter(&test.source)
            }
            syntax::Expression::Unwrap(unwrap) => {
                self.expression_depends_on_parameter(&unwrap.source)
            }
            syntax::Expression::PrimitiveCast(cast) => {
                self.expression_depends_on_parameter(&cast.source)
            }
            syntax::Expression::ObjectCast(cast) => {
                self.expression_depends_on_parameter(&cast.source)
                    || self.type_use_depends_on_parameter(cast.target.span)
            }
            syntax::Expression::Allocation(allocation) => {
                self.type_use_depends_on_parameter(allocation.target.span)
                    || self.call_arguments_depend_on_parameter(&allocation.arguments)
            }
            syntax::Expression::OptionalBoxAllocation(allocation) => {
                self.type_use_depends_on_parameter(allocation.target.span)
                    || match &allocation.initializer {
                        syntax::OptionalBoxInitializer::Absent { .. } => false,
                        syntax::OptionalBoxInitializer::Value { value, .. } => {
                            self.expression_depends_on_parameter(value)
                        }
                    }
            }
            syntax::Expression::ArrayConstruction(array) => {
                self.type_use_depends_on_parameter(array.array_type.span)
                    || match &array.arguments {
                        syntax::ArrayConstructionArguments::Empty { .. } => false,
                        syntax::ArrayConstructionArguments::Length { length, .. } => {
                            self.expression_depends_on_parameter(length)
                        }
                        syntax::ArrayConstructionArguments::Copy { source, .. } => {
                            self.expression_depends_on_parameter(source)
                        }
                        syntax::ArrayConstructionArguments::Indexed(initializer) => {
                            self.expression_depends_on_parameter(&initializer.length)
                                || self.expression_depends_on_parameter(&initializer.element)
                        }
                        syntax::ArrayConstructionArguments::Elements(elements) => elements
                            .elements
                            .iter()
                            .any(|element| self.expression_depends_on_parameter(element)),
                    }
            }
            syntax::Expression::Call(call) => {
                self.expression_depends_on_parameter(&call.callee)
                    || self.call_arguments_depend_on_parameter(&call.arguments)
            }
            syntax::Expression::Grouped(grouped) => {
                self.expression_depends_on_parameter(&grouped.expression)
            }
            syntax::Expression::MemberAccess(access) => {
                self.expression_depends_on_parameter(&access.receiver)
            }
            syntax::Expression::BracketProjection(projection) => {
                self.expression_depends_on_parameter(&projection.receiver)
                    || match &projection.bounds {
                        syntax::BracketProjectionBounds::Index(index) => {
                            self.expression_depends_on_parameter(index)
                        }
                        syntax::BracketProjectionBounds::Slice { start, end, .. } => start
                            .iter()
                            .chain(end.iter())
                            .any(|bound| self.expression_depends_on_parameter(bound)),
                    }
            }
        }
    }

    fn call_arguments_depend_on_parameter(&self, arguments: &syntax::CallArguments) -> bool {
        match arguments {
            syntax::CallArguments::Ordinary(arguments) => arguments
                .iter()
                .any(|argument| self.expression_depends_on_parameter(argument)),
            syntax::CallArguments::Copy { source, .. } => {
                self.expression_depends_on_parameter(source)
            }
        }
    }

    fn type_use_depends_on_parameter(&self, span: Span) -> bool {
        self.type_uses.iter().any(|type_use| {
            type_use.type_term.span == span && type_use.type_term.depends_on_parameter()
        })
    }
}
