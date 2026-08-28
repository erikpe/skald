//! Structural validation for the canonical `std::range` declaration bundle.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    identity::TypeParameterId,
    module::{ModulePath, ProgramModuleTable},
    resolve::{
        CanonicalOperatorProtocol, ResolvedClassTemplateTable,
        ResolvedInterfaceTemplateSemanticTable, ResolvedInterfaceTemplateTable,
        ResolvedInterfaceType, ResolvedIterableLanguageItem, ResolvedModuleDeclarationTable,
        ResolvedOperatorLanguageItem, ResolvedRangeLanguageItem, ResolvedTemplateType,
        ResolvedTemplateTypeKind, ResolvedTopLevelId, ResolvedTypeParameterTable,
        ResolvedVisibility,
    },
    source::Span,
    syntax,
};

use super::super::{
    super::INVALID_RANGE_LANGUAGE_ITEM, ResolvedClassTemplateSemanticTable, ResolvedTemplateBound,
    ResolvedTemplateTypeUseContext,
};

pub(in crate::resolve::resolver::program) struct RangeLanguageItemEvidence<'a> {
    pub requiring_spans: &'a [Span],
    pub range_declarations: &'a [&'a syntax::TopLevelDeclaration],
}

#[derive(Clone, Copy)]
pub(in crate::resolve::resolver::program) struct ValidatedSuccessorLanguageItem {
    template: crate::identity::InterfaceTemplateId,
    output_parameter: TypeParameterId,
    requirement: crate::identity::InterfaceTemplateRequirementId,
    declaration_span: Span,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::resolve::resolver::program) fn validate_successor_language_item(
    modules: &ProgramModuleTable,
    module_declarations: &ResolvedModuleDeclarationTable,
    templates: &ResolvedInterfaceTemplateTable,
    semantics: &ResolvedInterfaceTemplateSemanticTable,
    type_parameters: &ResolvedTypeParameterTable,
    requiring_spans: &[Span],
    successor_declaration_spans: &[Span],
    diagnostics: &mut Diagnostics,
) -> Option<ValidatedSuccessorLanguageItem> {
    let requirement_span = *requiring_spans.first()?;
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
    if successor_declaration_spans.len() > 1 {
        report(
            diagnostics,
            requirement_span,
            "`std::range` must declare `Successor` exactly once",
            successor_declaration_spans[1],
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

        return valid.then_some(ValidatedSuccessorLanguageItem {
            template: template_id,
            output_parameter: output.id,
            requirement: requirement.id,
            declaration_span: template.span,
        });
    }

    None
}

#[allow(clippy::too_many_arguments)]
pub(in crate::resolve::resolver::program) fn validate_range_language_item(
    modules: &ProgramModuleTable,
    module_declarations: &ResolvedModuleDeclarationTable,
    class_templates: &ResolvedClassTemplateTable,
    class_semantics: &ResolvedClassTemplateSemanticTable,
    type_parameters: &ResolvedTypeParameterTable,
    iterable: Option<&ResolvedIterableLanguageItem>,
    operators: Option<&ResolvedOperatorLanguageItem>,
    successor: Option<ValidatedSuccessorLanguageItem>,
    evidence: RangeLanguageItemEvidence<'_>,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedRangeLanguageItem> {
    let requirement_span = *evidence.requiring_spans.first()?;
    let successor = successor?;
    let iterable = iterable?;
    let less = operators?.get(CanonicalOperatorProtocol::Less);
    let path = ModulePath::try_from("std::range").expect("canonical range module path is valid");
    let module = modules
        .find(&path)
        .expect("ordinary reachability evidence must load the canonical range module")
        .module_id();
    let declarations = module_declarations
        .get(module)
        .expect("every loaded module has a declaration table");
    let Some(declaration) = declarations.get("Range") else {
        diagnostics.push(
            Diagnostic::error(
                INVALID_RANGE_LANGUAGE_ITEM,
                "`std::range` does not declare the required `Range` class",
            )
            .with_primary_label(requirement_span, "range class required here"),
        );
        return None;
    };
    if evidence.range_declarations.len() > 1 {
        report(
            diagnostics,
            requirement_span,
            "`std::range` must declare `Range` exactly once",
            evidence.range_declarations[1].span(),
            "duplicate language-item declaration",
        );
        return None;
    }
    let ResolvedTopLevelId::ClassTemplate(template_id) = declaration.declaration else {
        report(
            diagnostics,
            requirement_span,
            "`std::range::Range` must be a generic class",
            declaration.name_span,
            "declared with the wrong kind",
        );
        return None;
    };
    let template = class_templates
        .get(template_id)
        .expect("class-template declaration identity must exist");
    let semantic = class_semantics
        .get(template_id)
        .expect("every class template has semantic metadata");
    let parameters = type_parameters
        .for_template(template_id)
        .expect("every class template has one parameter list");
    let Some(syntax::TopLevelDeclaration::Class(source)) =
        evidence.range_declarations.first().copied()
    else {
        unreachable!("resolved class-template declarations retain class syntax")
    };
    let mut valid = true;

    if template.visibility != ResolvedVisibility::Public {
        report(
            diagnostics,
            requirement_span,
            "`std::range::Range` must be public",
            template.name_span,
            "private language-item declaration",
        );
        valid = false;
    }
    if parameters.len() != 1 {
        report(
            diagnostics,
            requirement_span,
            "`std::range::Range` must declare exactly one type parameter",
            template.name_span,
            format!("found {} type parameters", parameters.len()),
        );
        valid = false;
    }
    let parameter = parameters.iter().next();
    if let Some(parameter) = parameter {
        if parameter.name != "T" {
            report(
                diagnostics,
                requirement_span,
                "the `std::range::Range` parameter must be named `T`",
                parameter.name_span,
                format!("found `{}`", parameter.name),
            );
            valid = false;
        }
    }
    if let Some(base) = &semantic.direct_base {
        report(
            diagnostics,
            requirement_span,
            "`std::range::Range` must not declare a direct base class",
            base.span,
            "remove this direct base",
        );
        valid = false;
    }
    if semantic.bounds.len() != 2 {
        report(
            diagnostics,
            requirement_span,
            "`std::range::Range` must declare exactly two generic bounds",
            template.name_span,
            format!("found {} generic bounds", semantic.bounds.len()),
        );
        valid = false;
    }
    if semantic.implemented_interfaces.len() != 1 {
        report(
            diagnostics,
            requirement_span,
            "`std::range::Range` must declare exactly one interface claim",
            template.name_span,
            format!(
                "found {} interface claims",
                semantic.implemented_interfaces.len()
            ),
        );
        valid = false;
    }

    if let Some(parameter) = parameter {
        valid &= validate_bound(
            semantic.bounds.first(),
            parameter.id,
            less.template,
            "first",
            "OpLess<T>",
            requirement_span,
            diagnostics,
        );
        valid &= validate_bound(
            semantic.bounds.get(1),
            parameter.id,
            successor.template,
            "second",
            "Successor<T>",
            requirement_span,
            diagnostics,
        );
        valid &= validate_iterable_claim(
            semantic.implemented_interfaces.first(),
            parameter.id,
            iterable.template,
            requirement_span,
            diagnostics,
        );
    }

    let initializers = source
        .members
        .iter()
        .enumerate()
        .filter_map(|(member, declaration)| match declaration {
            syntax::ClassMember::Initializer(initializer) => Some((member, initializer)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if initializers.len() != 1 {
        report(
            diagnostics,
            requirement_span,
            "`std::range::Range` must declare exactly one initializer",
            template.name_span,
            format!("found {} initializers", initializers.len()),
        );
        valid = false;
    }
    let initializer = initializers.first().copied();
    if let (Some(parameter), Some((member, initializer))) = (parameter, initializer) {
        if initializer.visibility != syntax::MemberVisibility::Public {
            report(
                diagnostics,
                requirement_span,
                "`std::range::Range.init` must be public",
                initializer.introducer_span,
                "private language-item initializer",
            );
            valid = false;
        }
        if initializer.parameters.len() != 2 {
            report(
                diagnostics,
                requirement_span,
                "`std::range::Range.init` must declare exactly two parameters",
                initializer.introducer_span,
                format!("found {} parameters", initializer.parameters.len()),
            );
            valid = false;
        }
        for (index, expected_name) in ["start", "end"].into_iter().enumerate() {
            let Some(actual) = initializer.parameters.get(index) else {
                continue;
            };
            if actual.name.text != expected_name {
                report(
                    diagnostics,
                    requirement_span,
                    format!(
                        "the {} `std::range::Range.init` parameter must be named `{expected_name}`",
                        ordinal(index)
                    ),
                    actual.name.span,
                    format!("found `{}`", actual.name.text),
                );
                valid = false;
            }
            if actual.binding_mode != syntax::ParameterBindingMode::Value {
                report(
                    diagnostics,
                    requirement_span,
                    "`std::range::Range.init` parameters must use owning value binding",
                    actual.span,
                    "parameter has the wrong binding mode",
                );
                valid = false;
            }
            let type_use = semantic.type_uses.iter().find(|type_use| {
                matches!(
                    type_use.context,
                    ResolvedTemplateTypeUseContext::InitializerParameter {
                        member: actual_member,
                        parameter: actual_parameter,
                    } if actual_member == member && actual_parameter == index
                )
            });
            if !type_use.is_some_and(|type_use| is_parameter(&type_use.type_term, parameter.id)) {
                report(
                    diagnostics,
                    requirement_span,
                    "`std::range::Range.init` parameters must have type `T`",
                    actual.type_syntax.span,
                    "parameter has the wrong type",
                );
                valid = false;
            }
        }

        return valid.then(|| ResolvedRangeLanguageItem {
            successor_template: successor.template,
            successor_output_parameter: successor.output_parameter,
            successor_requirement: successor.requirement,
            successor_declaration_span: successor.declaration_span,
            range_template: template_id,
            range_parameter: parameter.id,
            range_initializer_member: member,
            range_ordering_bound: 0,
            range_ordering_requirement: less.requirement,
            range_successor_bound: 1,
            range_iterable_claim: 0,
            range_declaration_span: template.span,
            requiring_spans: evidence.requiring_spans.to_vec(),
        });
    }

    None
}

fn validate_bound(
    bound: Option<&ResolvedTemplateBound>,
    parameter: TypeParameterId,
    interface: crate::identity::InterfaceTemplateId,
    position: &str,
    expected: &str,
    requirement_span: Span,
    diagnostics: &mut Diagnostics,
) -> bool {
    let Some(bound) = bound else {
        return false;
    };
    if bound.parameter == parameter
        && is_interface_application(&bound.interface, interface, &[parameter])
    {
        return true;
    }
    report(
        diagnostics,
        requirement_span,
        format!("the {position} `std::range::Range` bound must be `T: {expected}`"),
        bound.span,
        "bound has the wrong application",
    );
    false
}

fn validate_iterable_claim(
    claim: Option<&crate::resolve::ResolvedInterfaceClaim>,
    parameter: TypeParameterId,
    iterable: crate::identity::InterfaceTemplateId,
    requirement_span: Span,
    diagnostics: &mut Diagnostics,
) -> bool {
    let Some(claim) = claim else {
        return false;
    };
    if is_interface_application(&claim.interface, iterable, &[parameter, parameter]) {
        return true;
    }
    report(
        diagnostics,
        requirement_span,
        "`std::range::Range` must implement exactly `Iterable<T, T>`",
        claim.span,
        "claim has the wrong application",
    );
    false
}

fn is_interface_application(
    actual: &ResolvedInterfaceType,
    expected_template: crate::identity::InterfaceTemplateId,
    expected_arguments: &[TypeParameterId],
) -> bool {
    matches!(
        actual,
        ResolvedInterfaceType::TemplateApplication { template, arguments }
            if *template == expected_template
                && arguments.len() == expected_arguments.len()
                && arguments.iter().zip(expected_arguments).all(|(argument, expected)| {
                    is_parameter(argument, *expected)
                })
    )
}

fn is_parameter(actual: &ResolvedTemplateType, expected: TypeParameterId) -> bool {
    matches!(actual.kind, ResolvedTemplateTypeKind::Parameter(parameter) if parameter == expected)
}

fn ordinal(index: usize) -> &'static str {
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
        Diagnostic::error(INVALID_RANGE_LANGUAGE_ITEM, message)
            .with_primary_label(primary_span, primary_label)
            .with_secondary_label(requirement_span, "range protocol required here"),
    );
}
