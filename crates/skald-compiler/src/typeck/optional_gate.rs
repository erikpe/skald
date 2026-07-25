//! Temporary boundary between resolved optional syntax and executable HIR.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    resolve::{
        ResolvedBlock, ResolvedConstructionMode, ResolvedExpression, ResolvedObjectReceiver,
        ResolvedProgram, ResolvedStatement, ResolvedType, ResolvedTypeKind,
    },
    source::Span,
};

use super::OPTIONAL_VALUES_NOT_IMPLEMENTED;

pub(super) fn reject_unimplemented_optionals(
    program: &ResolvedProgram,
    diagnostics: &mut Diagnostics,
) -> bool {
    let Some(span) = first_optional_span(program) else {
        return false;
    };
    diagnostics.push(
        Diagnostic::error(
            OPTIONAL_VALUES_NOT_IMPLEMENTED,
            "optional values are not executable yet",
        )
        .with_primary_label(
            span,
            "optional syntax is resolved, but HIR and execution support are not implemented",
        )
        .with_note("the compiler currently accepts optional syntax only through name resolution"),
    );
    true
}

fn first_optional_span(program: &ResolvedProgram) -> Option<Span> {
    let mut first = None;

    for interface in program.interfaces.iter() {
        for requirement in &interface.requirements {
            record_type(&mut first, &requirement.return_type);
            for parameter in &requirement.parameters {
                record_type(&mut first, &parameter.type_syntax);
            }
        }
    }

    for class in program.classes.iter() {
        for field in &class.fields {
            record_type(&mut first, &field.type_syntax);
        }
        for initializer in &class.initializers {
            record_parameters(&mut first, &initializer.parameters);
        }
        if let Some(copy) = &class.copy_constructor_declaration {
            record_parameters(&mut first, &copy.parameters);
        }
        if let Some(assignment) = &class.copy_assignment_declaration {
            record_type(&mut first, &assignment.parameter.type_syntax);
        }
        for method in &class.methods {
            record_parameters(&mut first, &method.parameters);
            record_type(&mut first, &method.return_type);
        }
    }

    for declaration in program.declarations.iter() {
        record_parameters(&mut first, &declaration.parameters);
        record_type(&mut first, &declaration.return_type);
    }

    for definition in program.definitions.iter() {
        for local in &definition.locals {
            record_type(&mut first, &local.type_syntax);
        }
        record_block(&mut first, &definition.body);
    }
    for class in program.class_definitions.iter() {
        for member in class
            .initializers
            .iter()
            .chain(class.copy_constructor.iter())
            .chain(class.copy_assignment.iter())
            .chain(class.destructor.iter())
            .chain(class.methods.iter())
        {
            for local in &member.locals {
                record_type(&mut first, &local.type_syntax);
            }
            record_block(&mut first, &member.body);
        }
    }

    first
}

fn record_parameters(first: &mut Option<Span>, parameters: &[crate::resolve::ResolvedParameter]) {
    for parameter in parameters {
        record_type(first, &parameter.type_syntax);
    }
}

fn record_type(first: &mut Option<Span>, ty: &ResolvedType) {
    if matches!(
        ty.kind,
        ResolvedTypeKind::Optional { .. } | ResolvedTypeKind::OptionalShared { .. }
    ) {
        record_span(first, ty.span);
    }
}

fn record_block(first: &mut Option<Span>, block: &ResolvedBlock) {
    for statement in &block.statements {
        match statement {
            ResolvedStatement::BaseInitialization(statement) => {
                record_expressions(first, &statement.arguments)
            }
            ResolvedStatement::Local(statement) => record_expression(first, &statement.initializer),
            ResolvedStatement::Return(statement) => {
                if let Some(value) = &statement.value {
                    record_expression(first, value);
                }
            }
            ResolvedStatement::Expression(statement) => {
                record_expression(first, &statement.expression)
            }
            ResolvedStatement::Conditional(statement) => {
                for arm in &statement.arms {
                    record_expression(first, &arm.condition);
                    record_block(first, &arm.body);
                }
                if let Some(else_block) = &statement.else_block {
                    record_block(first, else_block);
                }
            }
            ResolvedStatement::Block(block) => record_block(first, block),
            ResolvedStatement::FieldAssignment(statement) => {
                record_receiver(first, &statement.receiver);
                record_expression(first, &statement.value);
            }
            ResolvedStatement::ObjectAssignment(statement) => {
                record_expression(first, &statement.source)
            }
            ResolvedStatement::SharedAssignment(statement) => {
                record_expression(first, &statement.source)
            }
            ResolvedStatement::OptionalAssignment(statement) => {
                record_span(first, statement.span);
                record_expression(first, &statement.source);
            }
        }
    }
}

fn record_expressions(first: &mut Option<Span>, expressions: &[ResolvedExpression]) {
    for expression in expressions {
        record_expression(first, expression);
    }
}

fn record_expression(first: &mut Option<Span>, expression: &ResolvedExpression) {
    match expression {
        ResolvedExpression::Absent(expression) => record_span(first, expression.span),
        ResolvedExpression::PresenceTest(expression) => record_span(first, expression.span),
        ResolvedExpression::Unwrap(expression) => record_span(first, expression.span),
        ResolvedExpression::Unary(expression) => record_expression(first, &expression.operand),
        ResolvedExpression::Dereference(expression) => record_expression(first, &expression.source),
        ResolvedExpression::Binary(expression) => {
            record_expression(first, &expression.left);
            record_expression(first, &expression.right);
        }
        ResolvedExpression::TypeTest(expression) => record_expression(first, &expression.source),
        ResolvedExpression::ObjectCast(expression) => record_expression(first, &expression.source),
        ResolvedExpression::Allocation(expression) => {
            record_construction_mode(first, &expression.mode)
        }
        ResolvedExpression::DirectCall(expression) => {
            record_expressions(first, &expression.arguments)
        }
        ResolvedExpression::Grouped(expression) => record_expression(first, &expression.expression),
        ResolvedExpression::FieldAccess(expression) => record_receiver(first, &expression.receiver),
        ResolvedExpression::MethodCall(expression) => {
            record_receiver(first, &expression.receiver);
            record_expressions(first, &expression.arguments);
        }
        ResolvedExpression::InterfaceCall(expression) => {
            record_expressions(first, &expression.arguments)
        }
        ResolvedExpression::Construct(expression) => {
            record_construction_mode(first, &expression.mode)
        }
        ResolvedExpression::Binding(_)
        | ResolvedExpression::NumericLiteral(_)
        | ResolvedExpression::Boolean(_) => {}
    }
}

fn record_construction_mode(first: &mut Option<Span>, mode: &ResolvedConstructionMode) {
    match mode {
        ResolvedConstructionMode::Initialize { arguments } => record_expressions(first, arguments),
        ResolvedConstructionMode::Copy { source, .. } => record_expression(first, source),
    }
}

fn record_receiver(first: &mut Option<Span>, receiver: &ResolvedObjectReceiver) {
    match receiver {
        ResolvedObjectReceiver::BindingPath(_) => {}
        ResolvedObjectReceiver::CastRelative { cast, .. } => record_expression(first, &cast.source),
        ResolvedObjectReceiver::Dereference { dereference, .. } => {
            record_expression(first, &dereference.source)
        }
    }
}

fn record_span(first: &mut Option<Span>, candidate: Span) {
    if match first {
        Some(span) => candidate.range().start() < span.range().start(),
        None => true,
    } {
        *first = Some(candidate);
    }
}
