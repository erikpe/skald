//! Source-order discovery of explicit generic applications in the AST.

use super::syntax_type_closer::SyntaxTypeCloser;
use std::collections::HashMap;

use crate::{
    identity::ClassTemplateId,
    resolve::{ResolvedSharedTarget, ResolvedTypeKind},
    syntax,
};

pub(super) struct SourceRequestScanner<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup> {
    resolver: SyntaxTypeCloser<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup>,
    range_template: Option<ClassTemplateId>,
    scopes: Vec<HashMap<String, ResolvedTypeKind>>,
    function_results: HashMap<String, syntax::TypeSyntax>,
}

impl<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup>
    SourceRequestScanner<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup>
{
    pub(super) fn new(
        resolver: SyntaxTypeCloser<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup>,
        range_template: Option<ClassTemplateId>,
    ) -> Self {
        Self {
            resolver,
            range_template,
            scopes: Vec::new(),
            function_results: HashMap::new(),
        }
    }

    pub(super) fn visit_unit(&mut self, unit: &syntax::CompilationUnit) {
        for declaration in &unit.declarations {
            let (name, result) = match declaration {
                syntax::TopLevelDeclaration::Function(function) => {
                    (&function.name, &function.return_type)
                }
                syntax::TopLevelDeclaration::ExternalFunction(function) => {
                    (&function.name, &function.return_type)
                }
                syntax::TopLevelDeclaration::IntrinsicFunction(function) => {
                    (&function.name, &function.return_type)
                }
                syntax::TopLevelDeclaration::Class(_)
                | syntax::TopLevelDeclaration::Interface(_) => {
                    continue;
                }
            };
            self.function_results
                .insert(name.text.to_string(), result.clone());
        }
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
        self.scopes.push(HashMap::new());
        for parameter in parameters {
            if let Some(ty) = self.resolver.close(&parameter.type_syntax) {
                self.scopes
                    .last_mut()
                    .expect("callable scope exists")
                    .insert(parameter.name.text.to_string(), ty);
            }
        }
    }

    fn end_callable(&mut self) {
        self.scopes.pop().expect("callable scope exists");
    }

    fn visit_type(&mut self, syntax: &syntax::TypeSyntax) {
        let _ = self.resolver.close(syntax);
    }

    fn visit_named_type(&mut self, syntax: &syntax::NamedTypeSyntax) {
        let _ = self.resolver.close_named(syntax, false);
    }

    fn visit_block(&mut self, block: &syntax::Block) {
        self.scopes.push(HashMap::new());
        for statement in &block.statements {
            self.visit_statement(statement);
        }
        self.scopes.pop().expect("block scope exists");
    }

    fn visit_statement(&mut self, statement: &syntax::Statement) {
        match statement {
            syntax::Statement::BaseInitialization(statement) => {
                self.visit_expressions(&statement.arguments)
            }
            syntax::Statement::Local(statement) => {
                self.visit_expression(&statement.initializer);
                if let Some(ty) = self.resolver.close(&statement.type_syntax) {
                    self.scopes
                        .last_mut()
                        .expect("local declaration occurs in a scope")
                        .insert(statement.name.text.to_string(), ty);
                }
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
                let lower = self.static_type(&expression.lower);
                let upper = self.static_type(&expression.upper);
                if let (Some(template), Some(lower), Some(upper)) =
                    (self.range_template, lower, upper)
                {
                    if lower == upper {
                        let _ =
                            self.resolver
                                .request_range(template, lower, expression.operator_span);
                    }
                }
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

    fn static_type(&mut self, expression: &syntax::Expression) -> Option<ResolvedTypeKind> {
        match expression {
            syntax::Expression::NumericLiteral(literal) => Some(match literal.kind {
                crate::literal::NumericLiteralKind::I64(_) => ResolvedTypeKind::I64,
                crate::literal::NumericLiteralKind::U64(_) => ResolvedTypeKind::U64,
                crate::literal::NumericLiteralKind::U8(_) => ResolvedTypeKind::U8,
                crate::literal::NumericLiteralKind::F64 => ResolvedTypeKind::F64,
            }),
            syntax::Expression::ByteLiteral(_) => Some(ResolvedTypeKind::U8),
            syntax::Expression::Boolean(_)
            | syntax::Expression::Logical(_)
            | syntax::Expression::TypeTest(_)
            | syntax::Expression::PresenceTest(_) => Some(ResolvedTypeKind::Bool),
            syntax::Expression::Identifier(identifier) if !identifier.name.is_qualified() => self
                .scopes
                .iter()
                .rev()
                .find_map(|scope| scope.get(identifier.name.text.as_str()).copied()),
            syntax::Expression::Unary(unary) => self
                .static_type(&unary.operand)
                .filter(|kind| is_primitive_value(*kind)),
            syntax::Expression::Binary(binary) => match binary.operator {
                syntax::BinaryOperator::Equal
                | syntax::BinaryOperator::NotEqual
                | syntax::BinaryOperator::LessThan
                | syntax::BinaryOperator::LessEqual
                | syntax::BinaryOperator::GreaterThan
                | syntax::BinaryOperator::GreaterEqual => Some(ResolvedTypeKind::Bool),
                syntax::BinaryOperator::ShiftLeft | syntax::BinaryOperator::ShiftRight => {
                    let left = self.static_type(&binary.left)?;
                    let right = self.static_type(&binary.right)?;
                    (is_integer_value(left) && right == ResolvedTypeKind::U64).then_some(left)
                }
                _ => {
                    let left = self.static_type(&binary.left)?;
                    let right = self.static_type(&binary.right)?;
                    (left == right && is_primitive_value(left)).then_some(left)
                }
            },
            syntax::Expression::PrimitiveCast(cast) => Some(match cast.target {
                syntax::PrimitiveType::I64 => ResolvedTypeKind::I64,
                syntax::PrimitiveType::U64 => ResolvedTypeKind::U64,
                syntax::PrimitiveType::U8 => ResolvedTypeKind::U8,
                syntax::PrimitiveType::F64 => ResolvedTypeKind::F64,
                syntax::PrimitiveType::Bool => ResolvedTypeKind::Bool,
            }),
            syntax::Expression::ObjectCast(cast) => self.resolver.close_named(&cast.target, false),
            syntax::Expression::Allocation(allocation) => {
                let target = self.resolver.close_named(&allocation.target, false)?;
                ResolvedSharedTarget::from_direct_type(target).map(ResolvedTypeKind::Shared)
            }
            syntax::Expression::ArrayConstruction(construction) => {
                self.resolver.close(&construction.array_type)
            }
            syntax::Expression::Call(call) => {
                if let Some(ty) = self.resolver.constructor_type(&call.callee) {
                    return Some(ty);
                }
                let syntax::Expression::Identifier(identifier) = call.callee.as_ref() else {
                    return None;
                };
                if identifier.name.is_qualified() {
                    return None;
                }
                let result = self
                    .function_results
                    .get(identifier.name.text.as_str())?
                    .clone();
                self.resolver.close(&result)
            }
            syntax::Expression::Grouped(grouped) => self.static_type(&grouped.expression),
            syntax::Expression::Range(range) => {
                let lower = self.static_type(&range.lower)?;
                let upper = self.static_type(&range.upper)?;
                if lower != upper {
                    return None;
                }
                let template = self.range_template?;
                self.resolver
                    .request_range(template, lower, range.operator_span)
                    .map(ResolvedTypeKind::Class)
            }
            _ => None,
        }
    }
}

const fn is_integer_value(kind: ResolvedTypeKind) -> bool {
    matches!(
        kind,
        ResolvedTypeKind::I64 | ResolvedTypeKind::U64 | ResolvedTypeKind::U8
    )
}

const fn is_primitive_value(kind: ResolvedTypeKind) -> bool {
    matches!(
        kind,
        ResolvedTypeKind::I64
            | ResolvedTypeKind::U64
            | ResolvedTypeKind::U8
            | ResolvedTypeKind::F64
            | ResolvedTypeKind::Bool
    )
}
