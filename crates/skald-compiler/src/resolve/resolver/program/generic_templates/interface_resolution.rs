//! Definition-site semantic resolution for generic interfaces.

use std::collections::HashMap;

use super::interface_validation::validate_interface_signature_type;
use super::requirements::{infer_type_construction, push};
use super::*;
use crate::identity::InterfaceTemplateRequirementId;

pub(crate) fn resolve_interface_template_semantics(
    template: InterfaceTemplateId,
    interface: &syntax::InterfaceDecl,
    parameters: &ResolvedTypeParameters,
    lookup: ModuleLookup<'_>,
    diagnostics: &mut Diagnostics,
) -> ResolvedInterfaceTemplateSemantics {
    let mut type_uses = Vec::new();
    let bounds =
        resolve_interface_bounds(interface, parameters, lookup, diagnostics, &mut type_uses);
    let mut names = HashMap::new();
    let mut requirements = Vec::with_capacity(interface.requirements.len());
    let mut contextual_requirements = Vec::new();

    for (index, declaration) in interface.requirements.iter().enumerate() {
        let id = InterfaceTemplateRequirementId::new(template, index);
        if let Some(previous) = names.insert(declaration.name.text.as_str(), declaration.name.span)
        {
            diagnostics.push(
                Diagnostic::error(
                    super::super::super::INVALID_GENERIC_INTERFACE_REQUIREMENT,
                    format!(
                        "duplicate requirement `{}` in generic interface",
                        declaration.name.text
                    ),
                )
                .with_primary_label(declaration.name.span, "redeclared here")
                .with_secondary_label(previous, "first declared here"),
            );
        }

        let mut parameter_names = HashMap::new();
        let mut resolved_parameters = Vec::with_capacity(declaration.parameters.len());
        for (parameter_index, parameter) in declaration.parameters.iter().enumerate() {
            if let Some(previous) =
                parameter_names.insert(parameter.name.text.as_str(), parameter.name.span)
            {
                diagnostics.push(
                    Diagnostic::error(
                        super::super::super::INVALID_GENERIC_INTERFACE_REQUIREMENT,
                        format!(
                            "duplicate parameter `{}` in generic interface requirement `{}`",
                            parameter.name.text, declaration.name.text
                        ),
                    )
                    .with_primary_label(parameter.name.span, "redeclared here")
                    .with_secondary_label(previous, "first declared here"),
                );
            }
            let (term, valid) =
                resolve_or_placeholder(&parameter.type_syntax, parameters, lookup, diagnostics);
            let context = ResolvedInterfaceTemplateTypeUseContext::RequirementParameter {
                requirement: id,
                parameter: parameter_index,
            };
            record_type_use(
                context,
                &term,
                Some(parameter.binding_mode),
                valid,
                diagnostics,
                &mut type_uses,
                &mut contextual_requirements,
            );
            resolved_parameters.push(ResolvedInterfaceTemplateParameter {
                binding_mode: resolve_parameter_binding_mode(parameter.binding_mode),
                name: parameter.name.text.to_string(),
                name_span: parameter.name.span,
                type_syntax: term,
                span: parameter.span,
            });
        }

        let (return_type, valid) =
            resolve_or_placeholder(&declaration.return_type, parameters, lookup, diagnostics);
        record_type_use(
            ResolvedInterfaceTemplateTypeUseContext::RequirementResult { requirement: id },
            &return_type,
            None,
            valid,
            diagnostics,
            &mut type_uses,
            &mut contextual_requirements,
        );
        requirements.push(ResolvedInterfaceTemplateRequirementSignature {
            id,
            name: declaration.name.text.to_string(),
            name_span: declaration.name.span,
            mutable: declaration.mut_span.is_some(),
            parameters: resolved_parameters,
            return_type,
            span: declaration.span,
        });
    }

    ResolvedInterfaceTemplateSemantics {
        template,
        bounds,
        requirements,
        type_uses,
        contextual_requirements,
    }
}

fn resolve_interface_bounds(
    interface: &syntax::InterfaceDecl,
    parameters: &ResolvedTypeParameters,
    lookup: ModuleLookup<'_>,
    diagnostics: &mut Diagnostics,
    type_uses: &mut Vec<ResolvedInterfaceTemplateTypeUse>,
) -> Vec<ResolvedInterfaceTemplateBound> {
    let mut bounds = Vec::new();
    let Some(clause) = &interface.where_clause else {
        return bounds;
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
                    "bounds must name a parameter of this interface template",
                ),
            );
            continue;
        };
        let syntax = syntax::TypeSyntax {
            kind: syntax::TypeKind::Named(requirement.interface.clone()),
            span: requirement.interface.span,
        };
        let Some(term) =
            TemplateTypeResolver::for_interface_template(parameters, lookup, diagnostics)
                .resolve(&syntax)
        else {
            continue;
        };
        if !matches!(
            term.kind,
            ResolvedTemplateTypeKind::Interface(_)
                | ResolvedTemplateTypeKind::InterfaceTemplate { .. }
        ) {
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
        }
        if bounds.iter().any(|bound: &ResolvedInterfaceTemplateBound| {
            bound.parameter == parameter.id && same_type(&bound.interface, &term)
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
        let bound = bounds.len();
        type_uses.push(ResolvedInterfaceTemplateTypeUse {
            context: ResolvedInterfaceTemplateTypeUseContext::Bound { bound },
            type_term: term.clone(),
        });
        bounds.push(ResolvedInterfaceTemplateBound {
            parameter: parameter.id,
            interface: term,
            parameter_span: requirement.parameter.span,
            span: requirement.span,
        });
    }
    bounds
}

fn resolve_or_placeholder(
    syntax: &syntax::TypeSyntax,
    parameters: &ResolvedTypeParameters,
    lookup: ModuleLookup<'_>,
    diagnostics: &mut Diagnostics,
) -> (ResolvedTemplateType, bool) {
    match TemplateTypeResolver::for_interface_template(parameters, lookup, diagnostics)
        .resolve(syntax)
    {
        Some(term) => (term, true),
        None => (
            ResolvedTemplateType {
                kind: ResolvedTemplateTypeKind::Unit,
                span: syntax.span,
            },
            false,
        ),
    }
}

fn record_type_use(
    context: ResolvedInterfaceTemplateTypeUseContext,
    term: &ResolvedTemplateType,
    parameter_mode: Option<syntax::ParameterBindingMode>,
    valid: bool,
    diagnostics: &mut Diagnostics,
    type_uses: &mut Vec<ResolvedInterfaceTemplateTypeUse>,
    requirements: &mut Vec<GenericRequirement>,
) {
    type_uses.push(ResolvedInterfaceTemplateTypeUse {
        context,
        type_term: term.clone(),
    });
    if !valid {
        return;
    }
    infer_type_construction(term, requirements);
    let (capability, reason) = match (context, parameter_mode) {
        (
            ResolvedInterfaceTemplateTypeUseContext::RequirementParameter {
                requirement,
                parameter,
            },
            Some(mode),
        ) => (
            match mode {
                syntax::ParameterBindingMode::Value => GenericCapability::ValueParameter,
                syntax::ParameterBindingMode::ReadOnlyAlias { .. } => {
                    GenericCapability::AliasTarget(GenericAliasAccess::ReadOnly)
                }
                syntax::ParameterBindingMode::MutableAlias { .. } => {
                    GenericCapability::AliasTarget(GenericAliasAccess::Mutable)
                }
            },
            GenericRequirementReason::InterfaceParameter {
                requirement,
                parameter,
            },
        ),
        (ResolvedInterfaceTemplateTypeUseContext::RequirementResult { requirement }, None) => (
            GenericCapability::ValueResult,
            GenericRequirementReason::InterfaceResult { requirement },
        ),
        (ResolvedInterfaceTemplateTypeUseContext::Bound { .. }, None) => return,
        _ => unreachable!("interface type-use context and parameter mode agree"),
    };
    if term.depends_on_parameter() {
        push(requirements, term, capability, term.span, reason);
    }
    validate_interface_signature_type(term, capability, diagnostics);
}

fn same_type(left: &ResolvedTemplateType, right: &ResolvedTemplateType) -> bool {
    match (&left.kind, &right.kind) {
        (ResolvedTemplateTypeKind::I64, ResolvedTemplateTypeKind::I64)
        | (ResolvedTemplateTypeKind::U64, ResolvedTemplateTypeKind::U64)
        | (ResolvedTemplateTypeKind::U8, ResolvedTemplateTypeKind::U8)
        | (ResolvedTemplateTypeKind::F64, ResolvedTemplateTypeKind::F64)
        | (ResolvedTemplateTypeKind::Bool, ResolvedTemplateTypeKind::Bool)
        | (ResolvedTemplateTypeKind::Unit, ResolvedTemplateTypeKind::Unit)
        | (ResolvedTemplateTypeKind::Obj, ResolvedTemplateTypeKind::Obj) => true,
        (ResolvedTemplateTypeKind::Parameter(a), ResolvedTemplateTypeKind::Parameter(b)) => a == b,
        (ResolvedTemplateTypeKind::Class(a), ResolvedTemplateTypeKind::Class(b)) => a == b,
        (ResolvedTemplateTypeKind::Interface(a), ResolvedTemplateTypeKind::Interface(b)) => a == b,
        (
            ResolvedTemplateTypeKind::ClassTemplate {
                template: a,
                arguments: aa,
            },
            ResolvedTemplateTypeKind::ClassTemplate {
                template: b,
                arguments: ba,
            },
        ) => a == b && aa.len() == ba.len() && aa.iter().zip(ba).all(|(a, b)| same_type(a, b)),
        (
            ResolvedTemplateTypeKind::InterfaceTemplate {
                template: a,
                arguments: aa,
            },
            ResolvedTemplateTypeKind::InterfaceTemplate {
                template: b,
                arguments: ba,
            },
        ) => a == b && aa.len() == ba.len() && aa.iter().zip(ba).all(|(a, b)| same_type(a, b)),
        (ResolvedTemplateTypeKind::Shared(a), ResolvedTemplateTypeKind::Shared(b))
        | (ResolvedTemplateTypeKind::Optional(a), ResolvedTemplateTypeKind::Optional(b))
        | (ResolvedTemplateTypeKind::Array(a), ResolvedTemplateTypeKind::Array(b)) => {
            same_type(a, b)
        }
        (
            ResolvedTemplateTypeKind::Function {
                parameters: ap,
                result: ar,
            },
            ResolvedTemplateTypeKind::Function {
                parameters: bp,
                result: br,
            },
        ) => {
            ap.len() == bp.len()
                && ap
                    .iter()
                    .zip(bp)
                    .all(|(a, b)| a.mode == b.mode && same_type(&a.type_syntax, &b.type_syntax))
                && same_type(ar, br)
        }
        _ => false,
    }
}
