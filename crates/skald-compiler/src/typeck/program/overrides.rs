//! Exact virtual-override signature validation.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    resolve::{
        ResolvedMethodDeclaration, ResolvedMethodDispatch, ResolvedParameter,
        ResolvedParameterBindingMode, ResolvedProgram, ResolvedReceiverAccess,
    },
    source::Span,
};

use super::{same_resolved_type, INVALID_OVERRIDE_SIGNATURE};

pub(super) fn validate_override_signatures(
    program: &ResolvedProgram,
    diagnostics: &mut Diagnostics,
) {
    for class in program.classes.iter() {
        for method in &class.methods {
            let Some(ResolvedMethodDispatch::Override { overridden, .. }) = method.kind.dispatch()
            else {
                continue;
            };
            let inherited = program
                .method(overridden)
                .expect("resolved override target must be declared");
            if let Some(diagnostic) = signature_mismatch(method, inherited) {
                diagnostics.push(diagnostic);
            }
        }
    }
}

fn signature_mismatch(
    method: &ResolvedMethodDeclaration,
    inherited: &ResolvedMethodDeclaration,
) -> Option<Diagnostic> {
    let method_access = method.kind.receiver_access()?;
    let inherited_access = inherited.kind.receiver_access()?;
    if method_access != inherited_access {
        return Some(mismatch(
            format!(
                "override method `{}` has incompatible receiver access",
                method.name
            ),
            method.name_span,
            format!(
                "this method has a {} receiver",
                receiver_name(method_access)
            ),
            inherited.name_span,
            format!(
                "inherited virtual method has a {} receiver",
                receiver_name(inherited_access)
            ),
        ));
    }
    if method.parameters.len() != inherited.parameters.len() {
        return Some(mismatch(
            format!(
                "override method `{}` has {} parameters, expected {}",
                method.name,
                method.parameters.len(),
                inherited.parameters.len()
            ),
            method.span,
            "override parameter list has the wrong length".to_owned(),
            inherited.span,
            "inherited virtual method declared here",
        ));
    }
    for (index, (parameter, expected)) in method
        .parameters
        .iter()
        .zip(&inherited.parameters)
        .enumerate()
    {
        if parameter_mode(parameter) != parameter_mode(expected) {
            return Some(mismatch(
                format!(
                    "override method `{}` has an incompatible binding mode for parameter {}",
                    method.name,
                    index + 1
                ),
                parameter.span,
                format!("this parameter is {}", parameter_mode(parameter)),
                expected.span,
                format!(
                    "inherited virtual parameter is {}",
                    parameter_mode(expected)
                ),
            ));
        }
        if !same_resolved_type(&parameter.type_syntax, &expected.type_syntax) {
            return Some(mismatch(
                format!(
                    "override method `{}` has an incompatible type for parameter {}",
                    method.name,
                    index + 1
                ),
                parameter.type_syntax.span,
                "this parameter has a different type".to_owned(),
                expected.type_syntax.span,
                "inherited virtual parameter type declared here",
            ));
        }
    }
    if !same_resolved_type(&method.return_type, &inherited.return_type) {
        return Some(mismatch(
            format!(
                "override method `{}` has an incompatible result type",
                method.name
            ),
            method.return_type.span,
            "this result type differs".to_owned(),
            inherited.return_type.span,
            "inherited virtual result type declared here",
        ));
    }
    None
}

fn mismatch(
    message: String,
    primary_span: Span,
    primary: impl Into<String>,
    secondary_span: Span,
    secondary: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(INVALID_OVERRIDE_SIGNATURE, message)
        .with_primary_label(primary_span, primary)
        .with_secondary_label(secondary_span, secondary)
        .with_note("override signatures must match exactly; parameter names may differ")
}

fn parameter_mode(parameter: &ResolvedParameter) -> &'static str {
    match parameter.binding_mode {
        ResolvedParameterBindingMode::Value => "a value",
        ResolvedParameterBindingMode::ReadOnlyAlias { .. } => "a read-only alias",
        ResolvedParameterBindingMode::MutableAlias { .. } => "a mutable alias",
    }
}

const fn receiver_name(access: ResolvedReceiverAccess) -> &'static str {
    match access {
        ResolvedReceiverAccess::ReadOnly => "read-only",
        ResolvedReceiverAccess::Mutable => "mutable",
    }
}
