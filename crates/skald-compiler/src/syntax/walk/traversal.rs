use super::{SyntaxNode, SyntaxVisitor, WalkControl};
use crate::syntax::ast::{
    ArrayConstructionArguments, BracketProjectionBounds, CallArguments, ClassMember, Expression,
    ForInSource, GenericWhereClause, OptionalBoxInitializer, Parameter, Statement,
    TopLevelDeclaration,
};

#[derive(Clone, Copy)]
enum WorkItem<'ast> {
    Enter(SyntaxNode<'ast>),
    Leave(SyntaxNode<'ast>),
}

/// Walks a syntax subtree without using the process call stack.
pub(crate) fn walk<'ast>(
    root: impl Into<SyntaxNode<'ast>>,
    visitor: &mut impl SyntaxVisitor<'ast>,
) {
    let mut pending = vec![WorkItem::Enter(root.into())];

    while let Some(item) = pending.pop() {
        match item {
            WorkItem::Enter(node) => {
                let control = visitor.enter(node);
                pending.push(WorkItem::Leave(node));
                if control == WalkControl::Continue {
                    push_children(node, &mut pending);
                }
            }
            WorkItem::Leave(node) => visitor.leave(node),
        }
    }
}

fn push_children<'ast>(node: SyntaxNode<'ast>, pending: &mut Vec<WorkItem<'ast>>) {
    let first_child = pending.len();
    emit_children(node, |child| pending.push(WorkItem::Enter(child)));
    pending[first_child..].reverse();
}

fn emit_children<'ast>(node: SyntaxNode<'ast>, mut emit: impl FnMut(SyntaxNode<'ast>)) {
    match node {
        SyntaxNode::CompilationUnit(unit) => {
            for import in &unit.imports {
                emit(SyntaxNode::Import(import));
            }
            for declaration in &unit.declarations {
                emit(SyntaxNode::Declaration(declaration));
            }
        }
        SyntaxNode::Import(_) | SyntaxNode::Type(_) | SyntaxNode::NamedType(_) => {}
        SyntaxNode::Declaration(declaration) => emit_declaration_children(declaration, &mut emit),
        SyntaxNode::ClassMember(member) => emit_member_children(member, &mut emit),
        SyntaxNode::Block(block) => {
            for statement in &block.statements {
                emit(SyntaxNode::Statement(statement));
            }
        }
        SyntaxNode::Statement(statement) => emit_statement_children(statement, &mut emit),
        SyntaxNode::Expression(expression) => emit_expression_children(expression, &mut emit),
    }
}

fn emit_declaration_children<'ast>(
    declaration: &'ast TopLevelDeclaration,
    emit: &mut impl FnMut(SyntaxNode<'ast>),
) {
    match declaration {
        TopLevelDeclaration::Function(function) => {
            emit_parameter_types(&function.parameters, emit);
            emit(SyntaxNode::Type(&function.return_type));
            emit(SyntaxNode::Block(&function.body));
        }
        TopLevelDeclaration::ExternalFunction(function) => {
            emit_parameter_types(&function.parameters, emit);
            emit(SyntaxNode::Type(&function.return_type));
        }
        TopLevelDeclaration::IntrinsicFunction(function) => {
            emit_parameter_types(&function.parameters, emit);
            emit(SyntaxNode::Type(&function.return_type));
        }
        TopLevelDeclaration::Class(class) => {
            if let Some(base) = &class.direct_base {
                emit(SyntaxNode::NamedType(base));
            }
            for interface in &class.implemented_interfaces {
                emit(SyntaxNode::NamedType(interface));
            }
            if let Some(where_clause) = &class.where_clause {
                emit_where_clause_types(where_clause, emit);
            }
            for member in &class.members {
                emit(SyntaxNode::ClassMember(member));
            }
        }
        TopLevelDeclaration::Interface(interface) => {
            if let Some(where_clause) = &interface.where_clause {
                emit_where_clause_types(where_clause, emit);
            }
            for requirement in &interface.requirements {
                emit_parameter_types(&requirement.parameters, emit);
                emit(SyntaxNode::Type(&requirement.return_type));
            }
        }
    }
}

fn emit_member_children<'ast>(member: &'ast ClassMember, emit: &mut impl FnMut(SyntaxNode<'ast>)) {
    match member {
        ClassMember::Field(field) => emit(SyntaxNode::Type(&field.type_syntax)),
        ClassMember::StaticField(field) => {
            emit(SyntaxNode::Type(&field.type_syntax));
            if let Some(initializer) = &field.initializer {
                emit(SyntaxNode::Expression(&initializer.expression));
            }
        }
        ClassMember::Initializer(initializer) => {
            emit_parameter_types(&initializer.parameters, emit);
            emit(SyntaxNode::Block(&initializer.body));
        }
        ClassMember::CopyConstructor(constructor) => {
            emit_parameter_types(&constructor.parameters, emit);
            emit(SyntaxNode::Block(&constructor.body));
        }
        ClassMember::CopyAssignment(assignment) => {
            emit_parameter_types(&assignment.parameters, emit);
            emit(SyntaxNode::Block(&assignment.body));
        }
        ClassMember::Destructor(destructor) => emit(SyntaxNode::Block(&destructor.body)),
        ClassMember::Method(method) => {
            emit_parameter_types(&method.parameters, emit);
            emit(SyntaxNode::Type(&method.return_type));
            emit(SyntaxNode::Block(&method.body));
        }
    }
}

fn emit_parameter_types<'ast>(
    parameters: &'ast [Parameter],
    emit: &mut impl FnMut(SyntaxNode<'ast>),
) {
    for parameter in parameters {
        emit(SyntaxNode::Type(&parameter.type_syntax));
    }
}

fn emit_where_clause_types<'ast>(
    where_clause: &'ast GenericWhereClause,
    emit: &mut impl FnMut(SyntaxNode<'ast>),
) {
    for requirement in &where_clause.requirements {
        emit(SyntaxNode::NamedType(&requirement.interface));
    }
}

fn emit_statement_children<'ast>(
    statement: &'ast Statement,
    emit: &mut impl FnMut(SyntaxNode<'ast>),
) {
    match statement {
        Statement::BaseInitialization(statement) => {
            emit_expressions(&statement.arguments, emit);
        }
        Statement::Local(statement) => {
            emit(SyntaxNode::Type(&statement.type_syntax));
            emit(SyntaxNode::Expression(&statement.initializer));
        }
        Statement::Return(statement) => {
            if let Some(value) = &statement.value {
                emit(SyntaxNode::Expression(value));
            }
        }
        Statement::Break(_) | Statement::Continue(_) => {}
        Statement::Expression(statement) => {
            emit(SyntaxNode::Expression(&statement.expression));
        }
        Statement::Conditional(statement) => {
            emit(SyntaxNode::Expression(&statement.if_arm.condition));
            emit(SyntaxNode::Block(&statement.if_arm.body));
            for arm in &statement.elif_arms {
                emit(SyntaxNode::Expression(&arm.condition));
                emit(SyntaxNode::Block(&arm.body));
            }
            if let Some(block) = &statement.else_block {
                emit(SyntaxNode::Block(block));
            }
        }
        Statement::While(statement) => {
            emit(SyntaxNode::Expression(&statement.condition));
            emit(SyntaxNode::Block(&statement.body));
        }
        Statement::ForIn(statement) => {
            if let Some(annotation) = &statement.annotation {
                emit(SyntaxNode::Type(&annotation.type_syntax));
            }
            emit_for_in_source_children(&statement.source, emit);
            emit(SyntaxNode::Block(&statement.body));
        }
        Statement::Block(block) => emit(SyntaxNode::Block(block)),
        Statement::FieldAssignment(statement) => {
            emit(SyntaxNode::Expression(&statement.place.receiver));
            emit(SyntaxNode::Expression(&statement.value));
        }
        Statement::ObjectAssignment(statement) => {
            emit(SyntaxNode::Expression(&statement.place));
            emit(SyntaxNode::Expression(&statement.value));
        }
    }
}

fn emit_for_in_source_children<'ast>(
    source: &'ast ForInSource,
    emit: &mut impl FnMut(SyntaxNode<'ast>),
) {
    match source {
        ForInSource::Iterable(iterable) => emit(SyntaxNode::Expression(iterable)),
        ForInSource::Range(range) => {
            emit(SyntaxNode::Expression(&range.lower));
            emit(SyntaxNode::Expression(&range.upper));
        }
    }
}

fn emit_expression_children<'ast>(
    expression: &'ast Expression,
    emit: &mut impl FnMut(SyntaxNode<'ast>),
) {
    match expression {
        Expression::Absent(_)
        | Expression::Identifier(_)
        | Expression::NumericLiteral(_)
        | Expression::ByteLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::Boolean(_)
        | Expression::SelfValue(_) => {}
        Expression::Present(expression) => emit(SyntaxNode::Expression(&expression.value)),
        Expression::GenericTypeApplication(expression) => {
            emit(SyntaxNode::NamedType(&expression.target));
        }
        Expression::GenericStaticSelection(expression) => {
            emit(SyntaxNode::NamedType(&expression.target));
        }
        Expression::Unary(expression) => emit(SyntaxNode::Expression(&expression.operand)),
        Expression::Binary(expression) => {
            emit(SyntaxNode::Expression(&expression.left));
            emit(SyntaxNode::Expression(&expression.right));
        }
        Expression::Logical(expression) => {
            emit(SyntaxNode::Expression(&expression.left));
            emit(SyntaxNode::Expression(&expression.right));
        }
        Expression::TypeTest(expression) => {
            emit(SyntaxNode::Expression(&expression.source));
            emit(SyntaxNode::NamedType(&expression.target));
        }
        Expression::PresenceTest(expression) => {
            emit(SyntaxNode::Expression(&expression.source));
        }
        Expression::Unwrap(expression) => emit(SyntaxNode::Expression(&expression.source)),
        Expression::PrimitiveCast(expression) => {
            emit(SyntaxNode::Expression(&expression.source));
        }
        Expression::ObjectCast(expression) => {
            emit(SyntaxNode::NamedType(&expression.target));
            emit(SyntaxNode::Expression(&expression.source));
        }
        Expression::Allocation(expression) => {
            emit(SyntaxNode::NamedType(&expression.target));
            emit_call_argument_children(&expression.arguments, emit);
        }
        Expression::OptionalBoxAllocation(expression) => {
            emit(SyntaxNode::Type(&expression.target));
            emit_optional_initializer_children(&expression.initializer, emit);
        }
        Expression::ArrayConstruction(expression) => {
            emit(SyntaxNode::Type(&expression.array_type));
            emit_array_argument_children(&expression.arguments, emit);
        }
        Expression::Call(expression) => {
            emit(SyntaxNode::Expression(&expression.callee));
            emit_call_argument_children(&expression.arguments, emit);
        }
        Expression::Grouped(expression) => emit(SyntaxNode::Expression(&expression.expression)),
        Expression::MemberAccess(expression) => {
            emit(SyntaxNode::Expression(&expression.receiver));
        }
        Expression::BracketProjection(expression) => {
            emit(SyntaxNode::Expression(&expression.receiver));
            emit_bracket_bound_children(&expression.bounds, emit);
        }
    }
}

fn emit_call_argument_children<'ast>(
    arguments: &'ast CallArguments,
    emit: &mut impl FnMut(SyntaxNode<'ast>),
) {
    match arguments {
        CallArguments::Ordinary(arguments) => emit_expressions(arguments, emit),
        CallArguments::Copy { source, .. } => emit(SyntaxNode::Expression(source)),
    }
}

fn emit_array_argument_children<'ast>(
    arguments: &'ast ArrayConstructionArguments,
    emit: &mut impl FnMut(SyntaxNode<'ast>),
) {
    match arguments {
        ArrayConstructionArguments::Empty { .. } => {}
        ArrayConstructionArguments::Length { length, .. } => {
            emit(SyntaxNode::Expression(length));
        }
        ArrayConstructionArguments::Copy { source, .. } => {
            emit(SyntaxNode::Expression(source));
        }
        ArrayConstructionArguments::Indexed(initializer) => {
            emit(SyntaxNode::Expression(&initializer.length));
            emit(SyntaxNode::Expression(&initializer.element));
        }
        ArrayConstructionArguments::Elements(elements) => {
            emit_expressions(&elements.elements, emit);
        }
    }
}

fn emit_optional_initializer_children<'ast>(
    initializer: &'ast OptionalBoxInitializer,
    emit: &mut impl FnMut(SyntaxNode<'ast>),
) {
    match initializer {
        OptionalBoxInitializer::Absent { .. } => {}
        OptionalBoxInitializer::Value { value, .. } => {
            emit(SyntaxNode::Expression(value));
        }
    }
}

fn emit_bracket_bound_children<'ast>(
    bounds: &'ast BracketProjectionBounds,
    emit: &mut impl FnMut(SyntaxNode<'ast>),
) {
    match bounds {
        BracketProjectionBounds::Index(index) => emit(SyntaxNode::Expression(index)),
        BracketProjectionBounds::Slice { start, end, .. } => {
            if let Some(start) = start {
                emit(SyntaxNode::Expression(start));
            }
            if let Some(end) = end {
                emit(SyntaxNode::Expression(end));
            }
        }
    }
}

fn emit_expressions<'ast>(
    expressions: &'ast [Expression],
    emit: &mut impl FnMut(SyntaxNode<'ast>),
) {
    for expression in expressions {
        emit(SyntaxNode::Expression(expression));
    }
}
