//! Complete declaration/header orchestration for one class template.

use std::collections::HashMap;

use super::{body::resolve_template_body, bounds::*, requirements::*, *};

pub(crate) fn resolve_class_template_semantics(
    template: ClassTemplateId,
    class: &syntax::ClassDecl,
    parameters: &ResolvedTypeParameters,
    lookup: ModuleLookup<'_>,
    interfaces: &ResolvedInterfaceDeclarationTable,
    diagnostics: &mut Diagnostics,
) -> ResolvedClassTemplateSemantics {
    let bounds = resolve_bounds(class, parameters, lookup, diagnostics);
    let implemented_interfaces =
        resolve_implemented_interfaces(class, parameters, lookup, diagnostics);
    let mut type_uses = Vec::new();
    let mut fields = HashMap::new();
    let mut callable_parameters = Vec::with_capacity(class.members.len());
    let member_names = class
        .members
        .iter()
        .enumerate()
        .filter_map(|(index, member)| match member {
            syntax::ClassMember::Field(field) => Some((field.name.text.to_string(), index)),
            syntax::ClassMember::StaticField(field) => Some((field.name.text.to_string(), index)),
            syntax::ClassMember::Method(method) => Some((method.name.text.to_string(), index)),
            syntax::ClassMember::Initializer(_)
            | syntax::ClassMember::CopyConstructor(_)
            | syntax::ClassMember::CopyAssignment(_)
            | syntax::ClassMember::Destructor(_) => None,
        })
        .collect::<HashMap<_, _>>();

    let direct_base = class.direct_base.as_ref().and_then(|base| {
        let syntax = syntax::TypeSyntax {
            kind: syntax::TypeKind::Named(base.clone()),
            span: base.span,
        };
        let resolved =
            TemplateTypeResolver::new(parameters, lookup, diagnostics).resolve(&syntax)?;
        type_uses.push(ResolvedTemplateTypeUse {
            context: ResolvedTemplateTypeUseContext::DirectBase,
            type_term: resolved.clone(),
        });
        match resolved.kind {
            ResolvedTemplateTypeKind::Class(_) | ResolvedTemplateTypeKind::ClassTemplate { .. } => {
                Some(resolved)
            }
            ResolvedTemplateTypeKind::Parameter(parameter) => {
                let declaration = parameters
                    .iter()
                    .find(|candidate| candidate.id == parameter)
                    .expect("template type parameter belongs to its declaration");
                diagnostics.push(
                    Diagnostic::error(
                        super::super::super::INVALID_GENERIC_BASE,
                        "a class template cannot extend a bare type parameter",
                    )
                    .with_primary_label(base.span, "parameter-dependent base class is unsupported")
                    .with_secondary_label(declaration.name_span, "parameter declared here"),
                );
                None
            }
            _ => {
                diagnostics.push(
                    Diagnostic::error(
                        super::super::super::INVALID_GENERIC_BASE,
                        format!("`{}` is not a class base", base.name.text),
                    )
                    .with_primary_label(base.span, "expected a class or generic class application"),
                );
                None
            }
        }
    });

    for (member_index, member) in class.members.iter().enumerate() {
        let mut parameters_by_name = HashMap::new();
        match member {
            syntax::ClassMember::Field(field) => {
                if let Some(term) = resolve_type_use(
                    &field.type_syntax,
                    ResolvedTemplateTypeUseContext::Field {
                        member: member_index,
                    },
                    parameters,
                    lookup,
                    diagnostics,
                    &mut type_uses,
                ) {
                    fields.insert(field.name.text.to_string(), term);
                }
            }
            syntax::ClassMember::StaticField(field) => {
                resolve_type_use(
                    &field.type_syntax,
                    ResolvedTemplateTypeUseContext::StaticField {
                        member: member_index,
                    },
                    parameters,
                    lookup,
                    diagnostics,
                    &mut type_uses,
                );
            }
            syntax::ClassMember::Initializer(initializer) => resolve_parameters(
                &initializer.parameters,
                member_index,
                ParameterContext::Initializer,
                parameters,
                lookup,
                diagnostics,
                &mut type_uses,
                &mut parameters_by_name,
            ),
            syntax::ClassMember::CopyConstructor(constructor) => resolve_parameters(
                &constructor.parameters,
                member_index,
                ParameterContext::CopyConstructor,
                parameters,
                lookup,
                diagnostics,
                &mut type_uses,
                &mut parameters_by_name,
            ),
            syntax::ClassMember::CopyAssignment(assignment) => resolve_parameters(
                &assignment.parameters,
                member_index,
                ParameterContext::CopyAssignment,
                parameters,
                lookup,
                diagnostics,
                &mut type_uses,
                &mut parameters_by_name,
            ),
            syntax::ClassMember::Destructor(_) => {}
            syntax::ClassMember::Method(method) => {
                resolve_parameters(
                    &method.parameters,
                    member_index,
                    ParameterContext::Method,
                    parameters,
                    lookup,
                    diagnostics,
                    &mut type_uses,
                    &mut parameters_by_name,
                );
                resolve_type_use(
                    &method.return_type,
                    ResolvedTemplateTypeUseContext::MethodResult {
                        member: member_index,
                    },
                    parameters,
                    lookup,
                    diagnostics,
                    &mut type_uses,
                );
            }
        }
        callable_parameters.push(parameters_by_name);
    }

    let mut requirements = infer_declaration_requirements(class, &type_uses);
    let mut selections = Vec::new();
    let member_results = class
        .members
        .iter()
        .enumerate()
        .filter_map(|(member_index, member)| {
            let syntax::ClassMember::Method(method) = member else {
                return None;
            };
            type_uses
                .iter()
                .find_map(|type_use| {
                    matches!(
                        type_use.context,
                        ResolvedTemplateTypeUseContext::MethodResult { member }
                            if member == member_index
                    )
                    .then_some(type_use.type_term.clone())
                })
                .map(|result| (method.name.text.to_string(), result))
        })
        .collect::<HashMap<_, _>>();
    for (member_index, member) in class.members.iter().enumerate() {
        let callable_result = type_uses.iter().find_map(|type_use| {
            matches!(
                type_use.context,
                ResolvedTemplateTypeUseContext::MethodResult { member }
                    if member == member_index
            )
            .then_some(type_use.type_term.clone())
        });
        resolve_template_body(
            member,
            member_index,
            parameters,
            &bounds,
            interfaces,
            lookup,
            &fields,
            &member_names,
            &member_results,
            direct_base.is_some(),
            &callable_parameters[member_index],
            callable_result.as_ref(),
            &mut type_uses,
            &mut requirements,
            &mut selections,
            diagnostics,
        );
    }

    ResolvedClassTemplateSemantics {
        template,
        direct_base,
        implemented_interfaces,
        bounds,
        type_uses,
        requirements,
        selections,
    }
}

#[derive(Clone, Copy)]
enum ParameterContext {
    Initializer,
    CopyConstructor,
    CopyAssignment,
    Method,
}

#[allow(clippy::too_many_arguments)]
fn resolve_parameters(
    declarations: &[syntax::Parameter],
    member: usize,
    context: ParameterContext,
    parameters: &ResolvedTypeParameters,
    lookup: ModuleLookup<'_>,
    diagnostics: &mut Diagnostics,
    type_uses: &mut Vec<ResolvedTemplateTypeUse>,
    parameters_by_name: &mut HashMap<String, ResolvedTemplateType>,
) {
    for (parameter, declaration) in declarations.iter().enumerate() {
        let context = match context {
            ParameterContext::Initializer => {
                ResolvedTemplateTypeUseContext::InitializerParameter { member, parameter }
            }
            ParameterContext::CopyConstructor => {
                ResolvedTemplateTypeUseContext::CopyConstructorParameter { member, parameter }
            }
            ParameterContext::CopyAssignment => {
                ResolvedTemplateTypeUseContext::CopyAssignmentParameter { member, parameter }
            }
            ParameterContext::Method => {
                ResolvedTemplateTypeUseContext::MethodParameter { member, parameter }
            }
        };
        if let Some(term) = resolve_type_use(
            &declaration.type_syntax,
            context,
            parameters,
            lookup,
            diagnostics,
            type_uses,
        ) {
            parameters_by_name.insert(declaration.name.text.to_string(), term);
        }
    }
}

fn resolve_type_use(
    syntax: &syntax::TypeSyntax,
    context: ResolvedTemplateTypeUseContext,
    parameters: &ResolvedTypeParameters,
    lookup: ModuleLookup<'_>,
    diagnostics: &mut Diagnostics,
    type_uses: &mut Vec<ResolvedTemplateTypeUse>,
) -> Option<ResolvedTemplateType> {
    let resolved = TemplateTypeResolver::new(parameters, lookup, diagnostics).resolve(syntax)?;
    type_uses.push(ResolvedTemplateTypeUse {
        context,
        type_term: resolved.clone(),
    });
    Some(resolved)
}
