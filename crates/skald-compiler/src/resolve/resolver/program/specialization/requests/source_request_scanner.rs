//! Source-order discovery of explicit generic applications in the AST.

use super::syntax_type_closer::SyntaxTypeCloser;

use crate::syntax;

pub(super) struct SourceRequestScanner<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup> {
    resolver: SyntaxTypeCloser<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup>,
}

impl<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup>
    SourceRequestScanner<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup>
{
    pub(super) fn new(
        resolver: SyntaxTypeCloser<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup>,
    ) -> Self {
        Self { resolver }
    }

    pub(super) fn visit_unit(&mut self, unit: &syntax::CompilationUnit) {
        for declaration in &unit.declarations {
            self.visit_declaration(declaration);
        }
    }

    fn visit_declaration(&mut self, declaration: &syntax::TopLevelDeclaration) {
        match declaration {
            syntax::TopLevelDeclaration::Function(function) => {
                self.begin_callable(&function.parameters);
                self.visit_type(&function.return_type);
                self.visit_block(&function.body);
                self.end_callable();
            }
            syntax::TopLevelDeclaration::ExternalFunction(function) => {
                self.visit_parameters(&function.parameters);
                self.visit_type(&function.return_type);
            }
            syntax::TopLevelDeclaration::IntrinsicFunction(function) => {
                self.visit_parameters(&function.parameters);
                self.visit_type(&function.return_type);
            }
            syntax::TopLevelDeclaration::Class(class) if class.type_parameters.is_none() => {
                if let Some(base) = &class.direct_base {
                    self.visit_named_type(base);
                }
                for interface in &class.implemented_interfaces {
                    self.visit_named_type(interface);
                }
                for member in &class.members {
                    self.visit_member(member);
                }
            }
            syntax::TopLevelDeclaration::Interface(interface)
                if interface.type_parameters.is_none() =>
            {
                for requirement in &interface.requirements {
                    self.visit_parameters(&requirement.parameters);
                    self.visit_type(&requirement.return_type);
                }
            }
            syntax::TopLevelDeclaration::Class(_) | syntax::TopLevelDeclaration::Interface(_) => {
                // Applications in an unrequested template are discovered only
                // after substitution closes that template.
            }
        }
    }

    fn visit_member(&mut self, member: &syntax::ClassMember) {
        match member {
            syntax::ClassMember::Field(field) => self.visit_type(&field.type_syntax),
            syntax::ClassMember::StaticField(field) => {
                self.visit_type(&field.type_syntax);
                if let Some(initializer) = &field.initializer {
                    self.visit_expression(&initializer.expression);
                }
            }
            syntax::ClassMember::Initializer(declaration) => {
                self.begin_callable(&declaration.parameters);
                self.visit_block(&declaration.body);
                self.end_callable();
            }
            syntax::ClassMember::CopyConstructor(declaration) => {
                self.begin_callable(&declaration.parameters);
                self.visit_block(&declaration.body);
                self.end_callable();
            }
            syntax::ClassMember::CopyAssignment(declaration) => {
                self.begin_callable(&declaration.parameters);
                self.visit_block(&declaration.body);
                self.end_callable();
            }
            syntax::ClassMember::Destructor(declaration) => {
                self.begin_callable(&[]);
                self.visit_block(&declaration.body);
                self.end_callable();
            }
            syntax::ClassMember::Method(declaration) => {
                self.begin_callable(&declaration.parameters);
                self.visit_type(&declaration.return_type);
                self.visit_block(&declaration.body);
                self.end_callable();
            }
        }
    }

    fn visit_parameters(&mut self, parameters: &[syntax::Parameter]) {
        for parameter in parameters {
            self.visit_type(&parameter.type_syntax);
        }
    }

    fn begin_callable(&mut self, parameters: &[syntax::Parameter]) {
        self.visit_parameters(parameters);
    }

    fn end_callable(&mut self) {}

    fn visit_type(&mut self, syntax: &syntax::TypeSyntax) {
        let _ = self.resolver.close(syntax);
    }

    fn visit_named_type(&mut self, syntax: &syntax::NamedTypeSyntax) {
        let _ = self.resolver.close_named(syntax, false);
    }

    fn visit_block(&mut self, block: &syntax::Block) {
        for statement in &block.statements {
            self.visit_statement(statement);
        }
    }

    fn visit_statement(&mut self, statement: &syntax::Statement) {
        match statement {
            syntax::Statement::BaseInitialization(statement) => {
                self.visit_expressions(&statement.arguments)
            }
            syntax::Statement::Local(statement) => {
                self.visit_expression(&statement.initializer);
                self.visit_type(&statement.type_syntax);
            }
            syntax::Statement::Return(statement) => {
                if let Some(value) = &statement.value {
                    self.visit_expression(value);
                }
            }
            syntax::Statement::Break(_) | syntax::Statement::Continue(_) => {}
            syntax::Statement::Expression(statement) => {
                self.visit_expression(&statement.expression)
            }
            syntax::Statement::Conditional(statement) => {
                self.visit_expression(&statement.if_arm.condition);
                self.visit_block(&statement.if_arm.body);
                for arm in &statement.elif_arms {
                    self.visit_expression(&arm.condition);
                    self.visit_block(&arm.body);
                }
                if let Some(body) = &statement.else_block {
                    self.visit_block(body);
                }
            }
            syntax::Statement::While(statement) => {
                self.visit_expression(&statement.condition);
                self.visit_block(&statement.body);
            }
            syntax::Statement::ForIn(statement) => {
                if let Some(annotation) = &statement.annotation {
                    self.visit_type(&annotation.type_syntax);
                }
                self.visit_expression(&statement.iterable);
                self.visit_block(&statement.body);
            }
            syntax::Statement::Block(block) => self.visit_block(block),
            syntax::Statement::FieldAssignment(statement) => {
                self.visit_expression(&statement.place.receiver);
                self.visit_expression(&statement.value);
            }
            syntax::Statement::ObjectAssignment(statement) => {
                self.visit_expression(&statement.place);
                self.visit_expression(&statement.value);
            }
        }
    }

    fn visit_expression(&mut self, expression: &syntax::Expression) {
        match expression {
            syntax::Expression::Absent(_)
            | syntax::Expression::Identifier(_)
            | syntax::Expression::NumericLiteral(_)
            | syntax::Expression::ByteLiteral(_)
            | syntax::Expression::StringLiteral(_)
            | syntax::Expression::Boolean(_)
            | syntax::Expression::SelfValue(_) => {}
            syntax::Expression::Present(expression) => self.visit_expression(&expression.value),
            syntax::Expression::GenericTypeApplication(application) => {
                self.visit_named_type(&application.target)
            }
            syntax::Expression::GenericStaticSelection(selection) => {
                self.visit_named_type(&selection.target)
            }
            syntax::Expression::Unary(expression) => self.visit_expression(&expression.operand),
            syntax::Expression::Binary(expression) => {
                self.visit_expression(&expression.left);
                self.visit_expression(&expression.right);
            }
            syntax::Expression::Logical(expression) => {
                self.visit_expression(&expression.left);
                self.visit_expression(&expression.right);
            }
            syntax::Expression::Range(expression) => {
                self.visit_expression(&expression.lower);
                self.visit_expression(&expression.upper);
            }
            syntax::Expression::TypeTest(expression) => {
                self.visit_expression(&expression.source);
                self.visit_named_type(&expression.target);
            }
            syntax::Expression::PresenceTest(expression) => {
                self.visit_expression(&expression.source)
            }
            syntax::Expression::Unwrap(expression) => self.visit_expression(&expression.source),
            syntax::Expression::PrimitiveCast(expression) => {
                self.visit_expression(&expression.source)
            }
            syntax::Expression::ObjectCast(expression) => {
                self.visit_named_type(&expression.target);
                self.visit_expression(&expression.source);
            }
            syntax::Expression::Allocation(expression) => {
                self.visit_named_type(&expression.target);
                self.visit_call_arguments(&expression.arguments);
            }
            syntax::Expression::OptionalBoxAllocation(expression) => {
                self.visit_type(&expression.target);
                if let syntax::OptionalBoxInitializer::Value { value, .. } = &expression.initializer
                {
                    self.visit_expression(value);
                }
            }
            syntax::Expression::ArrayConstruction(expression) => {
                self.visit_type(&expression.array_type);
                match &expression.arguments {
                    syntax::ArrayConstructionArguments::Empty { .. } => {}
                    syntax::ArrayConstructionArguments::Length { length, .. } => {
                        self.visit_expression(length)
                    }
                    syntax::ArrayConstructionArguments::Copy { source, .. } => {
                        self.visit_expression(source)
                    }
                    syntax::ArrayConstructionArguments::Elements(elements) => {
                        self.visit_expressions(&elements.elements)
                    }
                }
            }
            syntax::Expression::Call(expression) => {
                self.visit_expression(&expression.callee);
                self.visit_call_arguments(&expression.arguments);
            }
            syntax::Expression::Grouped(expression) => {
                self.visit_expression(&expression.expression)
            }
            syntax::Expression::MemberAccess(expression) => {
                self.visit_expression(&expression.receiver)
            }
            syntax::Expression::BracketProjection(expression) => {
                self.visit_expression(&expression.receiver);
                match &expression.bounds {
                    syntax::BracketProjectionBounds::Index(index) => self.visit_expression(index),
                    syntax::BracketProjectionBounds::Slice { start, end, .. } => {
                        if let Some(start) = start {
                            self.visit_expression(start);
                        }
                        if let Some(end) = end {
                            self.visit_expression(end);
                        }
                    }
                }
            }
        }
    }

    fn visit_call_arguments(&mut self, arguments: &syntax::CallArguments) {
        match arguments {
            syntax::CallArguments::Ordinary(arguments) => self.visit_expressions(arguments),
            syntax::CallArguments::Copy { source, .. } => self.visit_expression(source),
        }
    }

    fn visit_expressions(&mut self, expressions: &[syntax::Expression]) {
        for expression in expressions {
            self.visit_expression(expression);
        }
    }
}
