//! Nominal interface-bound and implemented-interface resolution.

use super::*;

pub(super) fn resolve_bounds(
    class: &syntax::ClassDecl,
    parameters: &ResolvedTypeParameters,
    lookup: ModuleLookup<'_>,
    diagnostics: &mut Diagnostics,
) -> Vec<ResolvedTemplateBound> {
    let mut resolved = Vec::new();
    let Some(clause) = &class.where_clause else {
        return resolved;
    };

    for requirement in &clause.requirements {
        let Some(parameter) = (!requirement.parameter.is_qualified())
            .then(|| parameters.get(requirement.parameter.text.as_str()))
            .flatten()
        else {
            diagnostics.push(
                Diagnostic::error(
                    super::super::super::INVALID_GENERIC_BOUND,
                    format!(
                        "unknown type parameter `{}` in generic bound",
                        requirement.parameter.text
                    ),
                )
                .with_primary_label(
                    requirement.parameter.span,
                    "bounds must name a parameter of this class template",
                ),
            );
            continue;
        };
        let syntax = syntax::TypeSyntax {
            kind: syntax::TypeKind::Named(requirement.interface.clone()),
            span: requirement.interface.span,
        };
        let Some(term) =
            TemplateTypeResolver::new(parameters, lookup, diagnostics).resolve(&syntax)
        else {
            continue;
        };
        let Some(interface) = ResolvedInterfaceType::from_type(&term) else {
            diagnostics.push(
                Diagnostic::error(
                    super::super::super::INVALID_GENERIC_BOUND,
                    format!(
                        "`{}` does not name an interface",
                        requirement.interface.text
                    ),
                )
                .with_primary_label(requirement.interface.span, "expected an interface type"),
            );
            continue;
        };
        if resolved.iter().any(|bound: &ResolvedTemplateBound| {
            bound.parameter == parameter.id && bound.interface.semantically_eq(&interface)
        }) {
            diagnostics.push(
                Diagnostic::error(
                    super::super::super::DUPLICATE_GENERIC_BOUND,
                    format!(
                        "duplicate bound `{}: {}`",
                        requirement.parameter.text, requirement.interface.text
                    ),
                )
                .with_primary_label(requirement.span, "repeated here"),
            );
            continue;
        }
        resolved.push(ResolvedTemplateBound {
            parameter: parameter.id,
            interface,
            parameter_span: requirement.parameter.span,
            interface_span: requirement.interface.span,
            span: requirement.span,
        });
    }
    resolved
}

pub(super) fn resolve_implemented_interfaces(
    class: &syntax::ClassDecl,
    parameters: &ResolvedTypeParameters,
    lookup: ModuleLookup<'_>,
    diagnostics: &mut Diagnostics,
) -> Vec<ResolvedInterfaceClaim> {
    let mut interfaces = Vec::new();
    for claim in &class.implemented_interfaces {
        let syntax = syntax::TypeSyntax {
            kind: syntax::TypeKind::Named(claim.clone()),
            span: claim.span,
        };
        let Some(term) =
            TemplateTypeResolver::new(parameters, lookup, diagnostics).resolve(&syntax)
        else {
            continue;
        };
        let Some(interface) = ResolvedInterfaceType::from_type(&term) else {
            diagnostics.push(
                Diagnostic::error(
                    super::super::super::INVALID_INTERFACE_CLAIM,
                    format!("`{}` does not name an interface", claim.text),
                )
                .with_primary_label(claim.span, "expected an interface type"),
            );
            continue;
        };
        if interfaces
            .iter()
            .any(|existing: &ResolvedInterfaceClaim| existing.interface.semantically_eq(&interface))
        {
            diagnostics.push(
                Diagnostic::error(
                    super::super::super::INVALID_INTERFACE_CLAIM,
                    format!("duplicate interface `{}`", claim.text),
                )
                .with_primary_label(claim.span, "repeated here"),
            );
            continue;
        }
        interfaces.push(ResolvedInterfaceClaim {
            interface,
            span: claim.span,
        });
    }
    interfaces
}
