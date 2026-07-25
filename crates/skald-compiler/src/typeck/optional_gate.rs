//! Boundary for optional positions whose executable profile has not landed.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    resolve::{ResolvedProgram, ResolvedType, ResolvedTypeKind},
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
            "optional containers do not yet support alias parameter binding",
        )
        .with_note("use an owning optional value parameter; `ref?` is not a reference type"),
    );
    true
}

fn first_unsupported_optional_span(program: &ResolvedProgram) -> Option<Span> {
    let mut first = None;

    for interface in program.interfaces.iter() {
        for requirement in &interface.requirements {
            for parameter in &requirement.parameters {
                record_unsupported_parameter(
                    &mut first,
                    parameter.binding_mode,
                    &parameter.type_syntax,
                );
            }
        }
    }
    for class in program.classes.iter() {
        for initializer in &class.initializers {
            record_parameters(&mut first, &initializer.parameters);
        }
        if let Some(copy) = &class.copy_constructor_declaration {
            record_parameters(&mut first, &copy.parameters);
        }
        if let Some(assignment) = &class.copy_assignment_declaration {
            record_unsupported_parameter(
                &mut first,
                assignment.parameter.binding_mode,
                &assignment.parameter.type_syntax,
            );
        }
        for method in &class.methods {
            record_parameters(&mut first, &method.parameters);
        }
    }
    for declaration in program.declarations.iter() {
        if matches!(
            declaration.linkage,
            crate::resolve::ResolvedFunctionLinkage::Internal
        ) {
            record_parameters(&mut first, &declaration.parameters);
        }
    }

    first
}

fn record_parameters(first: &mut Option<Span>, parameters: &[crate::resolve::ResolvedParameter]) {
    for parameter in parameters {
        record_unsupported_parameter(first, parameter.binding_mode, &parameter.type_syntax);
    }
}

fn record_unsupported_parameter(
    first: &mut Option<Span>,
    mode: crate::resolve::ResolvedParameterBindingMode,
    ty: &ResolvedType,
) {
    if mode != crate::resolve::ResolvedParameterBindingMode::Value
        && matches!(
            ty.kind,
            ResolvedTypeKind::Optional { .. } | ResolvedTypeKind::OptionalShared { .. }
        )
    {
        record_span(first, ty.span);
    }
}

fn record_span(first: &mut Option<Span>, candidate: Span) {
    if first.is_none_or(|span| candidate.range().start() < span.range().start()) {
        *first = Some(candidate);
    }
}
