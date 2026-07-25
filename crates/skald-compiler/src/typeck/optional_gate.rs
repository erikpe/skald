//! Boundary for optional positions whose executable profile has not landed.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    resolve::{ResolvedOptionalPayload, ResolvedProgram, ResolvedType, ResolvedTypeKind},
    source::Span,
};

use super::OPTIONAL_VALUES_NOT_IMPLEMENTED;

pub(super) fn reject_unsupported_optionals(
    program: &ResolvedProgram,
    diagnostics: &mut Diagnostics,
) -> bool {
    let Some(span) = first_unsupported_optional_span(program) else {
        return false;
    };
    diagnostics.push(
        Diagnostic::error(
            OPTIONAL_VALUES_NOT_IMPLEMENTED,
            "this optional-value position is not executable yet",
        )
        .with_primary_label(
            span,
            "only primitive optional locals are executable in the current profile",
        )
        .with_note(
            "optional fields, parameters, results, class payloads, shared owners, and aliases remain planned",
        ),
    );
    true
}

fn first_unsupported_optional_span(program: &ResolvedProgram) -> Option<Span> {
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
            record_unsupported_local_type(&mut first, &local.type_syntax);
        }
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
                record_unsupported_local_type(&mut first, &local.type_syntax);
            }
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

fn record_unsupported_local_type(first: &mut Option<Span>, ty: &ResolvedType) {
    if matches!(
        ty.kind,
        ResolvedTypeKind::Optional {
            payload: ResolvedOptionalPayload::Class(_),
            ..
        } | ResolvedTypeKind::OptionalShared { .. }
    ) {
        record_span(first, ty.span);
    }
}

fn record_span(first: &mut Option<Span>, candidate: Span) {
    if first.is_none_or(|span| candidate.range().start() < span.range().start()) {
        *first = Some(candidate);
    }
}
