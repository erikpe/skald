//! Interface declaration collection and class conformance-name resolution.

use super::generic_templates::TemplateTypeResolver;
use super::*;

pub(super) fn collect_interface_declarations(
    ast: &syntax::CompilationUnit,
    module: ModuleId,
    work: &[(InterfaceId, usize)],
    lookup: ModuleLookup<'_>,
    type_interner: &mut ResolvedTypeInterner,
    diagnostics: &mut Diagnostics,
) -> Vec<ResolvedInterfaceDeclaration> {
    work.iter()
        .copied()
        .map(|(id, ast_index)| {
            let syntax::TopLevelDeclaration::Interface(interface) = &ast.declarations[ast_index]
            else {
                unreachable!("interface work item must reference an interface")
            };
            let requirements = interface
                .requirements
                .iter()
                .enumerate()
                .map(|(index, requirement)| ResolvedInterfaceRequirement {
                    id: InterfaceRequirementId::new(id, index),
                    name: requirement.name.text.to_string(),
                    name_span: requirement.name.span,
                    mutable: requirement.mut_span.is_some(),
                    parameters: requirement
                        .parameters
                        .iter()
                        .filter_map(|parameter| {
                            resolve_type(&parameter.type_syntax, lookup, type_interner, diagnostics)
                                .map(|type_syntax| ResolvedInterfaceParameter {
                                    binding_mode: resolve_parameter_binding_mode(
                                        parameter.binding_mode,
                                    ),
                                    name: parameter.name.text.to_string(),
                                    name_span: parameter.name.span,
                                    type_syntax,
                                    span: parameter.span,
                                })
                        })
                        .collect(),
                    return_type: resolve_result_type(
                        &requirement.return_type,
                        lookup,
                        type_interner,
                        diagnostics,
                    ),
                    span: requirement.span,
                })
                .collect();
            ResolvedInterfaceDeclaration {
                id,
                module,
                visibility: resolved_visibility(interface.visibility),
                name: interface.name.text.to_string(),
                name_span: interface.name.span,
                requirements,
                span: interface.span,
            }
        })
        .collect()
}

pub(super) fn resolve_interface_claims(
    ast: &syntax::CompilationUnit,
    work: &[(ClassId, usize)],
    lookup: ModuleLookup<'_>,
    classes: &mut ResolvedClassDeclarationTable,
    diagnostics: &mut Diagnostics,
) {
    for (class_id, ast_index) in work.iter().copied() {
        let class = classes
            .get_mut(class_id)
            .expect("class work must reference its declaration");
        let syntax::TopLevelDeclaration::Class(syntax_class) = &ast.declarations[ast_index] else {
            unreachable!("class work item must reference a class")
        };
        let mut seen = Vec::<ResolvedInterfaceType>::new();
        for claim in &syntax_class.implemented_interfaces {
            if claim.arguments.is_some() {
                let syntax = syntax::TypeSyntax {
                    kind: syntax::TypeKind::Named(claim.clone()),
                    span: claim.span,
                };
                let Some(term) = TemplateTypeResolver::for_application_site(lookup, diagnostics)
                    .resolve(&syntax)
                else {
                    continue;
                };
                let Some(interface) = ResolvedInterfaceType::from_type(&term) else {
                    diagnostics.push(
                        Diagnostic::error(
                            INVALID_INTERFACE_CLAIM,
                            format!("`{}` does not name an interface", claim.text),
                        )
                        .with_primary_label(claim.span, "expected an interface application"),
                    );
                    continue;
                };
                if seen
                    .iter()
                    .any(|existing| existing.semantically_eq(&interface))
                {
                    diagnostics.push(
                        Diagnostic::error(
                            INVALID_INTERFACE_CLAIM,
                            format!("duplicate interface `{}`", claim.text),
                        )
                        .with_primary_label(claim.span, "repeated here"),
                    );
                    continue;
                }
                seen.push(interface.clone());
                class.implemented_interfaces.push(ResolvedInterfaceClaim {
                    interface,
                    span: claim.span,
                });
                diagnostics.push(
                    Diagnostic::error(
                        UNSUPPORTED_GENERIC_INTERFACE,
                        format!(
                            "generic interface application `{}` is resolved but not yet specialized",
                            claim.text
                        ),
                    )
                    .with_primary_label(
                        claim.span,
                        "closed interface specialization is implemented by the next roadmap stage",
                    ),
                );
                continue;
            }
            match lookup.select(claim, diagnostics) {
                TopLevelLookup::Found(TopLevelSymbol {
                    kind: TopLevelSymbolKind::Interface(interface),
                    ..
                }) if !seen.contains(&ResolvedInterfaceType::Ordinary(interface)) => {
                    seen.push(ResolvedInterfaceType::Ordinary(interface));
                    class.implemented_interfaces.push(ResolvedInterfaceClaim {
                        interface: ResolvedInterfaceType::Ordinary(interface),
                        span: claim.span,
                    });
                }
                TopLevelLookup::Found(TopLevelSymbol {
                    kind: TopLevelSymbolKind::Interface(_),
                    name_span,
                }) => {
                    diagnostics.push(
                        Diagnostic::error(
                            INVALID_INTERFACE_CLAIM,
                            format!("duplicate interface `{}`", claim.text),
                        )
                        .with_primary_label(claim.span, "repeated here")
                        .with_secondary_label(name_span, "interface declared here"),
                    );
                }
                TopLevelLookup::Found(symbol) => diagnostics.push(
                    Diagnostic::error(
                        INVALID_INTERFACE_CLAIM,
                        format!("`{}` does not name an interface", claim.text),
                    )
                    .with_primary_label(claim.span, "expected an interface name")
                    .with_secondary_label(symbol.name_span, "different declaration kind here"),
                ),
                TopLevelLookup::Missing => diagnostics.push(
                    Diagnostic::error(
                        INVALID_INTERFACE_CLAIM,
                        format!("unknown interface `{}`", claim.text),
                    )
                    .with_primary_label(claim.span, "no interface with this name is declared"),
                ),
                TopLevelLookup::Diagnosed => {}
            }
        }
    }
}
