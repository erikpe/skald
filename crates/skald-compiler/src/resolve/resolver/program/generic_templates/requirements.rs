//! Inference of contextual mechanical requirements from template type uses.

use super::*;

pub(super) fn infer_declaration_requirements(
    class: &syntax::ClassDecl,
    type_uses: &[ResolvedTemplateTypeUse],
) -> Vec<GenericRequirement> {
    let mut requirements = Vec::new();
    for type_use in type_uses {
        infer_type_construction(&type_use.type_term, &mut requirements);
        match type_use.context {
            ResolvedTemplateTypeUseContext::Field { member } => {
                push(
                    &mut requirements,
                    &type_use.type_term,
                    GenericCapability::FieldStorage,
                    type_use.type_term.span,
                    GenericRequirementReason::FieldDeclaration { member },
                );
                push_destruction(&mut requirements, &type_use.type_term, member);
            }
            ResolvedTemplateTypeUseContext::StaticField { member } => {
                push(
                    &mut requirements,
                    &type_use.type_term,
                    GenericCapability::StaticStorage,
                    type_use.type_term.span,
                    GenericRequirementReason::StaticFieldDeclaration { member },
                );
                push_destruction(&mut requirements, &type_use.type_term, member);
                let syntax::ClassMember::StaticField(field) = &class.members[member] else {
                    unreachable!("static-field type use must reference a static field")
                };
                if field.initializer.is_none() {
                    push(
                        &mut requirements,
                        &type_use.type_term,
                        GenericCapability::DefaultConstructible,
                        type_use.type_term.span,
                        GenericRequirementReason::StaticZeroInitialization { member },
                    );
                } else if let Some(initializer) = &field.initializer {
                    if let Some(copy_term) = stored_initialization_copy_term(
                        &type_use.type_term,
                        &initializer.expression,
                    ) {
                        push(
                            &mut requirements,
                            copy_term,
                            GenericCapability::CopyConstructible,
                            initializer.expression.span(),
                            GenericRequirementReason::StoredInitializationCopy { member },
                        );
                    }
                }
            }
            ResolvedTemplateTypeUseContext::InitializerParameter { member, parameter }
            | ResolvedTemplateTypeUseContext::CopyConstructorParameter { member, parameter }
            | ResolvedTemplateTypeUseContext::CopyAssignmentParameter { member, parameter }
            | ResolvedTemplateTypeUseContext::MethodParameter { member, parameter } => {
                let declaration = member_parameters(&class.members[member])
                    .get(parameter)
                    .expect("parameter type use must reference its declaration");
                let capability = match declaration.binding_mode {
                    syntax::ParameterBindingMode::Value => GenericCapability::ValueParameter,
                    syntax::ParameterBindingMode::ReadOnlyAlias { .. } => {
                        GenericCapability::AliasTarget(GenericAliasAccess::ReadOnly)
                    }
                    syntax::ParameterBindingMode::MutableAlias { .. } => {
                        GenericCapability::AliasTarget(GenericAliasAccess::Mutable)
                    }
                };
                push(
                    &mut requirements,
                    &type_use.type_term,
                    capability,
                    type_use.type_term.span,
                    GenericRequirementReason::ParameterDeclaration { member, parameter },
                );
            }
            ResolvedTemplateTypeUseContext::MethodResult { member } => push(
                &mut requirements,
                &type_use.type_term,
                GenericCapability::ValueResult,
                type_use.type_term.span,
                GenericRequirementReason::MethodResult { member },
            ),
            ResolvedTemplateTypeUseContext::DirectBase
            | ResolvedTemplateTypeUseContext::Local { .. }
            | ResolvedTemplateTypeUseContext::CastTarget { .. }
            | ResolvedTemplateTypeUseContext::TypeTestTarget { .. }
            | ResolvedTemplateTypeUseContext::ConstructionTarget { .. }
            | ResolvedTemplateTypeUseContext::StaticSelectionTarget { .. }
            | ResolvedTemplateTypeUseContext::ArrayConstructionTarget { .. }
            | ResolvedTemplateTypeUseContext::OptionalBoxTarget { .. } => {}
        }
    }
    requirements
}

pub(super) fn infer_type_construction(
    term: &ResolvedTemplateType,
    requirements: &mut Vec<GenericRequirement>,
) {
    match &term.kind {
        ResolvedTemplateTypeKind::Shared(target) => {
            push(
                requirements,
                target,
                GenericCapability::SharedTarget,
                target.span,
                GenericRequirementReason::SharedType,
            );
            infer_shared_target_construction(target, requirements);
        }
        ResolvedTemplateTypeKind::Optional(payload) => {
            push(
                requirements,
                payload,
                GenericCapability::OptionalPayload,
                payload.span,
                GenericRequirementReason::OptionalType,
            );
            infer_type_construction(payload, requirements);
        }
        ResolvedTemplateTypeKind::Array(element) => {
            push(
                requirements,
                element,
                GenericCapability::ArrayElement,
                element.span,
                GenericRequirementReason::ArrayType,
            );
            infer_type_construction(element, requirements);
        }
        ResolvedTemplateTypeKind::ClassTemplate { arguments, .. } => {
            for argument in arguments {
                infer_type_construction(argument, requirements);
            }
        }
        ResolvedTemplateTypeKind::I64
        | ResolvedTemplateTypeKind::U64
        | ResolvedTemplateTypeKind::U8
        | ResolvedTemplateTypeKind::F64
        | ResolvedTemplateTypeKind::Bool
        | ResolvedTemplateTypeKind::Unit
        | ResolvedTemplateTypeKind::Obj
        | ResolvedTemplateTypeKind::Parameter(_)
        | ResolvedTemplateTypeKind::Class(_)
        | ResolvedTemplateTypeKind::Interface(_) => {}
    }
}

fn infer_shared_target_construction(
    mut target: &ResolvedTemplateType,
    requirements: &mut Vec<GenericRequirement>,
) {
    // Leading optional layers below `shared` describe one optional-box target,
    // not independently stored inline optionals. Constructors within the box
    // payload still keep their ordinary requirements (for example the array
    // element in `shared (T?[])?`).
    while let ResolvedTemplateTypeKind::Optional(payload) = &target.kind {
        target = payload;
    }
    infer_type_construction(target, requirements);
}

pub(super) fn push(
    requirements: &mut Vec<GenericRequirement>,
    type_term: &ResolvedTemplateType,
    capability: GenericCapability,
    origin: Span,
    reason: GenericRequirementReason,
) {
    if type_term.depends_on_parameter() {
        requirements.push(GenericRequirement {
            type_term: type_term.clone(),
            capability,
            origin,
            reason,
        });
    }
}

pub(super) fn push_destruction(
    requirements: &mut Vec<GenericRequirement>,
    type_term: &ResolvedTemplateType,
    member: usize,
) {
    push(
        requirements,
        type_term,
        GenericCapability::Destroyable,
        type_term.span,
        GenericRequirementReason::SynthesizedDestruction { member },
    );
}

pub(super) fn expression_is_named(expression: &syntax::Expression) -> bool {
    match expression {
        syntax::Expression::Identifier(_)
        | syntax::Expression::SelfValue(_)
        | syntax::Expression::MemberAccess(_)
        | syntax::Expression::BracketProjection(_)
        | syntax::Expression::Unwrap(_) => true,
        syntax::Expression::Grouped(grouped) => expression_is_named(&grouped.expression),
        _ => false,
    }
}

pub(super) fn stored_initialization_copy_term<'term>(
    destination: &'term ResolvedTemplateType,
    source: &syntax::Expression,
) -> Option<&'term ResolvedTemplateType> {
    match source {
        syntax::Expression::Grouped(grouped) => {
            stored_initialization_copy_term(destination, &grouped.expression)
        }
        syntax::Expression::Present(present) => {
            let ResolvedTemplateTypeKind::Optional(payload) = &destination.kind else {
                return None;
            };
            stored_initialization_copy_term(payload, &present.value)
        }
        source if expression_is_named(source) => Some(destination),
        _ => None,
    }
}

fn member_parameters(member: &syntax::ClassMember) -> &[syntax::Parameter] {
    match member {
        syntax::ClassMember::Initializer(declaration) => &declaration.parameters,
        syntax::ClassMember::CopyConstructor(declaration) => &declaration.parameters,
        syntax::ClassMember::CopyAssignment(declaration) => &declaration.parameters,
        syntax::ClassMember::Method(declaration) => &declaration.parameters,
        syntax::ClassMember::Field(_)
        | syntax::ClassMember::StaticField(_)
        | syntax::ClassMember::Destructor(_) => &[],
    }
}
