//! Canonical `std::iter::Iterable<Item, State>` identity validation.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    identity::TypeParameterId,
    module::{ModulePath, ProgramModuleTable},
    resolve::{
        ResolvedInterfaceTemplateRequirementSignature, ResolvedInterfaceTemplateSemanticTable,
        ResolvedInterfaceTemplateTable, ResolvedIterableLanguageItem,
        ResolvedModuleDeclarationTable, ResolvedParameterBindingMode, ResolvedTemplateType,
        ResolvedTemplateTypeKind, ResolvedTopLevelId, ResolvedTypeParameterTable,
        ResolvedVisibility,
    },
    source::Span,
};

use super::super::INVALID_ITERABLE_LANGUAGE_ITEM;

pub(super) struct IterableLanguageItemEvidence<'a> {
    pub requiring_spans: &'a [Span],
    pub declaration_spans: &'a [Span],
}

pub(super) fn validate_iterable_language_item(
    modules: &ProgramModuleTable,
    module_declarations: &ResolvedModuleDeclarationTable,
    templates: &ResolvedInterfaceTemplateTable,
    semantics: &ResolvedInterfaceTemplateSemanticTable,
    type_parameters: &ResolvedTypeParameterTable,
    evidence: IterableLanguageItemEvidence<'_>,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedIterableLanguageItem> {
    let requirement_span = *evidence.requiring_spans.first()?;
    let path = ModulePath::try_from("std::iter").expect("canonical iteration module path is valid");
    let module = modules
        .find(&path)
        .expect("iteration dependency evidence must load the canonical module")
        .module_id();
    let declarations = module_declarations
        .get(module)
        .expect("every loaded module has a declaration table");
    let Some(declaration) = declarations.get("Iterable") else {
        diagnostics.push(
            Diagnostic::error(
                INVALID_ITERABLE_LANGUAGE_ITEM,
                "`std::iter` does not declare the required `Iterable` interface",
            )
            .with_primary_label(requirement_span, "iteration language item required here"),
        );
        return None;
    };
    if evidence.declaration_spans.len() > 1 {
        report(
            diagnostics,
            requirement_span,
            "`std::iter` must declare `Iterable` exactly once",
            evidence.declaration_spans[1],
            "duplicate language-item declaration",
        );
        return None;
    }
    let ResolvedTopLevelId::InterfaceTemplate(template_id) = declaration.declaration else {
        report(
            diagnostics,
            requirement_span,
            "`std::iter::Iterable` must be a generic interface",
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
            "`std::iter::Iterable` must be public",
            template.name_span,
            "private language-item declaration",
        );
        valid = false;
    }
    if parameters.len() != 2 {
        report(
            diagnostics,
            requirement_span,
            "`std::iter::Iterable` must declare exactly two type parameters",
            template.name_span,
            format!("found {} type parameters", parameters.len()),
        );
        valid = false;
    }
    if !semantic.bounds.is_empty() {
        report(
            diagnostics,
            requirement_span,
            "`std::iter::Iterable` must not declare generic bounds",
            semantic.bounds[0].span,
            "remove this bound",
        );
        valid = false;
    }
    if semantic.requirements.len() != 2 {
        report(
            diagnostics,
            requirement_span,
            "`std::iter::Iterable` must declare exactly two requirements",
            template.name_span,
            format!("found {} requirements", semantic.requirements.len()),
        );
        valid = false;
    }

    let mut parameter_iter = parameters.iter();
    let item = parameter_iter.next();
    let state = parameter_iter.next();
    if let Some(item) = item {
        if item.name != "Item" {
            report(
                diagnostics,
                requirement_span,
                "the first `std::iter::Iterable` parameter must be named `Item`",
                item.name_span,
                format!("found `{}`", item.name),
            );
            valid = false;
        }
    }
    if let Some(state) = state {
        if state.name != "State" {
            report(
                diagnostics,
                requirement_span,
                "the second `std::iter::Iterable` parameter must be named `State`",
                state.name_span,
                format!("found `{}`", state.name),
            );
            valid = false;
        }
    }

    let iter_state = semantic.requirements.first();
    let iter_next = semantic.requirements.get(1);
    if let (Some(item), Some(state), Some(iter_state), Some(iter_next)) =
        (item, state, iter_state, iter_next)
    {
        valid &= validate_iter_state(iter_state, state.id, requirement_span, diagnostics);
        valid &= validate_iter_next(iter_next, item.id, state.id, requirement_span, diagnostics);

        return valid.then(|| ResolvedIterableLanguageItem {
            template: template_id,
            item_parameter: item.id,
            state_parameter: state.id,
            iter_state_requirement: iter_state.id,
            iter_next_requirement: iter_next.id,
            declaration_span: template.span,
            requiring_spans: evidence.requiring_spans.to_vec(),
        });
    }

    None
}

fn validate_iter_state(
    requirement: &ResolvedInterfaceTemplateRequirementSignature,
    state: TypeParameterId,
    requirement_span: Span,
    diagnostics: &mut Diagnostics,
) -> bool {
    let mut valid = true;
    if requirement.name != "iter_state" {
        report(
            diagnostics,
            requirement_span,
            "the first `std::iter::Iterable` requirement must be named `iter_state`",
            requirement.name_span,
            format!("found `{}`", requirement.name),
        );
        valid = false;
    }
    if requirement.mutable {
        report(
            diagnostics,
            requirement_span,
            "`std::iter::Iterable.iter_state` must have a read-only receiver",
            requirement.name_span,
            "remove `mut` from this requirement",
        );
        valid = false;
    }
    if !requirement.parameters.is_empty() {
        report(
            diagnostics,
            requirement_span,
            "`std::iter::Iterable.iter_state` must declare no parameters",
            requirement.parameters[0].span,
            "unexpected parameter",
        );
        valid = false;
    }
    if !is_parameter(&requirement.return_type, state) {
        report(
            diagnostics,
            requirement_span,
            "`std::iter::Iterable.iter_state` must return `State`",
            requirement.return_type.span,
            "result has the wrong type",
        );
        valid = false;
    }
    valid
}

fn validate_iter_next(
    requirement: &ResolvedInterfaceTemplateRequirementSignature,
    item: TypeParameterId,
    state: TypeParameterId,
    requirement_span: Span,
    diagnostics: &mut Diagnostics,
) -> bool {
    let mut valid = true;
    if requirement.name != "iter_next" {
        report(
            diagnostics,
            requirement_span,
            "the second `std::iter::Iterable` requirement must be named `iter_next`",
            requirement.name_span,
            format!("found `{}`", requirement.name),
        );
        valid = false;
    }
    if requirement.mutable {
        report(
            diagnostics,
            requirement_span,
            "`std::iter::Iterable.iter_next` must have a read-only receiver",
            requirement.name_span,
            "remove `mut` from this requirement",
        );
        valid = false;
    }
    if requirement.parameters.len() != 1 {
        report(
            diagnostics,
            requirement_span,
            "`std::iter::Iterable.iter_next` must declare exactly one parameter",
            requirement.name_span,
            format!("found {} parameters", requirement.parameters.len()),
        );
        valid = false;
    }
    if let Some(parameter) = requirement.parameters.first() {
        if parameter.name != "state" {
            report(
                diagnostics,
                requirement_span,
                "the `std::iter::Iterable.iter_next` parameter must be named `state`",
                parameter.name_span,
                format!("found `{}`", parameter.name),
            );
            valid = false;
        }
        if !matches!(
            parameter.binding_mode,
            ResolvedParameterBindingMode::MutableAlias { .. }
        ) {
            report(
                diagnostics,
                requirement_span,
                "the `std::iter::Iterable.iter_next` parameter must use `mut ref`",
                parameter.span,
                "parameter has the wrong binding mode",
            );
            valid = false;
        }
        if !is_parameter(&parameter.type_syntax, state) {
            report(
                diagnostics,
                requirement_span,
                "the `std::iter::Iterable.iter_next` parameter must have type `State`",
                parameter.type_syntax.span,
                "parameter has the wrong type",
            );
            valid = false;
        }
    }
    if !is_optional_parameter(&requirement.return_type, item) {
        report(
            diagnostics,
            requirement_span,
            "`std::iter::Iterable.iter_next` must return `Item?`",
            requirement.return_type.span,
            "result has the wrong type",
        );
        valid = false;
    }
    valid
}

fn is_parameter(term: &ResolvedTemplateType, expected: TypeParameterId) -> bool {
    matches!(term.kind, ResolvedTemplateTypeKind::Parameter(actual) if actual == expected)
}

fn is_optional_parameter(term: &ResolvedTemplateType, expected: TypeParameterId) -> bool {
    matches!(
        &term.kind,
        ResolvedTemplateTypeKind::Optional(payload) if is_parameter(payload, expected)
    )
}

fn report(
    diagnostics: &mut Diagnostics,
    requirement_span: Span,
    message: impl Into<String>,
    primary_span: Span,
    primary_label: impl Into<String>,
) {
    diagnostics.push(
        Diagnostic::error(INVALID_ITERABLE_LANGUAGE_ITEM, message)
            .with_primary_label(primary_span, primary_label)
            .with_secondary_label(requirement_span, "iteration language item required here"),
    );
}
