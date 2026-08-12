//! Stable template and source-ordered parameter identity collection.

use std::collections::HashMap;

use super::super::resolver::ModuleUnit;
use super::*;
use crate::identity::{ClassTemplateId, TypeParameterId};

#[derive(Clone, Copy)]
pub(crate) struct ClassTemplateWorkItem {
    pub(crate) id: ClassTemplateId,
    pub(crate) ast_index: usize,
}

pub(crate) fn collect_class_templates(
    units: &[ModuleUnit<'_>],
    diagnostics: &mut Diagnostics,
) -> (ResolvedClassTemplateTable, ResolvedTypeParameterTable) {
    let mut templates = Vec::new();
    let mut parameter_lists = Vec::new();

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
                    id: TypeParameterId::new(item.id, index),
                    name: parameter.text.to_string(),
                    name_span: parameter.span,
                });
            }
            templates.push(ResolvedClassTemplate {
                id: item.id,
                module: unit.module,
                visibility: resolved_visibility(class.visibility),
                name: class.name.text.to_string(),
                name_span: class.name.span,
                span: class.span,
            });
            parameter_lists.push(ResolvedTypeParameters::new(item.id, resolved));
        }
    }

    (
        ResolvedClassTemplateTable::new(templates),
        ResolvedTypeParameterTable::new(parameter_lists),
    )
}
