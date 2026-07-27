//! Interface declaration collection and class conformance-name resolution.

use super::*;
use std::collections::HashSet;

pub(super) fn collect_interface_declarations(
    ast: &syntax::CompilationUnit,
    module: ModuleId,
    work: &[(InterfaceId, usize)],
    top_levels: &HashMap<String, TopLevelSymbol>,
    array_types: &mut ArrayTypeInterner,
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
                            resolve_type(
                                &parameter.type_syntax,
                                top_levels,
                                array_types,
                                diagnostics,
                            )
                            .map(|type_syntax| {
                                ResolvedInterfaceParameter {
                                    binding_mode: resolve_parameter_binding_mode(
                                        parameter.binding_mode,
                                    ),
                                    name: parameter.name.text.to_string(),
                                    name_span: parameter.name.span,
                                    type_syntax,
                                    span: parameter.span,
                                }
                            })
                        })
                        .collect(),
                    return_type: resolve_result_type(
                        &requirement.return_type,
                        top_levels,
                        array_types,
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
    top_levels: &HashMap<String, TopLevelSymbol>,
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
        let mut seen = HashSet::new();
        for claim in &syntax_class.implemented_interfaces {
            if reject_qualified_name(claim, diagnostics) {
                continue;
            }
            match top_levels.get(claim.text.as_str()) {
                Some(TopLevelSymbol {
                    kind: TopLevelSymbolKind::Interface(interface),
                    ..
                }) if seen.insert(*interface) => {
                    class.implemented_interfaces.push(ResolvedInterfaceClaim {
                        interface: *interface,
                        span: claim.span,
                    });
                }
                Some(TopLevelSymbol {
                    kind: TopLevelSymbolKind::Interface(_),
                    name_span,
                }) => {
                    diagnostics.push(
                        Diagnostic::error(
                            INVALID_INTERFACE_CLAIM,
                            format!("duplicate interface `{}`", claim.text),
                        )
                        .with_primary_label(claim.span, "repeated here")
                        .with_secondary_label(*name_span, "interface declared here"),
                    );
                }
                Some(symbol) => diagnostics.push(
                    Diagnostic::error(
                        INVALID_INTERFACE_CLAIM,
                        format!("`{}` does not name an interface", claim.text),
                    )
                    .with_primary_label(claim.span, "expected an interface name")
                    .with_secondary_label(symbol.name_span, "different declaration kind here"),
                ),
                None => diagnostics.push(
                    Diagnostic::error(
                        INVALID_INTERFACE_CLAIM,
                        format!("unknown interface `{}`", claim.text),
                    )
                    .with_primary_label(claim.span, "no interface with this name is declared"),
                ),
            }
        }
    }
}
