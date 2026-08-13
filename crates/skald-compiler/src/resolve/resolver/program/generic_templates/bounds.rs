//! Nominal interface-bound and implemented-interface resolution.

use std::collections::HashSet;

use super::*;

pub(super) fn resolve_bounds(
    class: &syntax::ClassDecl,
    parameters: &ResolvedTypeParameters,
    lookup: ModuleLookup<'_>,
    diagnostics: &mut Diagnostics,
) -> Vec<ResolvedTemplateBound> {
    let mut resolved = Vec::new();
    let mut seen = HashSet::new();
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
        match lookup.select(&requirement.interface, diagnostics) {
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Interface(interface),
                ..
            }) if seen.insert((parameter.id, interface)) => {
                resolved.push(ResolvedTemplateBound {
                    parameter: parameter.id,
                    interface,
                    parameter_span: requirement.parameter.span,
                    interface_span: requirement.interface.span,
                    span: requirement.span,
                });
            }
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Interface(_),
                name_span,
            }) => diagnostics.push(
                Diagnostic::error(
                    super::super::super::DUPLICATE_GENERIC_BOUND,
                    format!(
                        "duplicate bound `{}: {}`",
                        requirement.parameter.text, requirement.interface.text
                    ),
                )
                .with_primary_label(requirement.span, "repeated here")
                .with_secondary_label(name_span, "interface declared here"),
            ),
            TopLevelLookup::Found(symbol) => diagnostics.push(
                Diagnostic::error(
                    super::super::super::INVALID_GENERIC_BOUND,
                    format!(
                        "`{}` does not name an interface",
                        requirement.interface.text
                    ),
                )
                .with_primary_label(requirement.interface.span, "expected an interface name")
                .with_secondary_label(symbol.name_span, "different declaration kind here"),
            ),
            TopLevelLookup::Missing => diagnostics.push(
                Diagnostic::error(
                    super::super::super::INVALID_GENERIC_BOUND,
                    format!("unknown interface `{}`", requirement.interface.text),
                )
                .with_primary_label(
                    requirement.interface.span,
                    "no interface with this name is visible in the template's module",
                ),
            ),
            TopLevelLookup::Diagnosed => {}
        }
    }
    resolved
}

pub(super) fn resolve_implemented_interfaces(
    class: &syntax::ClassDecl,
    lookup: ModuleLookup<'_>,
    diagnostics: &mut Diagnostics,
) -> Vec<ResolvedInterfaceClaim> {
    let mut interfaces = Vec::new();
    let mut seen = HashSet::new();
    for claim in &class.implemented_interfaces {
        match lookup.select(claim, diagnostics) {
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Interface(interface),
                ..
            }) if seen.insert(interface) => interfaces.push(ResolvedInterfaceClaim {
                interface,
                span: claim.span,
            }),
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Interface(_),
                name_span,
            }) => diagnostics.push(
                Diagnostic::error(
                    super::super::super::INVALID_INTERFACE_CLAIM,
                    format!("duplicate interface `{}`", claim.text),
                )
                .with_primary_label(claim.span, "repeated here")
                .with_secondary_label(name_span, "interface declared here"),
            ),
            TopLevelLookup::Found(symbol) => diagnostics.push(
                Diagnostic::error(
                    super::super::super::INVALID_INTERFACE_CLAIM,
                    format!("`{}` does not name an interface", claim.text),
                )
                .with_primary_label(claim.span, "expected an interface name")
                .with_secondary_label(symbol.name_span, "different declaration kind here"),
            ),
            TopLevelLookup::Missing => diagnostics.push(
                Diagnostic::error(
                    super::super::super::INVALID_INTERFACE_CLAIM,
                    format!("unknown interface `{}`", claim.text),
                )
                .with_primary_label(claim.span, "no interface with this name is declared"),
            ),
            TopLevelLookup::Diagnosed => {}
        }
    }
    interfaces
}
