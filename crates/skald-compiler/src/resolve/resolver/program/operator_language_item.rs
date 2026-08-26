//! Canonical `std::ops` bundle validation.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    module::{ModulePath, ProgramModuleTable},
    resolve::{
        CanonicalOperatorProtocol, CanonicalOperatorProtocolShape,
        ResolvedInterfaceTemplateRequirementSignature, ResolvedInterfaceTemplateSemanticTable,
        ResolvedInterfaceTemplateTable, ResolvedModuleDeclarationTable,
        ResolvedOperatorLanguageItem, ResolvedOperatorProtocol, ResolvedOperatorProtocolParameters,
        ResolvedParameterBindingMode, ResolvedTemplateType, ResolvedTemplateTypeKind,
        ResolvedTopLevelId, ResolvedTypeParameterTable, ResolvedVisibility,
    },
    source::Span,
};

use super::super::INVALID_OPERATOR_LANGUAGE_ITEM;

pub(super) struct OperatorLanguageItemEvidence<'a> {
    pub requiring_spans: &'a [Span],
    pub declaration_spans: &'a [(String, Span)],
}

pub(super) fn validate_operator_language_item(
    modules: &ProgramModuleTable,
    module_declarations: &ResolvedModuleDeclarationTable,
    templates: &ResolvedInterfaceTemplateTable,
    semantics: &ResolvedInterfaceTemplateSemanticTable,
    type_parameters: &ResolvedTypeParameterTable,
    evidence: OperatorLanguageItemEvidence<'_>,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedOperatorLanguageItem> {
    let requirement_span = *evidence.requiring_spans.first()?;
    let path = ModulePath::try_from("std::ops").expect("canonical operator module path is valid");
    let module = modules
        .find(&path)
        .expect("ordinary reachability evidence must load the canonical operator module")
        .module_id();
    let declarations = module_declarations
        .get(module)
        .expect("every loaded module has a declaration table");
    let mut protocols = Vec::with_capacity(CanonicalOperatorProtocol::COUNT);

    for kind in CanonicalOperatorProtocol::ALL {
        let name = kind.interface_name();
        let spans = evidence
            .declaration_spans
            .iter()
            .filter(|(candidate, _)| candidate == name)
            .map(|(_, span)| *span)
            .collect::<Vec<_>>();
        let Some(declaration) = declarations.get(name) else {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_OPERATOR_LANGUAGE_ITEM,
                    format!("`std::ops` does not declare the required `{name}` interface"),
                )
                .with_primary_label(requirement_span, "operator protocol bundle required here"),
            );
            continue;
        };
        if spans.len() > 1 {
            report(
                diagnostics,
                requirement_span,
                format!("`std::ops` must declare `{name}` exactly once"),
                spans[1],
                "duplicate operator protocol declaration",
            );
            continue;
        }
        let ResolvedTopLevelId::InterfaceTemplate(template_id) = declaration.declaration else {
            report(
                diagnostics,
                requirement_span,
                format!("`std::ops::{name}` must be a generic interface"),
                declaration.name_span,
                "declared with the wrong kind",
            );
            continue;
        };
        let template = templates
            .get(template_id)
            .expect("interface-template declaration identity must exist");
        let semantic = semantics
            .get(template_id)
            .expect("every interface template has semantic metadata");
        let parameters = type_parameters
            .for_interface_template(template_id)
            .expect("every interface template has one parameter list");

        let source = ProtocolSource {
            template_id,
            visibility: template.visibility,
            name_span: template.name_span,
            declaration_span: template.span,
            parameters,
            semantic,
        };
        if let Some(protocol) = validate_protocol(kind, source, requirement_span, diagnostics) {
            protocols.push(protocol);
        }
    }

    (protocols.len() == CanonicalOperatorProtocol::COUNT)
        .then(|| ResolvedOperatorLanguageItem::new(protocols, evidence.requiring_spans.to_vec()))
}

#[derive(Clone, Copy)]
struct ProtocolSource<'a> {
    template_id: crate::identity::InterfaceTemplateId,
    visibility: ResolvedVisibility,
    name_span: Span,
    declaration_span: Span,
    parameters: &'a crate::resolve::ResolvedTypeParameters,
    semantic: &'a crate::resolve::ResolvedInterfaceTemplateSemantics,
}

fn validate_protocol(
    kind: CanonicalOperatorProtocol,
    source: ProtocolSource<'_>,
    requirement_span: Span,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedOperatorProtocol> {
    let interface_name = kind.interface_name();
    let mut valid = source.semantic.valid;
    if source.visibility != ResolvedVisibility::Public {
        report(
            diagnostics,
            requirement_span,
            format!("`std::ops::{interface_name}` must be public"),
            source.name_span,
            "private operator protocol declaration",
        );
        valid = false;
    }
    if !source.semantic.bounds.is_empty() {
        report(
            diagnostics,
            requirement_span,
            format!("`std::ops::{interface_name}` must not declare generic bounds"),
            source.semantic.bounds[0].span,
            "remove this bound",
        );
        valid = false;
    }

    let expected_parameter_names: &[&str] = match kind.shape() {
        CanonicalOperatorProtocolShape::Unary => &["Output"],
        CanonicalOperatorProtocolShape::Predicate => &["Rhs"],
        CanonicalOperatorProtocolShape::Binary => &["Rhs", "Output"],
    };
    if source.parameters.len() != expected_parameter_names.len() {
        report(
            diagnostics,
            requirement_span,
            format!(
                "`std::ops::{interface_name}` must declare exactly {} type parameter{}",
                expected_parameter_names.len(),
                if expected_parameter_names.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ),
            source.name_span,
            format!("found {} type parameters", source.parameters.len()),
        );
        valid = false;
    }
    for (index, (parameter, expected)) in source
        .parameters
        .iter()
        .zip(expected_parameter_names)
        .enumerate()
    {
        if parameter.name != *expected {
            report(
                diagnostics,
                requirement_span,
                format!(
                    "the {} `std::ops::{interface_name}` parameter must be named `{expected}`",
                    ordinal(index)
                ),
                parameter.name_span,
                format!("found `{}`", parameter.name),
            );
            valid = false;
        }
    }

    if source.semantic.requirements.len() != 1 {
        report(
            diagnostics,
            requirement_span,
            format!("`std::ops::{interface_name}` must declare exactly one requirement"),
            source.name_span,
            format!("found {} requirements", source.semantic.requirements.len()),
        );
        valid = false;
    }

    let identities = protocol_parameters(kind.shape(), source.parameters);
    let requirement = source.semantic.requirements.first();
    if let (Some(identities), Some(requirement)) = (identities, requirement) {
        valid &= validate_requirement(kind, requirement, identities, requirement_span, diagnostics);
        return valid.then_some(ResolvedOperatorProtocol {
            kind,
            template: source.template_id,
            parameters: identities,
            requirement: requirement.id,
            declaration_span: source.declaration_span,
        });
    }

    None
}

fn protocol_parameters(
    shape: CanonicalOperatorProtocolShape,
    parameters: &crate::resolve::ResolvedTypeParameters,
) -> Option<ResolvedOperatorProtocolParameters> {
    let mut parameters = parameters.iter();
    match shape {
        CanonicalOperatorProtocolShape::Unary => Some(ResolvedOperatorProtocolParameters::Unary {
            output: parameters.next()?.id,
        }),
        CanonicalOperatorProtocolShape::Predicate => {
            Some(ResolvedOperatorProtocolParameters::Predicate {
                rhs: parameters.next()?.id,
            })
        }
        CanonicalOperatorProtocolShape::Binary => {
            Some(ResolvedOperatorProtocolParameters::Binary {
                rhs: parameters.next()?.id,
                output: parameters.next()?.id,
            })
        }
    }
}

fn validate_requirement(
    kind: CanonicalOperatorProtocol,
    requirement: &ResolvedInterfaceTemplateRequirementSignature,
    identities: ResolvedOperatorProtocolParameters,
    requirement_span: Span,
    diagnostics: &mut Diagnostics,
) -> bool {
    let interface_name = kind.interface_name();
    let requirement_name = kind.requirement_name();
    let mut valid = true;
    if requirement.name != requirement_name {
        report(
            diagnostics,
            requirement_span,
            format!(
                "the `std::ops::{interface_name}` requirement must be named `{requirement_name}`"
            ),
            requirement.name_span,
            format!("found `{}`", requirement.name),
        );
        valid = false;
    }
    if requirement.mutable {
        report(
            diagnostics,
            requirement_span,
            format!(
                "`std::ops::{interface_name}.{requirement_name}` must have a read-only receiver"
            ),
            requirement.name_span,
            "remove `mut` from this requirement",
        );
        valid = false;
    }

    let rhs = match identities {
        ResolvedOperatorProtocolParameters::Unary { .. } => None,
        ResolvedOperatorProtocolParameters::Predicate { rhs }
        | ResolvedOperatorProtocolParameters::Binary { rhs, .. } => Some(rhs),
    };
    let expected_parameter_count = usize::from(rhs.is_some());
    if requirement.parameters.len() != expected_parameter_count {
        report(
            diagnostics,
            requirement_span,
            format!(
                "`std::ops::{interface_name}.{requirement_name}` must declare {} parameter{}",
                if expected_parameter_count == 0 {
                    "no"
                } else {
                    "exactly one"
                },
                if expected_parameter_count == 1 {
                    ""
                } else {
                    "s"
                }
            ),
            requirement.name_span,
            format!("found {} parameters", requirement.parameters.len()),
        );
        valid = false;
    }
    if let (Some(rhs), Some(parameter)) = (rhs, requirement.parameters.first()) {
        if parameter.name != "rhs" {
            report(
                diagnostics,
                requirement_span,
                format!(
                    "the `std::ops::{interface_name}.{requirement_name}` parameter must be named `rhs`"
                ),
                parameter.name_span,
                format!("found `{}`", parameter.name),
            );
            valid = false;
        }
        if !matches!(
            parameter.binding_mode,
            ResolvedParameterBindingMode::ReadOnlyAlias { .. }
        ) {
            report(
                diagnostics,
                requirement_span,
                format!(
                    "the `std::ops::{interface_name}.{requirement_name}` parameter must use `ref`"
                ),
                parameter.span,
                "parameter has the wrong binding mode",
            );
            valid = false;
        }
        if !is_parameter(&parameter.type_syntax, rhs) {
            report(
                diagnostics,
                requirement_span,
                format!(
                    "the `std::ops::{interface_name}.{requirement_name}` parameter must have type `Rhs`"
                ),
                parameter.type_syntax.span,
                "parameter has the wrong type",
            );
            valid = false;
        }
    }

    let valid_result = match identities {
        ResolvedOperatorProtocolParameters::Unary { output }
        | ResolvedOperatorProtocolParameters::Binary { output, .. } => {
            is_parameter(&requirement.return_type, output)
        }
        ResolvedOperatorProtocolParameters::Predicate { .. } => {
            matches!(requirement.return_type.kind, ResolvedTemplateTypeKind::Bool)
        }
    };
    if !valid_result {
        let expected = match identities {
            ResolvedOperatorProtocolParameters::Unary { .. }
            | ResolvedOperatorProtocolParameters::Binary { .. } => "`Output`",
            ResolvedOperatorProtocolParameters::Predicate { .. } => "`bool`",
        };
        report(
            diagnostics,
            requirement_span,
            format!("`std::ops::{interface_name}.{requirement_name}` must return {expected}"),
            requirement.return_type.span,
            "result has the wrong type",
        );
        valid = false;
    }
    valid
}

fn is_parameter(term: &ResolvedTemplateType, expected: crate::identity::TypeParameterId) -> bool {
    matches!(term.kind, ResolvedTemplateTypeKind::Parameter(actual) if actual == expected)
}

const fn ordinal(index: usize) -> &'static str {
    match index {
        0 => "first",
        1 => "second",
        _ => "later",
    }
}

fn report(
    diagnostics: &mut Diagnostics,
    requirement_span: Span,
    message: impl Into<String>,
    primary_span: Span,
    primary_label: impl Into<String>,
) {
    diagnostics.push(
        Diagnostic::error(INVALID_OPERATOR_LANGUAGE_ITEM, message)
            .with_primary_label(primary_span, primary_label)
            .with_secondary_label(requirement_span, "operator protocol bundle required here"),
    );
}
