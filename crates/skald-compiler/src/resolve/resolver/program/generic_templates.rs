//! Collection of source-level class templates and their frozen parameter scope.

use std::collections::HashMap;

use super::resolver::ModuleUnit;
use super::*;
use crate::identity::{ClassTemplateId, TypeParameterId};

#[derive(Clone, Copy)]
pub(super) struct ClassTemplateWorkItem {
    pub(super) id: ClassTemplateId,
    pub(super) ast_index: usize,
}

pub(super) fn collect_class_templates(
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
                            super::super::DUPLICATE_TYPE_PARAMETER,
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

pub(super) fn validate_class_template_types(
    class: &syntax::ClassDecl,
    parameters: &ResolvedTypeParameters,
    lookup: ModuleLookup<'_>,
    diagnostics: &mut Diagnostics,
) {
    if let Some(base) = &class.direct_base {
        validate_named_type(base, parameters, lookup, diagnostics);
    }
    for member in &class.members {
        match member {
            syntax::ClassMember::Field(field) => {
                validate_type(&field.type_syntax, parameters, lookup, diagnostics)
            }
            syntax::ClassMember::StaticField(field) => {
                validate_type(&field.type_syntax, parameters, lookup, diagnostics)
            }
            syntax::ClassMember::Initializer(initializer) => {
                validate_parameters(&initializer.parameters, parameters, lookup, diagnostics)
            }
            syntax::ClassMember::CopyConstructor(constructor) => {
                validate_parameters(&constructor.parameters, parameters, lookup, diagnostics)
            }
            syntax::ClassMember::CopyAssignment(assignment) => {
                validate_parameters(&assignment.parameters, parameters, lookup, diagnostics)
            }
            syntax::ClassMember::Destructor(_) => {}
            syntax::ClassMember::Method(method) => {
                validate_parameters(&method.parameters, parameters, lookup, diagnostics);
                validate_type(&method.return_type, parameters, lookup, diagnostics);
            }
        }
    }
}

fn validate_parameters(
    declarations: &[syntax::Parameter],
    parameters: &ResolvedTypeParameters,
    lookup: ModuleLookup<'_>,
    diagnostics: &mut Diagnostics,
) {
    for declaration in declarations {
        validate_type(&declaration.type_syntax, parameters, lookup, diagnostics);
    }
}

fn validate_type(
    type_syntax: &syntax::TypeSyntax,
    parameters: &ResolvedTypeParameters,
    lookup: ModuleLookup<'_>,
    diagnostics: &mut Diagnostics,
) {
    match &type_syntax.kind {
        syntax::TypeKind::Named(named) => {
            validate_named_type(named, parameters, lookup, diagnostics)
        }
        syntax::TypeKind::Shared { target, .. }
        | syntax::TypeKind::Optional {
            payload: target, ..
        }
        | syntax::TypeKind::Grouped { inner: target, .. }
        | syntax::TypeKind::Array {
            element: target, ..
        } => validate_type(target, parameters, lookup, diagnostics),
        syntax::TypeKind::I64
        | syntax::TypeKind::U64
        | syntax::TypeKind::U8
        | syntax::TypeKind::F64
        | syntax::TypeKind::Bool
        | syntax::TypeKind::Unit => {}
    }
}

fn validate_named_type(
    named: &syntax::NamedTypeSyntax,
    parameters: &ResolvedTypeParameters,
    lookup: ModuleLookup<'_>,
    diagnostics: &mut Diagnostics,
) {
    if let Some(arguments) = &named.arguments {
        for argument in &arguments.arguments {
            validate_type(argument, parameters, lookup, diagnostics);
        }
    }

    if !named.name.is_qualified() {
        if let Some(parameter) = parameters.get(named.name.text.as_str()) {
            if let Some(arguments) = &named.arguments {
                diagnostics.push(
                    Diagnostic::error(
                        super::super::INVALID_GENERIC_APPLICATION,
                        format!(
                            "type parameter `{}` is not a generic class",
                            named.name.text
                        ),
                    )
                    .with_primary_label(arguments.span, "type arguments are not allowed here")
                    .with_secondary_label(parameter.name_span, "parameter declared here"),
                );
            }
            return;
        }
    }

    match lookup.select(&named.name, diagnostics) {
        TopLevelLookup::Found(TopLevelSymbol {
            kind: TopLevelSymbolKind::ClassTemplate(template),
            name_span,
        }) => match &named.arguments {
            None => diagnostics.push(
                Diagnostic::error(
                    super::super::RAW_GENERIC_TYPE,
                    format!(
                        "generic class `{}` requires type arguments",
                        named.name.text
                    ),
                )
                .with_primary_label(named.name.span, "type arguments cannot be omitted")
                .with_secondary_label(name_span, "template declared here"),
            ),
            Some(arguments) if arguments.arguments.len() != lookup.template_arity(template) => {
                let expected = lookup.template_arity(template);
                diagnostics.push(
                    Diagnostic::error(
                        super::super::GENERIC_ARITY_MISMATCH,
                        format!(
                            "generic class `{}` expects {expected} type argument{}",
                            named.name.text,
                            if expected == 1 { "" } else { "s" },
                        ),
                    )
                    .with_primary_label(arguments.span, "wrong number of type arguments")
                    .with_secondary_label(name_span, "template declared here"),
                );
            }
            Some(_) => {}
        },
        TopLevelLookup::Found(symbol) if named.arguments.is_some() => diagnostics.push(
            Diagnostic::error(
                super::super::INVALID_GENERIC_APPLICATION,
                format!("`{}` is not a generic class", named.name.text),
            )
            .with_primary_label(
                named.arguments.as_ref().unwrap().span,
                "type arguments are not allowed here",
            )
            .with_secondary_label(symbol.name_span, "declaration is non-generic"),
        ),
        TopLevelLookup::Found(_) | TopLevelLookup::Missing | TopLevelLookup::Diagnosed => {}
    }
}
