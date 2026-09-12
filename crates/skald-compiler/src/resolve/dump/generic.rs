//! Templates, constraints, specialization states, and resolved type tables.

use std::fmt::Write;

use super::super::ir::*;
use super::ResolvedDumper;
use crate::dump_format::{write_quoted, write_span};

const fn function_parameter_mode_name(mode: ResolvedFunctionTypeParameterMode) -> &'static str {
    match mode {
        ResolvedFunctionTypeParameterMode::Value => "Value",
        ResolvedFunctionTypeParameterMode::ReadOnlyAlias => "ReadOnlyAlias",
        ResolvedFunctionTypeParameterMode::MutableAlias => "MutableAlias",
    }
}

fn render_template_bound_requirement(requirement: ResolvedTemplateBoundRequirement) -> String {
    match requirement {
        ResolvedTemplateBoundRequirement::Ordinary(requirement) => {
            format!("ordinary-requirement {requirement}")
        }
        ResolvedTemplateBoundRequirement::Generic(requirement) => {
            format!("template-requirement {requirement}")
        }
    }
}

fn render_generic_capability(capability: GenericCapability) -> &'static str {
    match capability {
        GenericCapability::FieldStorage => "field-storage",
        GenericCapability::StaticStorage => "static-storage",
        GenericCapability::ValueParameter => "value-parameter",
        GenericCapability::ValueResult => "value-result",
        GenericCapability::AliasTarget(GenericAliasAccess::ReadOnly) => "readonly-alias-target",
        GenericCapability::AliasTarget(GenericAliasAccess::Mutable) => "mutable-alias-target",
        GenericCapability::OptionalPayload => "optional-payload",
        GenericCapability::ArrayElement => "array-element",
        GenericCapability::SharedTarget => "shared-target",
        GenericCapability::DefaultConstructible => "default-constructible",
        GenericCapability::CopyConstructible => "copy-constructible",
        GenericCapability::Assignable => "assignable",
        GenericCapability::Destroyable => "destroyable",
    }
}

fn render_generic_requirement_reason(reason: GenericRequirementReason) -> String {
    match reason {
        GenericRequirementReason::FieldDeclaration { member } => {
            format!("member{member}:field-declaration")
        }
        GenericRequirementReason::StaticFieldDeclaration { member } => {
            format!("member{member}:static-field-declaration")
        }
        GenericRequirementReason::ParameterDeclaration { member, parameter } => {
            format!("member{member}:parameter{parameter}-declaration")
        }
        GenericRequirementReason::MethodResult { member } => {
            format!("member{member}:method-result")
        }
        GenericRequirementReason::InterfaceParameter {
            requirement,
            parameter,
        } => format!("{requirement}:parameter{parameter}-declaration"),
        GenericRequirementReason::InterfaceResult { requirement } => {
            format!("{requirement}:result")
        }
        GenericRequirementReason::OptionalType => "optional-type".to_owned(),
        GenericRequirementReason::ArrayType => "array-type".to_owned(),
        GenericRequirementReason::SharedType => "shared-type".to_owned(),
        GenericRequirementReason::StaticZeroInitialization { member } => {
            format!("member{member}:static-zero-initialization")
        }
        GenericRequirementReason::ArrayLengthConstruction { member } => {
            format!("member{member}:array-length-construction")
        }
        GenericRequirementReason::ExplicitArrayCopy { member } => {
            format!("member{member}:explicit-array-copy")
        }
        GenericRequirementReason::ExplicitCopyConstruction { member } => {
            format!("member{member}:explicit-copy-construction")
        }
        GenericRequirementReason::StoredInitializationCopy { member } => {
            format!("member{member}:stored-initialization-copy")
        }
        GenericRequirementReason::Assignment { member } => {
            format!("member{member}:assignment")
        }
        GenericRequirementReason::SynthesizedDestruction { member } => {
            format!("member{member}:synthesized-destruction")
        }
    }
}

fn render_template_type(type_term: &ResolvedTemplateType) -> String {
    match &type_term.kind {
        ResolvedTemplateTypeKind::I64 => "i64".to_owned(),
        ResolvedTemplateTypeKind::U64 => "u64".to_owned(),
        ResolvedTemplateTypeKind::U8 => "u8".to_owned(),
        ResolvedTemplateTypeKind::F64 => "f64".to_owned(),
        ResolvedTemplateTypeKind::Bool => "bool".to_owned(),
        ResolvedTemplateTypeKind::Unit => "unit".to_owned(),
        ResolvedTemplateTypeKind::Obj => "Obj".to_owned(),
        ResolvedTemplateTypeKind::Parameter(parameter) => parameter.to_string(),
        ResolvedTemplateTypeKind::Class(class) => format!("class {class}"),
        ResolvedTemplateTypeKind::Interface(interface) => format!("interface {interface}"),
        ResolvedTemplateTypeKind::ClassTemplate {
            template,
            arguments,
        } => format!(
            "{template}<{}>",
            arguments
                .iter()
                .map(render_template_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ResolvedTemplateTypeKind::InterfaceTemplate {
            template,
            arguments,
        } => format!(
            "{template}<{}>",
            arguments
                .iter()
                .map(render_template_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ResolvedTemplateTypeKind::Function { parameters, result } => format!(
            "fn({}) -> {}",
            parameters
                .iter()
                .map(|parameter| {
                    let mode = match parameter.mode {
                        ResolvedFunctionTypeParameterMode::Value => "",
                        ResolvedFunctionTypeParameterMode::ReadOnlyAlias => "ref ",
                        ResolvedFunctionTypeParameterMode::MutableAlias => "mut ",
                    };
                    format!("{mode}{}", render_template_type(&parameter.type_syntax))
                })
                .collect::<Vec<_>>()
                .join(", "),
            render_template_type(result)
        ),
        ResolvedTemplateTypeKind::Shared(target) => {
            format!("shared ({})", render_template_type(target))
        }
        ResolvedTemplateTypeKind::Optional(payload) => {
            format!("optional ({})", render_template_type(payload))
        }
        ResolvedTemplateTypeKind::Array(element) => {
            format!("array ({})", render_template_type(element))
        }
    }
}

pub(super) fn render_interface_type(interface: &ResolvedInterfaceType) -> String {
    match interface {
        ResolvedInterfaceType::Ordinary(interface) => interface.to_string(),
        ResolvedInterfaceType::TemplateApplication {
            template,
            arguments,
        } => format!(
            "{template}<{}>",
            arguments
                .iter()
                .map(render_template_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn render_template_type_context(context: ResolvedTemplateTypeUseContext) -> String {
    match context {
        ResolvedTemplateTypeUseContext::DirectBase => "direct-base".to_owned(),
        ResolvedTemplateTypeUseContext::Field { member } => format!("member{member}:field"),
        ResolvedTemplateTypeUseContext::StaticField { member } => {
            format!("member{member}:static-field")
        }
        ResolvedTemplateTypeUseContext::InitializerParameter { member, parameter } => {
            format!("member{member}:initializer-parameter{parameter}")
        }
        ResolvedTemplateTypeUseContext::CopyConstructorParameter { member, parameter } => {
            format!("member{member}:copy-parameter{parameter}")
        }
        ResolvedTemplateTypeUseContext::CopyAssignmentParameter { member, parameter } => {
            format!("member{member}:assignment-parameter{parameter}")
        }
        ResolvedTemplateTypeUseContext::MethodParameter { member, parameter } => {
            format!("member{member}:method-parameter{parameter}")
        }
        ResolvedTemplateTypeUseContext::MethodResult { member } => {
            format!("member{member}:method-result")
        }
        ResolvedTemplateTypeUseContext::Local { member } => format!("member{member}:local"),
        ResolvedTemplateTypeUseContext::CastTarget { member } => {
            format!("member{member}:cast-target")
        }
        ResolvedTemplateTypeUseContext::TypeTestTarget { member } => {
            format!("member{member}:type-test-target")
        }
        ResolvedTemplateTypeUseContext::ConstructionTarget { member } => {
            format!("member{member}:construction-target")
        }
        ResolvedTemplateTypeUseContext::StaticSelectionTarget { member } => {
            format!("member{member}:static-selection-target")
        }
        ResolvedTemplateTypeUseContext::ArrayConstructionTarget { member } => {
            format!("member{member}:array-construction-target")
        }
        ResolvedTemplateTypeUseContext::OptionalBoxTarget { member } => {
            format!("member{member}:optional-box-target")
        }
        ResolvedTemplateTypeUseContext::IterationItemAnnotation { member } => {
            format!("member{member}:iteration-item-annotation")
        }
    }
}

fn render_template_selection_kind(kind: ResolvedTemplateDependentSelectionKind) -> &'static str {
    match kind {
        ResolvedTemplateDependentSelectionKind::Construction(
            ResolvedTemplateConstructionMode::Inline,
        ) => "inline-construction",
        ResolvedTemplateDependentSelectionKind::Construction(
            ResolvedTemplateConstructionMode::Shared,
        ) => "shared-construction",
        ResolvedTemplateDependentSelectionKind::Cast => "cast",
        ResolvedTemplateDependentSelectionKind::TypeTest => "type-test",
        ResolvedTemplateDependentSelectionKind::StaticMember => "static-member",
    }
}

fn render_template_operator_syntax(syntax: ResolvedTemplateOperatorSyntax) -> &'static str {
    match syntax {
        ResolvedTemplateOperatorSyntax::Unary { operator, .. } => operator.spelling(),
        ResolvedTemplateOperatorSyntax::Binary { operator, .. } => operator.spelling(),
    }
}

fn render_template_member(member: Option<&str>) -> String {
    member.map_or_else(String::new, |member| format!(" member {member}"))
}

fn render_specialization_state(state: GenericSpecializationState) -> String {
    match state {
        GenericSpecializationState::Requested => "requested".to_owned(),
        GenericSpecializationState::InProgress(class) => format!("in-progress {class}"),
        GenericSpecializationState::Complete(class) => format!("complete {class}"),
        GenericSpecializationState::Failed { reserved_class } => format!(
            "failed {}",
            reserved_class.map_or_else(|| "unassigned".to_owned(), |class| class.to_string())
        ),
    }
}

fn render_specialization_transition(transition: GenericSpecializationTransition) -> String {
    match transition {
        GenericSpecializationTransition::Requested => "requested".to_owned(),
        GenericSpecializationTransition::InProgress(class) => format!("in-progress {class}"),
        GenericSpecializationTransition::Complete(class) => format!("complete {class}"),
        GenericSpecializationTransition::Failed { reserved_class } => format!(
            "failed {}",
            reserved_class.map_or_else(|| "unassigned".to_owned(), |class| class.to_string())
        ),
    }
}

fn render_interface_specialization_state(state: GenericInterfaceSpecializationState) -> String {
    match state {
        GenericInterfaceSpecializationState::Requested => "requested".to_owned(),
        GenericInterfaceSpecializationState::InProgress(interface) => {
            format!("in-progress {interface}")
        }
        GenericInterfaceSpecializationState::Complete(interface) => format!("complete {interface}"),
        GenericInterfaceSpecializationState::Failed { reserved_interface } => {
            format!("failed {reserved_interface}")
        }
    }
}

fn render_interface_specialization_transition(
    transition: GenericInterfaceSpecializationTransition,
) -> String {
    match transition {
        GenericInterfaceSpecializationTransition::Requested => "requested".to_owned(),
        GenericInterfaceSpecializationTransition::InProgress(interface) => {
            format!("in-progress {interface}")
        }
        GenericInterfaceSpecializationTransition::Complete(interface) => {
            format!("complete {interface}")
        }
        GenericInterfaceSpecializationTransition::Failed { reserved_interface } => {
            format!("failed {reserved_interface}")
        }
    }
}

/// Renders generic declarations, published specialization evidence, and type tables.
pub(super) fn dump_generic_metadata(dumper: &mut ResolvedDumper<'_>) {
    let program = dumper.program;
    if !program.class_templates.is_empty() {
        dumper.heading("ClassTemplates");
        dumper.indented(|dumper| {
            for template in program.class_templates.iter() {
                let parameters = program
                    .type_parameters
                    .for_template(template.id)
                    .expect("every class template has one parameter list");
                dumper.write_indentation();
                let _ = write!(
                    dumper.output,
                    "Template {} module {} ",
                    template.id, template.module
                );
                let name = dumper.qualified_declaration_name(template.module, &template.name);
                write_quoted(&mut dumper.output, &name);
                dumper.output.push_str(" parameters");
                for parameter in parameters.iter() {
                    let _ = write!(dumper.output, " {}=", parameter.id);
                    write_quoted(&mut dumper.output, &parameter.name);
                }
                write_span(&mut dumper.output, template.span);
                dumper.output.push('\n');
            }
        });
    }
    if !program.interface_templates.is_empty() {
        dumper.heading("InterfaceTemplates");
        dumper.indented(|dumper| {
            for template in program.interface_templates.iter() {
                let parameters = program
                    .type_parameters
                    .for_interface_template(template.id)
                    .expect("every interface template has one parameter list");
                dumper.write_indentation();
                let _ = write!(
                    dumper.output,
                    "Template {} module {} ",
                    template.id, template.module
                );
                let name = dumper.qualified_declaration_name(template.module, &template.name);
                write_quoted(&mut dumper.output, &name);
                dumper.output.push_str(" parameters");
                for parameter in parameters.iter() {
                    let _ = write!(dumper.output, " {}=", parameter.id);
                    write_quoted(&mut dumper.output, &parameter.name);
                }
                write_span(&mut dumper.output, template.span);
                dumper.output.push('\n');
                dumper.indented(|dumper| {
                    for requirement in template.requirements() {
                        dumper.write_indentation();
                        let _ = write!(dumper.output, "Requirement {} ", requirement.id);
                        write_quoted(&mut dumper.output, &requirement.name);
                        write_span(&mut dumper.output, requirement.span);
                        dumper.output.push('\n');
                    }
                });
            }
        });
    }
    if !program.interface_template_semantics.is_empty() {
        dumper.heading("InterfaceTemplateSemantics");
        dumper.indented(|dumper| {
            for semantics in program.interface_template_semantics.iter() {
                dumper.raw_line(&format!("Template {}", semantics.template));
                dumper.indented(|dumper| {
                    for bound in &semantics.bounds {
                        dumper.line(
                            &format!(
                                "Bound {} {}",
                                bound.parameter,
                                render_interface_type(&bound.interface)
                            ),
                            bound.span,
                        );
                    }
                    for requirement in &semantics.requirements {
                        dumper.line(
                            &format!(
                                "Requirement {} {}{}",
                                requirement.id,
                                if requirement.mutable { "mutable " } else { "" },
                                requirement.name
                            ),
                            requirement.span,
                        );
                        dumper.indented(|dumper| {
                            for (index, parameter) in requirement.parameters.iter().enumerate() {
                                let mode = match parameter.binding_mode {
                                    ResolvedParameterBindingMode::Value => "value",
                                    ResolvedParameterBindingMode::ReadOnlyAlias { .. } => {
                                        "readonly-alias"
                                    }
                                    ResolvedParameterBindingMode::MutableAlias { .. } => {
                                        "mutable-alias"
                                    }
                                };
                                dumper.line(
                                    &format!(
                                        "Parameter {index} {mode} {} {}",
                                        parameter.name,
                                        render_template_type(&parameter.type_syntax)
                                    ),
                                    parameter.span,
                                );
                            }
                            dumper.line(
                                &format!(
                                    "Result {}",
                                    render_template_type(&requirement.return_type)
                                ),
                                requirement.return_type.span,
                            );
                        });
                    }
                    for type_use in &semantics.type_uses {
                        let context = match type_use.context {
                            ResolvedInterfaceTemplateTypeUseContext::Bound { bound } => {
                                format!("bound{bound}")
                            }
                            ResolvedInterfaceTemplateTypeUseContext::RequirementParameter {
                                requirement,
                                parameter,
                            } => format!("{requirement}:parameter{parameter}"),
                            ResolvedInterfaceTemplateTypeUseContext::RequirementResult {
                                requirement,
                            } => format!("{requirement}:result"),
                        };
                        dumper.line(
                            &format!(
                                "TypeUse {context} {}",
                                render_template_type(&type_use.type_term)
                            ),
                            type_use.type_term.span,
                        );
                    }
                    for requirement in &semantics.contextual_requirements {
                        dumper.line(
                            &format!(
                                "ContextualRequirement {} {} reason {}",
                                render_generic_capability(requirement.capability),
                                render_template_type(&requirement.type_term),
                                render_generic_requirement_reason(requirement.reason),
                            ),
                            requirement.origin,
                        );
                    }
                });
            }
        });
    }
    if program.template_semantics.iter().next().is_some() {
        dumper.heading("TemplateSemantics");
        dumper.indented(|dumper| {
                for semantics in program.template_semantics.iter() {
                    dumper.raw_line(&format!(
                        "Template {} {}",
                        semantics.template,
                        dumper.template_name(semantics.template)
                    ));
                    dumper.indented(|dumper| {
                        if let Some(base) = &semantics.direct_base {
                            dumper.line(
                                &format!("DirectBase {}", render_template_type(base)),
                                base.span,
                            );
                        }
                        for interface in &semantics.implemented_interfaces {
                            dumper.line(
                                &format!(
                                    "Implements {}",
                                    render_interface_type(&interface.interface)
                                ),
                                interface.span,
                            );
                        }
                        for bound in &semantics.bounds {
                            dumper.line(
                                &format!(
                                    "Bound {} interface {}",
                                    bound.parameter,
                                    render_interface_type(&bound.interface)
                                ),
                                bound.span,
                            );
                        }
                        for type_use in &semantics.type_uses {
                            dumper.line(
                                &format!(
                                    "TypeUse {} {}",
                                    render_template_type_context(type_use.context),
                                    render_template_type(&type_use.type_term)
                                ),
                                type_use.type_term.span,
                            );
                        }
                        for requirement in &semantics.requirements {
                            dumper.line(
                                &format!(
                                    "Requirement {} {} reason {}",
                                    render_generic_capability(requirement.capability),
                                    render_template_type(&requirement.type_term),
                                    render_generic_requirement_reason(requirement.reason),
                                ),
                                requirement.origin,
                            );
                        }
                        for selection in &semantics.selections {
                            match selection {
                                ResolvedTemplateSelection::TopLevel { declaration, span } => {
                                    let identity = match declaration {
                                        ResolvedTopLevelId::Function(function) => {
                                            function.to_string()
                                        }
                                        ResolvedTopLevelId::Class(class) => class.to_string(),
                                        ResolvedTopLevelId::ClassTemplate(template) => {
                                            template.to_string()
                                        }
                                        ResolvedTopLevelId::Interface(interface) => {
                                            interface.to_string()
                                        }
                                        ResolvedTopLevelId::InterfaceTemplate(template) => {
                                            template.to_string()
                                        }
                                    };
                                    dumper.line(
                                        &format!("Selection definition-site top-level {identity}"),
                                        *span,
                                    );
                                }
                                ResolvedTemplateSelection::TemplateMember {
                                    member,
                                    member_name,
                                    span,
                                } => dumper.line(
                                    &format!(
                                        "Selection definition-site template-member member{member} {member_name}"
                                    ),
                                    *span,
                                ),
                                ResolvedTemplateSelection::DefinitionSite {
                                    kind,
                                    target,
                                    member_name,
                                    span,
                                } => dumper.line(
                                    &format!(
                                        "Selection definition-site {} {}{}",
                                        render_template_selection_kind(*kind),
                                        render_template_type(target),
                                        render_template_member(member_name.as_deref())
                                    ),
                                    *span,
                                ),
                                ResolvedTemplateSelection::ArgumentDependent {
                                    kind,
                                    target,
                                    member_name,
                                    span,
                                } => dumper.line(
                                    &format!(
                                        "Selection argument-dependent {} {}{}",
                                        render_template_selection_kind(*kind),
                                        render_template_type(target),
                                        render_template_member(member_name.as_deref())
                                    ),
                                    *span,
                                ),
                                ResolvedTemplateSelection::BoundMember {
                                    parameter,
                                    bound,
                                    requirement,
                                    member_name,
                                    span,
                                    ..
                                } => dumper.line(
                                    &format!(
                                        "Selection bound-member {parameter} bound {bound} {} {} member {member_name}",
                                        render_interface_type(&semantics.bounds[*bound].interface),
                                        render_template_bound_requirement(*requirement),
                                    ),
                                    *span,
                                ),
                                ResolvedTemplateSelection::Operator(selection) => dumper.line(
                                    &format!(
                                        "Selection operator {} {} bound {} {} requirement {} rhs {} output {}",
                                        render_template_operator_syntax(selection.syntax),
                                        selection.parameter,
                                        selection.bound,
                                        selection.protocol.interface_name(),
                                        selection.requirement,
                                        selection
                                            .rhs
                                            .as_ref()
                                            .map_or("-".to_owned(), render_template_type),
                                        render_template_type(&selection.output),
                                    ),
                                    selection.span,
                                ),
                                ResolvedTemplateSelection::Iteration {
                                    parameter,
                                    bound,
                                    item,
                                    state,
                                    iter_state,
                                    iter_next,
                                    span,
                                } => dumper.line(
                                    &format!(
                                        "Selection iteration {parameter} bound {bound} {} item {} state {} iter_state {} iter_next {}",
                                        render_interface_type(&semantics.bounds[*bound].interface),
                                        render_template_type(item),
                                        render_template_type(state),
                                        iter_state,
                                        iter_next,
                                    ),
                                    *span,
                                ),
                                ResolvedTemplateSelection::Range {
                                    endpoint,
                                    endpoint_provenance,
                                    span,
                                } => dumper.line(
                                    &format!(
                                        "Selection range endpoint {} provenance lower={} upper={}",
                                        endpoint.as_ref().map_or_else(
                                            || "deferred".to_owned(),
                                            render_template_type,
                                        ),
                                        endpoint_provenance[0].name(),
                                        endpoint_provenance[1].name(),
                                    ),
                                    *span,
                                ),
                            }
                        }
                    });
                }
            });
    }
    if !program.generic_interface_specializations.is_empty() {
        dumper.heading("GenericInterfaceSpecializations");
        dumper.indented(|dumper| {
            for specialization in program.generic_interface_specializations.iter() {
                dumper.line(
                    &format!(
                        "Specialization {} interface {} state {}",
                        dumper.render_interface_specialization_key(&specialization.key),
                        specialization.interface().map_or_else(
                            || "unassigned".to_owned(),
                            |interface| interface.to_string(),
                        ),
                        render_interface_specialization_state(specialization.state),
                    ),
                    specialization.provenance.template_span,
                );
                dumper.indented(|dumper| {
                    if let Some(parameters) = program
                        .type_parameters
                        .for_interface_template(specialization.key.template)
                    {
                        for (parameter, argument) in
                            parameters.iter().zip(&specialization.key.arguments)
                        {
                            dumper.raw_line(&format!(
                                "TypeArgument {} = {}",
                                parameter.id,
                                dumper.render_semantic_type_kind(*argument),
                            ));
                        }
                    }
                    for mapping in &specialization.requirement_mappings {
                        dumper.raw_line(&format!(
                            "RequirementMapping {} -> {}",
                            mapping.template, mapping.closed,
                        ));
                    }
                    for transition in &specialization.transitions {
                        dumper.raw_line(&format!(
                            "Transition {}",
                            render_interface_specialization_transition(*transition),
                        ));
                    }
                    for origin in &specialization.provenance.origins {
                        dumper.line(&format!("Origin module {}", origin.module), origin.span);
                    }
                    if !specialization.provenance.recursion_path.is_empty() {
                        dumper.raw_line("RecursionPath");
                        dumper.indented(|dumper| {
                            for key in &specialization.provenance.recursion_path {
                                dumper.raw_line(&dumper.render_cross_kind_specialization_key(key));
                            }
                        });
                    }
                });
            }
        });
    }
    if !program.generic_specializations.is_empty() {
        dumper.heading("GenericSpecializations");
        dumper.indented(|dumper| {
            for specialization in program.generic_specializations.iter() {
                dumper.line(
                    &format!(
                        "Specialization {} class {} state {}",
                        dumper.render_specialization_key(&specialization.key),
                        specialization
                            .class()
                            .map_or_else(|| "unassigned".to_owned(), |class| class.to_string()),
                        render_specialization_state(specialization.state),
                    ),
                    specialization.provenance.template_span,
                );
                dumper.indented(|dumper| {
                    for transition in &specialization.transitions {
                        dumper.raw_line(&format!(
                            "Transition {}",
                            render_specialization_transition(*transition)
                        ));
                    }
                    for origin in &specialization.provenance.origins {
                        dumper.line(&format!("Origin module {}", origin.module), origin.span);
                    }
                    if !specialization.provenance.recursion_path.is_empty() {
                        dumper.raw_line("RecursionPath");
                        dumper.indented(|dumper| {
                            for key in &specialization.provenance.recursion_path {
                                dumper.raw_line(&dumper.render_cross_kind_specialization_key(key));
                            }
                        });
                    }
                });
            }
        });
    }
    dumper.write_indentation();
    match program.entry_function {
        Some(function) => {
            let _ = writeln!(dumper.output, "Entry {function}");
        }
        None => dumper.output.push_str("Entry <none>\n"),
    }
    if !program.function_types.is_empty() {
        dumper.heading("FunctionTypes");
        dumper.indented(|dumper| {
            for function in program.function_types.iter() {
                dumper.line(
                    &format!(
                        "FunctionType {} {}",
                        function.id,
                        dumper.render_semantic_type_kind(ResolvedTypeKind::Function(function.id))
                    ),
                    function.span,
                );
                dumper.indented(|dumper| {
                    dumper.heading("Parameters");
                    dumper.indented(|dumper| {
                        for parameter in &function.parameters {
                            dumper.line(
                                &format!(
                                    "{} {}",
                                    function_parameter_mode_name(parameter.mode),
                                    dumper.render_type_kind(parameter.type_syntax.kind)
                                ),
                                parameter.span,
                            );
                        }
                    });
                    dumper.heading("Result");
                    dumper.indented(|dumper| dumper.type_syntax(&function.result));
                });
            }
        });
    }
    if !program.address_taken_callables.is_empty() {
        dumper.heading("AddressTakenCallables");
        dumper.indented(|dumper| {
            for callable in program.address_taken_callables.iter() {
                dumper.line(
                    &format!(
                        "AddressTaken {} type {}",
                        callable.target, callable.function_type
                    ),
                    callable.first_reference_span,
                );
            }
        });
    }
    if !program.optional_types.is_empty() {
        dumper.heading("OptionalTypes");
        dumper.indented(|dumper| {
            for optional in program.optional_types.iter() {
                dumper.line(
                    &format!(
                        "OptionalType {} payload {}",
                        optional.id,
                        dumper.render_type_kind(optional.payload.kind)
                    ),
                    optional.payload.span,
                );
            }
        });
    }
    if !program.optional_box_types.is_empty() {
        dumper.heading("OptionalBoxTypes");
        dumper.indented(|dumper| {
            for target in program.optional_box_types.iter() {
                let leaf = match target.object_leaf {
                    Some(ResolvedObjectTarget::Obj) => " object Obj".to_owned(),
                    Some(ResolvedObjectTarget::Class(class)) => {
                        format!(" object class {class}")
                    }
                    Some(ResolvedObjectTarget::Interface(interface)) => {
                        format!(" object interface {interface}")
                    }
                    None => String::new(),
                };
                dumper.line(
                    &format!(
                        "OptionalBoxType {} exact {} depth {}{}",
                        target.id,
                        target
                            .optional
                            .map(|optional| optional.to_string())
                            .unwrap_or_else(|| "view-only".to_owned()),
                        target.optional_depth,
                        leaf
                    ),
                    target.span,
                );
            }
        });
    }
    if !program.array_types.is_empty() {
        dumper.heading("ArrayTypes");
        dumper.indented(|dumper| {
            for array in program.array_types.iter() {
                dumper.line(&format!("ArrayType {}", array.id), array.element.span);
                dumper.indented(|dumper| dumper.type_syntax(&array.element));
            }
        });
    }
}
