//! Canonical `std::range::Successor<Output>` identity validation.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    module::{ModulePath, ProgramModuleTable},
    resolve::{
        ResolvedInterfaceTemplateSemanticTable, ResolvedInterfaceTemplateTable,
        ResolvedModuleDeclarationTable, ResolvedRangeLanguageItem, ResolvedTemplateTypeKind,
        ResolvedTopLevelId, ResolvedTypeParameterTable, ResolvedVisibility,
    },
    source::Span,
};

use super::super::INVALID_RANGE_LANGUAGE_ITEM;

pub(super) struct RangeLanguageItemEvidence<'a> {
    pub requiring_spans: &'a [Span],
    pub successor_declaration_spans: &'a [Span],
}

pub(super) fn validate_range_language_item(
    modules: &ProgramModuleTable,
    module_declarations: &ResolvedModuleDeclarationTable,
    templates: &ResolvedInterfaceTemplateTable,
    semantics: &ResolvedInterfaceTemplateSemanticTable,
    type_parameters: &ResolvedTypeParameterTable,
    evidence: RangeLanguageItemEvidence<'_>,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedRangeLanguageItem> {
    let requirement_span = *evidence.requiring_spans.first()?;
    let path = ModulePath::try_from("std::range").expect("canonical range module path is valid");
    let module = modules
        .find(&path)
        .expect("ordinary reachability evidence must load the canonical range module")
        .module_id();
    let declarations = module_declarations
        .get(module)
        .expect("every loaded module has a declaration table");
    let Some(declaration) = declarations.get("Successor") else {
        diagnostics.push(
            Diagnostic::error(
                INVALID_RANGE_LANGUAGE_ITEM,
                "`std::range` does not declare the required `Successor` interface",
            )
            .with_primary_label(requirement_span, "range protocol required here"),
        );
        return None;
    };
    if evidence.successor_declaration_spans.len() > 1 {
        report(
            diagnostics,
            requirement_span,
            "`std::range` must declare `Successor` exactly once",
            evidence.successor_declaration_spans[1],
            "duplicate language-item declaration",
        );
        return None;
    }
    let ResolvedTopLevelId::InterfaceTemplate(template_id) = declaration.declaration else {
        report(
            diagnostics,
            requirement_span,
            "`std::range::Successor` must be a generic interface",
            declaration.name_span,
            "declared with the wrong kind",
        );
        return None;
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
    let mut valid = semantic.valid;

    if template.visibility != ResolvedVisibility::Public {
        report(
            diagnostics,
            requirement_span,
            "`std::range::Successor` must be public",
            template.name_span,
            "private language-item declaration",
        );
        valid = false;
    }
    if parameters.len() != 1 {
        report(
            diagnostics,
            requirement_span,
            "`std::range::Successor` must declare exactly one type parameter",
            template.name_span,
            format!("found {} type parameters", parameters.len()),
        );
        valid = false;
    }
    if !semantic.bounds.is_empty() {
        report(
            diagnostics,
            requirement_span,
            "`std::range::Successor` must not declare generic bounds",
            semantic.bounds[0].span,
            "remove this bound",
        );
        valid = false;
    }
    if semantic.requirements.len() != 1 {
        report(
            diagnostics,
            requirement_span,
            "`std::range::Successor` must declare exactly one requirement",
            template.name_span,
            format!("found {} requirements", semantic.requirements.len()),
        );
        valid = false;
    }

    let output = parameters.iter().next();
    if let Some(output) = output {
        if output.name != "Output" {
            report(
                diagnostics,
                requirement_span,
                "the `std::range::Successor` parameter must be named `Output`",
                output.name_span,
                format!("found `{}`", output.name),
            );
            valid = false;
        }
    }

    let requirement = semantic.requirements.first();
    if let (Some(output), Some(requirement)) = (output, requirement) {
        if requirement.name != "successor" {
            report(
                diagnostics,
                requirement_span,
                "the `std::range::Successor` requirement must be named `successor`",
                requirement.name_span,
                format!("found `{}`", requirement.name),
            );
            valid = false;
        }
        if requirement.mutable {
            report(
                diagnostics,
                requirement_span,
                "`std::range::Successor.successor` must have a read-only receiver",
                requirement.name_span,
                "remove `mut` from this requirement",
            );
            valid = false;
        }
        if !requirement.parameters.is_empty() {
            report(
                diagnostics,
                requirement_span,
                "`std::range::Successor.successor` must declare no parameters",
                requirement.parameters[0].span,
                "unexpected parameter",
            );
            valid = false;
        }
        if !matches!(
            requirement.return_type.kind,
            ResolvedTemplateTypeKind::Parameter(actual) if actual == output.id
        ) {
            report(
                diagnostics,
                requirement_span,
                "`std::range::Successor.successor` must return `Output`",
                requirement.return_type.span,
                "result has the wrong type",
            );
            valid = false;
        }

        return valid.then(|| ResolvedRangeLanguageItem {
            successor_template: template_id,
            successor_output_parameter: output.id,
            successor_requirement: requirement.id,
            successor_declaration_span: template.span,
            requiring_spans: evidence.requiring_spans.to_vec(),
        });
    }

    None
}

fn report(
    diagnostics: &mut Diagnostics,
    requirement_span: Span,
    message: impl Into<String>,
    primary_span: Span,
    primary_label: impl Into<String>,
) {
    diagnostics.push(
        Diagnostic::error(INVALID_RANGE_LANGUAGE_ITEM, message)
            .with_primary_label(primary_span, primary_label)
            .with_secondary_label(requirement_span, "range protocol required here"),
    );
}
