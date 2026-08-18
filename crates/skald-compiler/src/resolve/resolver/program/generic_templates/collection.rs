//! Stable template and source-ordered parameter identity collection.

use std::collections::HashMap;

use super::super::resolver::ModuleUnit;
use super::*;
use crate::identity::{
    ClassTemplateId, GenericTemplateId, InterfaceTemplateId, InterfaceTemplateRequirementId,
    TypeParameterId,
};

#[derive(Clone, Copy)]
pub(crate) struct ClassTemplateWorkItem {
    pub(crate) id: ClassTemplateId,
    pub(crate) ast_index: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct InterfaceTemplateWorkItem {
    pub(crate) id: InterfaceTemplateId,
    pub(crate) ast_index: usize,
}

pub(crate) struct CollectedGenericTemplates {
    pub(crate) classes: ResolvedClassTemplateTable,
    pub(crate) interfaces: ResolvedInterfaceTemplateTable,
    pub(crate) parameters: ResolvedTypeParameterTable,
}

pub(crate) fn collect_generic_templates(
    units: &[ModuleUnit<'_>],
    diagnostics: &mut Diagnostics,
) -> CollectedGenericTemplates {
    let mut class_templates = Vec::new();
    let mut class_parameter_lists = Vec::new();
    let mut interface_templates = Vec::new();
    let mut interface_parameter_lists = Vec::new();

    for unit in units {
        for item in &unit.template_work {
            let syntax::TopLevelDeclaration::Class(class) = &unit.ast.declarations[item.ast_index]
            else {
                unreachable!("class-template work must reference a class declaration")
            };
            let parameters = class
                .type_parameters
                .as_ref()
                .expect("class-template work must have type parameters");
            let resolved = collect_parameters(item.id.into(), parameters, diagnostics);
            class_templates.push(ResolvedClassTemplate {
                id: item.id,
                module: unit.module,
                visibility: resolved_visibility(class.visibility),
                name: class.name.text.to_string(),
                name_span: class.name.span,
                span: class.span,
            });
            class_parameter_lists.push(ResolvedTypeParameters::new(item.id, resolved));
        }

        for item in &unit.interface_template_work {
            let syntax::TopLevelDeclaration::Interface(interface) =
                &unit.ast.declarations[item.ast_index]
            else {
                unreachable!("interface-template work must reference an interface declaration")
            };
            let resolved_parameters = interface
                .type_parameters
                .as_ref()
                .map_or_else(Vec::new, |parameters| {
                    collect_parameters(item.id.into(), parameters, diagnostics)
                });
            let requirements = interface
                .requirements
                .iter()
                .enumerate()
                .map(
                    |(index, requirement)| ResolvedInterfaceTemplateRequirement {
                        id: InterfaceTemplateRequirementId::new(item.id, index),
                        name: requirement.name.text.to_string(),
                        name_span: requirement.name.span,
                        span: requirement.span,
                    },
                )
                .collect();
            interface_templates.push(ResolvedInterfaceTemplate::new(
                item.id,
                unit.module,
                resolved_visibility(interface.visibility),
                interface.name.text.to_string(),
                interface.name.span,
                interface.span,
                requirements,
            ));
            interface_parameter_lists
                .push(ResolvedTypeParameters::new(item.id, resolved_parameters));
        }
    }

    CollectedGenericTemplates {
        classes: ResolvedClassTemplateTable::new(class_templates),
        interfaces: ResolvedInterfaceTemplateTable::new(interface_templates),
        parameters: ResolvedTypeParameterTable::new(
            class_parameter_lists,
            interface_parameter_lists,
        ),
    }
}

fn collect_parameters(
    owner: GenericTemplateId,
    parameters: &syntax::GenericParameterList,
    diagnostics: &mut Diagnostics,
) -> Vec<ResolvedTypeParameter> {
    let mut names = HashMap::new();
    let mut resolved = Vec::with_capacity(parameters.parameters.len());
    for (index, parameter) in parameters.parameters.iter().enumerate() {
        if let Some(previous_span) = names.get(parameter.text.as_str()).copied() {
            diagnostics.push(
                Diagnostic::error(
                    super::super::super::DUPLICATE_TYPE_PARAMETER,
                    format!("duplicate type parameter `{}`", parameter.text),
                )
                .with_primary_label(parameter.span, "redeclared here")
                .with_secondary_label(previous_span, "first declared here"),
            );
        } else {
            names.insert(parameter.text.to_string(), parameter.span);
        }
        resolved.push(ResolvedTypeParameter {
            id: TypeParameterId::new(owner, index),
            name: parameter.text.to_string(),
            name_span: parameter.span,
        });
    }
    resolved
}
