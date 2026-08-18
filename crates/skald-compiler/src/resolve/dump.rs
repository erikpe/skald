//! Deterministic textual rendering of the resolved program.

use std::fmt::Write;

use crate::{
    dump_format::{write_indentation, write_quoted, write_span},
    identity::{
        ArrayTypeId, ClassId, ClassTemplateId, FunctionTypeId, InterfaceId, ModuleId,
        OptionalBoxTypeId, OptionalTypeId,
    },
    source::Span,
};

use super::ir::*;

const fn function_parameter_mode_name(mode: ResolvedFunctionTypeParameterMode) -> &'static str {
    match mode {
        ResolvedFunctionTypeParameterMode::Value => "Value",
        ResolvedFunctionTypeParameterMode::ReadOnlyAlias => "ReadOnlyAlias",
        ResolvedFunctionTypeParameterMode::MutableAlias => "MutableAlias",
    }
}

const fn function_parameter_mode_prefix(mode: ResolvedFunctionTypeParameterMode) -> &'static str {
    match mode {
        ResolvedFunctionTypeParameterMode::Value => "",
        ResolvedFunctionTypeParameterMode::ReadOnlyAlias => "ref ",
        ResolvedFunctionTypeParameterMode::MutableAlias => "mut ref ",
    }
}

pub fn dump_resolved(program: &ResolvedProgram) -> String {
    let mut dumper = ResolvedDumper::new(program);
    dumper.line("ResolvedProgram", program.span);
    dumper.indented(|dumper| {
        dumper.raw_line(&format!("SelectedModule {}", program.modules.selected()));
        dumper.heading("Modules");
        dumper.indented(|dumper| {
            for module in program.modules.iter() {
                dumper.raw_line(&format!(
                    "Module {} {} source {} provider {} package {}",
                    module.module_id(),
                    module.module_path(),
                    module.source_id().index(),
                    module.provider_id(),
                    module.package_id()
                ));
            }
        });
        if let Some(item) = &program.string_language_item {
            dumper.raw_line(&format!(
                "StringLanguageItem class {} fields {} {} {} {}",
                item.class,
                item.storage_field,
                item.start_field,
                item.length_field,
                item.hash_code_field
            ));
        }
        if !program.literal_data.is_empty() {
            dumper.heading("LiteralData");
            dumper.indented(|dumper| {
                for literal in program.literal_data.iter() {
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "{} bytes=", literal.id);
                    for byte in &literal.bytes {
                        let _ = write!(dumper.output, "{byte:02x}");
                    }
                    write_span(&mut dumper.output, literal.span);
                    dumper.output.push('\n');
                }
            });
        }
        if !program.external_links.is_empty() {
            dumper.heading("ExternalLinks");
            dumper.indented(|dumper| {
                for link in program.external_links.iter() {
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Link {} ", link.id);
                    write_quoted(&mut dumper.output, &link.symbol);
                    dumper.output.push_str(" declarations");
                    for declaration in &link.declarations {
                        let _ = write!(dumper.output, " {declaration}");
                    }
                    dumper.output.push('\n');
                }
            });
        }
        if program
            .module_bindings
            .iter()
            .any(|module| module.iter().next().is_some())
        {
            dumper.heading("ModuleBindings");
            dumper.indented(|dumper| {
                for module in program.module_bindings.iter() {
                    if module.iter().next().is_none() {
                        continue;
                    }
                    dumper.raw_line(&format!("Module {}", module.module));
                    dumper.indented(|dumper| {
                        for binding in module.iter() {
                            let target = program
                                .modules
                                .get(binding.target)
                                .expect("resolved bindings reference loaded modules");
                            dumper.write_indentation();
                            let _ = write!(
                                dumper.output,
                                "{} -> {} {}",
                                binding.local_path,
                                binding.target,
                                target.module_path()
                            );
                            write_span(&mut dumper.output, binding.name_span);
                            dumper.output.push('\n');
                        }
                    });
                }
            });
        }
        if program
            .ordinary_bindings
            .iter()
            .any(|module| module.iter().next().is_some())
        {
            dumper.heading("OrdinaryBindings");
            dumper.indented(|dumper| {
                for module in program.ordinary_bindings.iter() {
                    if module.iter().next().is_none() {
                        continue;
                    }
                    dumper.raw_line(&format!("Module {}", module.module));
                    dumper.indented(|dumper| {
                        for binding in module.iter() {
                            let target_module = program
                                .modules
                                .get(binding.target_module)
                                .expect("ordinary bindings reference loaded modules");
                            let target = program
                                .module_declarations
                                .declaration(binding.target_module, binding.target)
                                .expect("ordinary bindings reference target declarations");
                            let identity = match binding.target {
                                ResolvedTopLevelId::Function(function) => function.to_string(),
                                ResolvedTopLevelId::Class(class) => class.to_string(),
                                ResolvedTopLevelId::ClassTemplate(template) => template.to_string(),
                                ResolvedTopLevelId::Interface(interface) => interface.to_string(),
                                ResolvedTopLevelId::InterfaceTemplate(template) => {
                                    template.to_string()
                                }
                            };
                            dumper.write_indentation();
                            let _ = write!(
                                dumper.output,
                                "{} -> {} {} {}::{}",
                                binding.local_name,
                                identity,
                                binding.target_module,
                                target_module.module_path(),
                                target.name
                            );
                            write_span(&mut dumper.output, binding.name_span);
                            dumper.output.push('\n');
                        }
                    });
                }
            });
        }
        dumper.heading("ModuleDeclarations");
        dumper.indented(|dumper| {
            for module in program.module_declarations.iter() {
                dumper.raw_line(&format!("Module {}", module.module));
                dumper.indented(|dumper| {
                    for declaration in module.iter() {
                        dumper.write_indentation();
                        let visibility = match declaration.visibility {
                            ResolvedVisibility::Private => "private",
                            ResolvedVisibility::Public => "public",
                        };
                        let identity = match declaration.declaration {
                            ResolvedTopLevelId::Function(function) => function.to_string(),
                            ResolvedTopLevelId::Class(class) => class.to_string(),
                            ResolvedTopLevelId::ClassTemplate(template) => template.to_string(),
                            ResolvedTopLevelId::Interface(interface) => interface.to_string(),
                            ResolvedTopLevelId::InterfaceTemplate(template) => {
                                template.to_string()
                            }
                        };
                        let _ = write!(dumper.output, "{visibility} {identity} ");
                        write_quoted(&mut dumper.output, &declaration.name);
                        write_span(&mut dumper.output, declaration.name_span);
                        dumper.output.push('\n');
                    }
                });
            }
        });
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
                    let name =
                        dumper.qualified_declaration_name(template.module, &template.name);
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
                    let name =
                        dumper.qualified_declaration_name(template.module, &template.name);
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
                                for (index, parameter) in
                                    requirement.parameters.iter().enumerate()
                                {
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
                                    interface,
                                    requirement,
                                    member_name,
                                    span,
                                } => dumper.line(
                                    &format!(
                                        "Selection bound-member {parameter} interface {interface} requirement {requirement} member {member_name}"
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
        if !program.classes.is_empty() {
            dumper.heading("ClassDeclarations");
            dumper.indented(|dumper| {
                for class in program.classes.iter() {
                    let specialization = program.generic_specializations.for_class(class.id);
                    let parameters = specialization.and_then(|specialization| {
                        program
                            .type_parameters
                            .for_template(specialization.key.template)
                    });
                    dumper.class_declaration(class, specialization, parameters);
                }
            });
        }
        if !program.interfaces.is_empty() {
            dumper.heading("InterfaceDeclarations");
            dumper.indented(|dumper| {
                for interface in program.interfaces.iter() {
                    dumper.interface_declaration(interface);
                }
            });
        }
        if !program.virtual_families.is_empty() {
            dumper.heading("VirtualFamilies");
            dumper.indented(|dumper| {
                for family in program.virtual_families.iter() {
                    dumper.raw_line(&format!(
                        "Family {} slot {} root {}",
                        family.id, family.slot, family.root
                    ));
                }
            });
        }
        dumper.heading("Declarations");
        dumper.indented(|dumper| {
            for declaration in program.declarations.iter() {
                dumper.declaration(declaration);
            }
        });
        dumper.heading("Definitions");
        dumper.indented(|dumper| {
            for definition in program.definitions.iter() {
                dumper.definition(definition);
            }
        });
        if !program.classes.is_empty() {
            dumper.heading("ClassDefinitions");
            dumper.indented(|dumper| {
                for class in program.class_definitions.iter() {
                    dumper.class_definition(class);
                }
            });
        }
    });
    dumper.output
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

fn render_interface_type(interface: &ResolvedInterfaceType) -> String {
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

fn render_template_member(member: Option<&str>) -> String {
    member.map_or_else(String::new, |member| format!(" member {member}"))
}

struct ResolvedDumper<'program> {
    output: String,
    indentation: usize,
    program: &'program ResolvedProgram,
}

impl<'program> ResolvedDumper<'program> {
    fn new(program: &'program ResolvedProgram) -> Self {
        Self {
            output: String::new(),
            indentation: 0,
            program,
        }
    }

    fn interface_declaration(&mut self, interface: &ResolvedInterfaceDeclaration) {
        self.write_indentation();
        let _ = write!(
            self.output,
            "Interface {} module {} ",
            interface.id, interface.module
        );
        write_quoted(&mut self.output, &interface.name);
        write_span(&mut self.output, interface.span);
        self.output.push('\n');
        self.indented(|dumper| {
            for requirement in &interface.requirements {
                dumper.write_indentation();
                let _ = write!(
                    dumper.output,
                    "Requirement {} {} ",
                    requirement.id,
                    if requirement.mutable {
                        "mutable"
                    } else {
                        "readonly"
                    },
                );
                write_quoted(&mut dumper.output, &requirement.name);
                write_span(&mut dumper.output, requirement.span);
                dumper.output.push('\n');
                dumper.indented(|dumper| {
                    for parameter in &requirement.parameters {
                        dumper.named_parameter(parameter);
                    }
                    dumper.heading("ReturnType");
                    dumper.indented(|dumper| dumper.type_syntax(&requirement.return_type));
                });
            }
        });
    }

    fn named_parameter(&mut self, parameter: &ResolvedInterfaceParameter) {
        self.write_indentation();
        self.output.push_str("Parameter ");
        write_quoted(&mut self.output, &parameter.name);
        write_span(&mut self.output, parameter.span);
        self.output.push('\n');
        self.indented(|dumper| dumper.type_syntax(&parameter.type_syntax));
    }

    fn class_declaration(
        &mut self,
        class: &ResolvedClassDeclaration,
        specialization: Option<&GenericSpecialization>,
        parameters: Option<&ResolvedTypeParameters>,
    ) {
        self.write_indentation();
        let _ = write!(self.output, "Class {} module {} ", class.id, class.module);
        write_quoted(&mut self.output, &class.name);
        write_span(&mut self.output, class.span);
        self.output.push('\n');
        self.indented(|dumper| {
            if let Some(specialization) = specialization {
                dumper.raw_line(&format!(
                    "SpecializedFrom {}",
                    dumper.render_specialization_key(&specialization.key)
                ));
                if let Some(parameters) = parameters {
                    for (parameter, argument) in
                        parameters.iter().zip(&specialization.key.arguments)
                    {
                        dumper.raw_line(&format!(
                            "TypeArgument {} = {}",
                            parameter.id,
                            dumper.render_type_kind(*argument)
                        ));
                    }
                }
                for (index, interface) in specialization.closed_interface_claims.iter().enumerate()
                {
                    if let Some(interface) = interface {
                        dumper.raw_line(&format!("ClosedInterfaceClaim {index} -> {interface}"));
                    }
                }
                for origin in &specialization.provenance.origins {
                    dumper.line(
                        &format!("SpecializationOrigin module {}", origin.module),
                        origin.span,
                    );
                }
            }
            if let Some(base) = class.direct_base {
                dumper.line(&format!("DirectBase {}", base.class), base.span);
            }
            for claim in &class.implemented_interfaces {
                dumper.line(
                    &format!("Implements {}", render_interface_type(&claim.interface)),
                    claim.span,
                );
            }
            dumper.heading("Fields");
            dumper.indented(|dumper| {
                for field in &class.fields {
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Field {} ", field.id);
                    if field.visibility.private_span().is_some() {
                        dumper.output.push_str("private ");
                    }
                    if field.cell_span.is_some() {
                        dumper.output.push_str("cell ");
                    }
                    if field.final_span.is_some() {
                        dumper.output.push_str("final ");
                    }
                    write_quoted(&mut dumper.output, &field.name);
                    write_span(&mut dumper.output, field.span);
                    dumper.output.push('\n');
                    dumper.indented(|dumper| {
                        if let Some(span) = field.visibility.private_span() {
                            dumper.line("Private", span);
                        }
                        if let Some(span) = field.cell_span {
                            dumper.line("Cell", span);
                        }
                        if let Some(span) = field.final_span {
                            dumper.line("Final", span);
                        }
                        dumper.type_syntax(&field.type_syntax);
                    });
                }
            });
            if !class.static_fields.is_empty() {
                dumper.heading("StaticFields");
                dumper.indented(|dumper| {
                    for field in &class.static_fields {
                        dumper.write_indentation();
                        let _ = write!(dumper.output, "StaticField {} ", field.id);
                        if field.visibility.private_span().is_some() {
                            dumper.output.push_str("private ");
                        }
                        if field.final_span.is_some() {
                            dumper.output.push_str("final ");
                        }
                        write_quoted(&mut dumper.output, &field.name);
                        write_span(&mut dumper.output, field.span);
                        dumper.output.push('\n');
                        dumper.indented(|dumper| {
                            if let Some(span) = field.visibility.private_span() {
                                dumper.line("Private", span);
                            }
                            if let Some(span) = field.final_span {
                                dumper.line("Final", span);
                            }
                            dumper.line("Static", field.static_span);
                            dumper.type_syntax(&field.type_syntax);
                            if let Some(initializer) = &field.initializer {
                                dumper.line(
                                    &format!("DeclarationInitializer {}", initializer.id),
                                    initializer.span,
                                );
                                dumper.indented(|dumper| {
                                    dumper.line("Equal", initializer.equal_span);
                                    dumper.expression(&initializer.expression);
                                });
                            }
                        });
                    }
                });
            }
            dumper.heading("OrdinaryInitializers");
            dumper.indented(|dumper| {
                for initializer in &class.initializers {
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Initializer {}", initializer.id);
                    if initializer.visibility.private_span().is_some() {
                        dumper.output.push_str(" private");
                    }
                    write_span(&mut dumper.output, initializer.span);
                    dumper.output.push('\n');
                    dumper.indented(|dumper| {
                        if let Some(span) = initializer.visibility.private_span() {
                            dumper.line("Private", span);
                        }
                        dumper.parameters(&initializer.parameters);
                    });
                }
            });
            dumper.heading("CopyConstructor");
            dumper.indented(|dumper| match class.copy_constructor {
                ResolvedCopyOperation::User(id) => {
                    let declaration = class
                        .copy_constructor_declaration
                        .as_ref()
                        .expect("user copy constructor must have declaration metadata");
                    dumper.line(&format!("User {id}"), declaration.span);
                    dumper.indented(|dumper| dumper.parameters(&declaration.parameters));
                }
                ResolvedCopyOperation::Synthesized(class) => {
                    dumper.raw_line(&format!("Synthesized {class}"));
                }
                ResolvedCopyOperation::Unavailable => dumper.raw_line("Unavailable"),
            });
            dumper.heading("CopyAssignment");
            dumper.indented(|dumper| match class.copy_assignment {
                ResolvedCopyOperation::User(id) => {
                    let declaration = class
                        .copy_assignment_declaration
                        .as_ref()
                        .expect("user copy assignment must have declaration metadata");
                    dumper.line(&format!("User {id}"), declaration.span);
                    dumper.indented(|dumper| {
                        dumper.parameters(std::slice::from_ref(&declaration.parameter))
                    });
                }
                ResolvedCopyOperation::Synthesized(class) => {
                    dumper.raw_line(&format!("Synthesized {class}"));
                }
                ResolvedCopyOperation::Unavailable => dumper.raw_line("Unavailable"),
            });
            dumper.heading("Destructor");
            if let Some(destructor) = &class.destructor {
                dumper.indented(|dumper| {
                    dumper.line(&format!("Destructor {}", destructor.id), destructor.span);
                });
            } else {
                dumper.indented(|dumper| dumper.raw_line("<none>"));
            }
            dumper.heading("Methods");
            dumper.indented(|dumper| {
                for method in &class.methods {
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Method {} ", method.id);
                    match method.kind {
                        ResolvedMethodKind::Instance {
                            receiver_access, ..
                        } => dumper.output.push_str(match receiver_access {
                            ResolvedReceiverAccess::ReadOnly => "readonly",
                            ResolvedReceiverAccess::Mutable => "mutable",
                        }),
                        ResolvedMethodKind::Static => dumper.output.push_str("static"),
                    }
                    dumper.output.push(' ');
                    if method.visibility.private_span().is_some() {
                        dumper.output.push_str("private ");
                    }
                    write_quoted(&mut dumper.output, &method.name);
                    write_span(&mut dumper.output, method.span);
                    dumper.output.push('\n');
                    dumper.indented(|dumper| {
                        if let Some(span) = method.visibility.private_span() {
                            dumper.line("Private", span);
                        }
                        if let Some(dispatch) = method.kind.dispatch() {
                            dumper.method_dispatch(dispatch);
                        }
                        dumper.parameters(&method.parameters);
                        dumper.heading("ReturnType");
                        dumper.indented(|dumper| dumper.type_syntax(&method.return_type));
                    });
                }
            });
        });
    }

    fn method_dispatch(&mut self, dispatch: ResolvedMethodDispatch) {
        match dispatch {
            ResolvedMethodDispatch::Direct => {}
            ResolvedMethodDispatch::VirtualRoot { family, slot } => {
                self.raw_line(&format!("Dispatch VirtualRoot {family} slot {slot}"));
            }
            ResolvedMethodDispatch::Override {
                family,
                slot,
                root,
                overridden,
            } => self.raw_line(&format!(
                "Dispatch Override {family} slot {slot} root {root} overridden {overridden}"
            )),
        }
    }

    fn class_definition(&mut self, class: &ResolvedClassDefinition) {
        self.line(&format!("ClassDefinition {}", class.class), class.span);
        self.indented(|dumper| {
            for initializer in &class.initializers {
                dumper.member_definition(initializer);
            }
            if let Some(copy_constructor) = &class.copy_constructor {
                dumper.member_definition(copy_constructor);
            }
            if let Some(copy_assignment) = &class.copy_assignment {
                dumper.member_definition(copy_assignment);
            }
            if let Some(destructor) = &class.destructor {
                dumper.member_definition(destructor);
            }
            for method in &class.methods {
                dumper.member_definition(method);
            }
        });
    }

    fn member_definition(&mut self, definition: &ResolvedMemberDefinition) {
        self.line(
            &format!("MemberDefinition {}", definition.callable),
            definition.span,
        );
        self.indented(|dumper| {
            dumper.locals(&definition.locals);
            dumper.block(&definition.body);
        });
    }

    fn declaration(&mut self, declaration: &ResolvedFunctionDeclaration) {
        self.write_indentation();
        let _ = write!(
            self.output,
            "Declaration {} module {} ",
            declaration.id, declaration.module
        );
        write_quoted(&mut self.output, &declaration.name);
        match &declaration.linkage {
            ResolvedFunctionLinkage::Internal => self.output.push_str(" internal"),
            ResolvedFunctionLinkage::External { link } => {
                let _ = write!(self.output, " external {link}");
            }
            ResolvedFunctionLinkage::Intrinsic { intrinsic } => {
                let _ = write!(self.output, " intrinsic {intrinsic:?}");
            }
            ResolvedFunctionLinkage::UnrecognizedIntrinsic => {
                self.output.push_str(" intrinsic Unrecognized");
            }
        }
        write_span(&mut self.output, declaration.span);
        self.output.push('\n');

        self.indented(|dumper| {
            dumper.parameters(&declaration.parameters);

            dumper.heading("ReturnType");
            dumper.indented(|dumper| dumper.type_syntax(&declaration.return_type));
        });
    }

    fn definition(&mut self, definition: &ResolvedFunctionDefinition) {
        self.line(
            &format!("Definition {}", definition.function),
            definition.span,
        );

        self.indented(|dumper| {
            dumper.locals(&definition.locals);
            dumper.block(&definition.body);
        });
    }

    fn parameters(&mut self, parameters: &[ResolvedParameter]) {
        self.heading("Parameters");
        self.indented(|dumper| {
            for parameter in parameters {
                dumper.write_indentation();
                let _ = write!(dumper.output, "Parameter {} ", parameter.id);
                write_quoted(&mut dumper.output, &parameter.name);
                write_span(&mut dumper.output, parameter.span);
                dumper.output.push('\n');
                dumper.indented(|dumper| {
                    dumper.parameter_binding_mode(parameter.binding_mode);
                    dumper.type_syntax(&parameter.type_syntax);
                });
            }
        });
    }

    fn parameter_binding_mode(&mut self, mode: ResolvedParameterBindingMode) {
        match mode {
            ResolvedParameterBindingMode::Value => self.heading("Binding Value"),
            ResolvedParameterBindingMode::ReadOnlyAlias { ref_span } => {
                self.heading("Binding ReadOnlyAlias");
                self.indented(|dumper| dumper.line("Ref", ref_span));
            }
            ResolvedParameterBindingMode::MutableAlias { mut_span, ref_span } => {
                self.heading("Binding MutableAlias");
                self.indented(|dumper| {
                    dumper.line("Mut", mut_span);
                    dumper.line("Ref", ref_span);
                });
            }
        }
    }

    fn locals(&mut self, locals: &[ResolvedLocal]) {
        self.heading("Locals");
        self.indented(|dumper| {
            for local in locals {
                dumper.write_indentation();
                let _ = write!(dumper.output, "Local {} ", local.id);
                write_quoted(&mut dumper.output, &local.name);
                write_span(&mut dumper.output, local.span);
                dumper.output.push('\n');
                dumper.indented(|dumper| dumper.type_syntax(&local.type_syntax));
            }
        });
    }

    fn type_syntax(&mut self, type_syntax: &ResolvedType) {
        let name = match type_syntax.kind {
            ResolvedTypeKind::I64 => "I64",
            ResolvedTypeKind::U64 => "U64",
            ResolvedTypeKind::U8 => "U8",
            ResolvedTypeKind::F64 => "F64",
            ResolvedTypeKind::Bool => "Bool",
            ResolvedTypeKind::Unit => "Unit",
            ResolvedTypeKind::Obj => "Obj",
            ResolvedTypeKind::Class(class) => {
                self.line(&format!("Type Class {class}"), type_syntax.span);
                return;
            }
            ResolvedTypeKind::Interface(interface) => {
                self.line(&format!("Type Interface {interface}"), type_syntax.span);
                return;
            }
            ResolvedTypeKind::Function(function) => {
                self.line(
                    &format!(
                        "Type Function {function} {}",
                        self.render_semantic_type_kind(ResolvedTypeKind::Function(function))
                    ),
                    type_syntax.span,
                );
                return;
            }
            ResolvedTypeKind::Array(array) => {
                self.line(&format!("Type Array {array}"), type_syntax.span);
                return;
            }
            ResolvedTypeKind::Shared(target) => {
                self.line(
                    &format!("Type Shared {}", self.render_shared_target(target)),
                    type_syntax.span,
                );
                return;
            }
            ResolvedTypeKind::Optional(optional) => {
                self.line(
                    &format!(
                        "Type Optional {optional} {}",
                        self.render_type_kind(ResolvedTypeKind::Optional(optional))
                    ),
                    type_syntax.span,
                );
                return;
            }
        };
        self.line(&format!("Type {name}"), type_syntax.span);
    }

    fn block(&mut self, block: &ResolvedBlock) {
        self.line("Block", block.span);
        self.indented(|dumper| {
            for statement in &block.statements {
                dumper.statement(statement);
            }
        });
    }

    fn statement(&mut self, statement: &ResolvedStatement) {
        match statement {
            ResolvedStatement::BaseInitialization(statement) => {
                self.line(
                    &format!("BaseInitialization {}", statement.base),
                    statement.span,
                );
                self.indented(|dumper| {
                    dumper.line("Super", statement.super_span);
                    dumper.heading("Arguments");
                    dumper.indented(|dumper| {
                        for argument in &statement.arguments {
                            dumper.expression(argument);
                        }
                    });
                });
            }
            ResolvedStatement::Local(local) => {
                self.line(&format!("LocalDeclaration {}", local.local), local.span);
                self.indented(|dumper| dumper.expression(&local.initializer));
            }
            ResolvedStatement::Return(statement) => {
                self.line("Return", statement.span);
                if let Some(value) = &statement.value {
                    self.indented(|dumper| dumper.expression(value));
                }
            }
            ResolvedStatement::Break(statement) => {
                self.line(&format!("Break {}", statement.target), statement.span);
            }
            ResolvedStatement::Continue(statement) => {
                self.line(&format!("Continue {}", statement.target), statement.span);
            }
            ResolvedStatement::Expression(statement) => {
                self.line("ExpressionStatement", statement.span);
                self.indented(|dumper| dumper.expression(&statement.expression));
            }
            ResolvedStatement::Conditional(statement) => self.conditional(statement),
            ResolvedStatement::While(statement) => {
                self.line(&format!("While {}", statement.loop_id), statement.span);
                self.indented(|dumper| {
                    dumper.heading("Condition");
                    dumper.indented(|dumper| dumper.expression(&statement.condition));
                    dumper.block(&statement.body);
                });
            }
            ResolvedStatement::Block(block) => self.block(block),
            ResolvedStatement::ScalarBindingAssignment(assignment) => {
                self.line(
                    &format!("ScalarBindingAssignment {}", assignment.destination),
                    assignment.span,
                );
                self.indented(|dumper| {
                    dumper.line("Equal", assignment.equal_span);
                    dumper.expression(&assignment.source);
                });
            }
            ResolvedStatement::FieldAssignment(assignment) => {
                self.line(
                    &format!("FieldAssignment {}", assignment.field),
                    assignment.span,
                );
                self.indented(|dumper| {
                    dumper.object_receiver(&assignment.receiver);
                    dumper.line("Equal", assignment.equal_span);
                    dumper.heading("Value");
                    dumper.indented(|dumper| dumper.expression(&assignment.value));
                });
            }
            ResolvedStatement::StaticFieldAssignment(assignment) => {
                self.line(
                    &format!("StaticFieldAssignment {}", assignment.field),
                    assignment.span,
                );
                self.indented(|dumper| {
                    dumper.line("Equal", assignment.equal_span);
                    dumper.heading("Value");
                    dumper.indented(|dumper| dumper.expression(&assignment.value));
                });
            }
            ResolvedStatement::ObjectAssignment(assignment) => {
                self.line("ObjectAssignment", assignment.span);
                self.indented(|dumper| {
                    dumper.heading("Destination");
                    dumper.indented(|dumper| dumper.object_place(&assignment.destination));
                    dumper.heading("Source");
                    dumper.indented(|dumper| dumper.expression(&assignment.source));
                });
            }
            ResolvedStatement::SharedAssignment(assignment) => {
                self.line(
                    &format!("SharedAssignment {}", assignment.destination),
                    assignment.span,
                );
                self.indented(|dumper| {
                    dumper.line("Equal", assignment.equal_span);
                    dumper.expression(&assignment.source);
                });
            }
            ResolvedStatement::OptionalAssignment(assignment) => {
                self.line(
                    &format!(
                        "OptionalAssignment {} type {}",
                        assignment.destination,
                        self.render_type_kind(assignment.target)
                    ),
                    assignment.span,
                );
                self.indented(|dumper| {
                    dumper.line("Equal", assignment.equal_span);
                    dumper.expression(&assignment.source);
                });
            }
            ResolvedStatement::ArrayAssignment(assignment) => {
                self.line("ArrayAssignment", assignment.span);
                self.indented(|dumper| {
                    dumper.heading("Destination");
                    dumper.indented(|dumper| dumper.expression(&assignment.destination));
                    dumper.line("Equal", assignment.equal_span);
                    dumper.heading("Source");
                    dumper.indented(|dumper| dumper.expression(&assignment.source));
                });
            }
        }
    }

    fn conditional(&mut self, statement: &ResolvedConditional) {
        self.line("Conditional", statement.span);
        self.indented(|dumper| {
            for (index, arm) in statement.arms.iter().enumerate() {
                dumper.line(if index == 0 { "IfArm" } else { "ElifArm" }, arm.span);
                dumper.indented(|dumper| {
                    dumper.heading("Condition");
                    dumper.indented(|dumper| dumper.expression(&arm.condition));
                    dumper.block(&arm.body);
                });
            }
            if let Some(block) = &statement.else_block {
                dumper.heading("ElseArm");
                dumper.indented(|dumper| dumper.block(block));
            }
        });
    }

    fn expression(&mut self, expression: &ResolvedExpression) {
        match expression {
            ResolvedExpression::Absent(absent) => self.line("Absent", absent.span),
            ResolvedExpression::Present(present) => {
                self.line("Present", present.span);
                self.indented(|dumper| dumper.expression(&present.value));
            }
            ResolvedExpression::Binding(binding) => {
                self.line(&format!("Binding {}", binding.binding), binding.span);
            }
            ResolvedExpression::FunctionReference(reference) => {
                self.line(
                    &format!(
                        "FunctionReference {} type {}",
                        reference.target, reference.function_type
                    ),
                    reference.span,
                );
            }
            ResolvedExpression::IndirectCall(call) => {
                self.line(
                    &format!("IndirectCall type {}", call.function_type),
                    call.span,
                );
                self.indented(|dumper| {
                    dumper.heading("Callee");
                    dumper.indented(|dumper| dumper.expression(&call.callee));
                    dumper.heading("Arguments");
                    dumper.indented(|dumper| {
                        for argument in &call.arguments {
                            dumper.expression(argument);
                        }
                    });
                });
            }
            ResolvedExpression::StaticFieldAccess(access) => {
                self.line(&format!("StaticFieldAccess {}", access.field), access.span);
            }
            ResolvedExpression::NumericLiteral(literal) => {
                self.write_indentation();
                self.output.push_str(match literal.kind {
                    crate::literal::NumericLiteralKind::I64(_) => "Integer ",
                    crate::literal::NumericLiteralKind::U64(_) => "U64 ",
                    crate::literal::NumericLiteralKind::U8(_) => "U8 ",
                    crate::literal::NumericLiteralKind::F64 => "F64 ",
                });
                write_quoted(&mut self.output, &literal.spelling);
                write_span(&mut self.output, literal.span);
                self.output.push('\n');
            }
            ResolvedExpression::ByteLiteral(literal) => {
                self.line(&format!("Byte {:02x}", literal.value), literal.span);
            }
            ResolvedExpression::StringLiteral(literal) => {
                self.line(
                    &format!("StringLiteral {} class {}", literal.data, literal.class),
                    literal.span,
                );
            }
            ResolvedExpression::Boolean(boolean) => {
                self.line(
                    if boolean.value {
                        "Boolean true"
                    } else {
                        "Boolean false"
                    },
                    boolean.span,
                );
            }
            ResolvedExpression::Unary(unary) => {
                let operator = match unary.operator {
                    ResolvedUnaryOperator::Negate => "Negate",
                    ResolvedUnaryOperator::LogicalNot => "LogicalNot",
                    ResolvedUnaryOperator::BitwiseComplement => "BitwiseComplement",
                };
                self.line(&format!("Unary {operator}"), unary.span);
                self.indented(|dumper| dumper.expression(&unary.operand));
            }
            ResolvedExpression::Dereference(dereference) => {
                self.dereference(dereference);
            }
            ResolvedExpression::Binary(binary) => {
                let operator = match binary.operator {
                    ResolvedBinaryOperator::Add => "Add",
                    ResolvedBinaryOperator::Subtract => "Subtract",
                    ResolvedBinaryOperator::Multiply => "Multiply",
                    ResolvedBinaryOperator::Divide => "Divide",
                    ResolvedBinaryOperator::Remainder => "Remainder",
                    ResolvedBinaryOperator::ShiftLeft => "ShiftLeft",
                    ResolvedBinaryOperator::ShiftRight => "ShiftRight",
                    ResolvedBinaryOperator::BitwiseAnd => "BitwiseAnd",
                    ResolvedBinaryOperator::BitwiseOr => "BitwiseOr",
                    ResolvedBinaryOperator::BitwiseXor => "BitwiseXor",
                    ResolvedBinaryOperator::Equal => "Equal",
                    ResolvedBinaryOperator::NotEqual => "NotEqual",
                    ResolvedBinaryOperator::LessThan => "LessThan",
                    ResolvedBinaryOperator::LessEqual => "LessEqual",
                    ResolvedBinaryOperator::GreaterThan => "GreaterThan",
                    ResolvedBinaryOperator::GreaterEqual => "GreaterEqual",
                };
                self.line(&format!("Binary {operator}"), binary.span);
                self.indented(|dumper| {
                    dumper.expression(&binary.left);
                    dumper.expression(&binary.right);
                });
            }
            ResolvedExpression::Logical(logical) => {
                let operator = match logical.operator {
                    ResolvedLogicalOperator::And => "And",
                    ResolvedLogicalOperator::Or => "Or",
                };
                self.line(&format!("Logical {operator}"), logical.span);
                self.indented(|dumper| {
                    dumper.expression(&logical.left);
                    dumper.expression(&logical.right);
                });
            }
            ResolvedExpression::TypeTest(test) => {
                self.line(
                    &format!(
                        "TypeTest target {}",
                        self.render_type_kind(test.target.kind)
                    ),
                    test.span,
                );
                self.indented(|dumper| dumper.expression(&test.source));
            }
            ResolvedExpression::PresenceTest(test) => {
                let kind = match test.kind {
                    ResolvedPresenceTestKind::Some => "Some",
                    ResolvedPresenceTestKind::None => "None",
                };
                self.line(&format!("PresenceTest {kind}"), test.span);
                self.indented(|dumper| {
                    dumper.expression(&test.source);
                    dumper.line("Is", test.is_span);
                    dumper.line(kind, test.target_span);
                });
            }
            ResolvedExpression::Unwrap(unwrap) => {
                self.line("Unwrap", unwrap.span);
                self.indented(|dumper| {
                    dumper.expression(&unwrap.source);
                    dumper.line("Bang", unwrap.bang_span);
                });
            }
            ResolvedExpression::PrimitiveCast(cast) => {
                self.line(
                    &format!("PrimitiveCast target {}", cast.target.name()),
                    cast.span,
                );
                self.indented(|dumper| dumper.expression(&cast.source));
            }
            ResolvedExpression::ObjectCast(cast) => {
                let mode = match cast.target_mode {
                    ResolvedObjectCastTargetMode::Plain => "ObjectCast",
                    ResolvedObjectCastTargetMode::Shared { .. } => "SharedObjectCast",
                };
                self.line(
                    &format!("{mode} target {}", self.render_type_kind(cast.target.kind)),
                    cast.span,
                );
                self.indented(|dumper| dumper.expression(&cast.source));
            }
            ResolvedExpression::Allocation(allocation) => {
                let mode = match &allocation.mode {
                    ResolvedConstructionMode::Initialize { .. } => "Allocate",
                    ResolvedConstructionMode::Copy { .. } => "CopyAllocate",
                };
                self.line(&format!("{mode} {}", allocation.class), allocation.span);
                self.indented(|dumper| match &allocation.mode {
                    ResolvedConstructionMode::Initialize { arguments } => {
                        for argument in arguments {
                            dumper.expression(argument);
                        }
                    }
                    ResolvedConstructionMode::Copy { copy_span, source } => {
                        dumper.line("Copy", *copy_span);
                        dumper.heading("Source");
                        dumper.indented(|dumper| dumper.expression(source));
                    }
                });
            }
            ResolvedExpression::OptionalBoxAllocation(allocation) => {
                self.line(
                    &format!(
                        "OptionalBoxAllocate exact {} target {}",
                        allocation.exact_optional, allocation.target
                    ),
                    allocation.span,
                );
                self.indented(|dumper| {
                    dumper.line("New", allocation.new_span);
                    dumper.line("Target", allocation.target_span);
                    match &allocation.initializer {
                        ResolvedOptionalBoxInitializer::Absent {
                            left_paren_span,
                            right_paren_span,
                        } => {
                            dumper.line("LeftParen", *left_paren_span);
                            dumper.line("RightParen", *right_paren_span);
                        }
                        ResolvedOptionalBoxInitializer::Value {
                            left_paren_span,
                            value,
                            right_paren_span,
                        } => {
                            dumper.line("LeftParen", *left_paren_span);
                            dumper.heading("Initializer");
                            dumper.indented(|dumper| dumper.expression(value));
                            dumper.line("RightParen", *right_paren_span);
                        }
                    }
                });
            }
            ResolvedExpression::ArrayConstruction(construction) => {
                self.line(
                    &format!(
                        "ArrayConstruction {} {}",
                        if construction.new_span.is_some() {
                            "shared"
                        } else {
                            "inline"
                        },
                        self.render_type_kind(construction.array_type.kind)
                    ),
                    construction.span,
                );
                self.indented(|dumper| match &construction.arguments {
                    ResolvedArrayConstructionArguments::Empty { .. } => {
                        dumper.heading("Empty");
                    }
                    ResolvedArrayConstructionArguments::Length { length, .. } => {
                        dumper.heading("Length");
                        dumper.indented(|dumper| dumper.expression(length));
                    }
                    ResolvedArrayConstructionArguments::Copy {
                        copy_span, source, ..
                    } => {
                        dumper.line("Copy", *copy_span);
                        dumper.indented(|dumper| dumper.expression(source));
                    }
                    ResolvedArrayConstructionArguments::Elements(list) => {
                        dumper.line("Elements", list.left_brace_span);
                        dumper.indented(|dumper| {
                            for (index, element) in list.elements.iter().enumerate() {
                                dumper.expression(element);
                                if let Some(comma_span) = list.comma_spans.get(index) {
                                    dumper.line("Comma", *comma_span);
                                }
                            }
                        });
                        dumper.line("RightBrace", list.right_brace_span);
                    }
                });
            }
            ResolvedExpression::ArrayLength(length) => {
                self.line(
                    match length.operator {
                        crate::resolve::ResolvedArrayLengthOperator::Ordinary { .. } => {
                            "ArrayLength"
                        }
                        crate::resolve::ResolvedArrayLengthOperator::Shared { .. } => {
                            "SharedArrayLength"
                        }
                    },
                    length.span,
                );
                self.indented(|dumper| {
                    dumper.expression(&length.receiver);
                    for argument in &length.arguments {
                        dumper.expression(argument);
                    }
                });
            }
            ResolvedExpression::DirectCall(call) => {
                self.line(&format!("DirectCall {}", call.function), call.span);
                self.indented(|dumper| {
                    for argument in &call.arguments {
                        dumper.expression(argument);
                    }
                });
            }
            ResolvedExpression::StaticCall(call) => {
                self.line(&format!("StaticCall {}", call.method), call.span);
                self.indented(|dumper| {
                    for argument in &call.arguments {
                        dumper.expression(argument);
                    }
                });
            }
            ResolvedExpression::Grouped(grouped) => {
                self.line("Grouped", grouped.span);
                self.indented(|dumper| dumper.expression(&grouped.expression));
            }
            ResolvedExpression::FieldAccess(access) => {
                self.line(&format!("FieldAccess {}", access.field), access.span);
                self.indented(|dumper| dumper.object_receiver(&access.receiver));
            }
            ResolvedExpression::ArrayProjection(projection) => {
                self.line(
                    match projection.operator {
                        ResolvedArrayProjectionOperator::Ordinary { .. } => "ArrayProjection",
                        ResolvedArrayProjectionOperator::Shared { .. } => "SharedArrayProjection",
                    },
                    projection.span,
                );
                self.indented(|dumper| {
                    dumper.expression(&projection.receiver);
                    match &projection.bounds {
                        ResolvedArrayProjectionBounds::Index(index) => {
                            dumper.heading("Index");
                            dumper.indented(|dumper| dumper.expression(index));
                        }
                        ResolvedArrayProjectionBounds::Slice { start, end, .. } => {
                            dumper.heading("Slice");
                            dumper.indented(|dumper| {
                                if let Some(start) = start {
                                    dumper.heading("Start");
                                    dumper.indented(|dumper| dumper.expression(start));
                                }
                                if let Some(end) = end {
                                    dumper.heading("End");
                                    dumper.indented(|dumper| dumper.expression(end));
                                }
                            });
                        }
                    }
                });
            }
            ResolvedExpression::MethodCall(call) => {
                self.line(&format!("MethodCall {}", call.method), call.span);
                self.indented(|dumper| {
                    dumper.object_receiver(&call.receiver);
                    dumper.heading("Arguments");
                    dumper.indented(|dumper| {
                        for argument in &call.arguments {
                            dumper.expression(argument);
                        }
                    });
                });
            }
            ResolvedExpression::InterfaceCall(call) => {
                let receiver = match &call.receiver {
                    ResolvedInterfaceReceiver::Binding { binding, .. } => {
                        format!("{binding}")
                    }
                    ResolvedInterfaceReceiver::Object(_) => "exact object".to_owned(),
                    ResolvedInterfaceReceiver::Cast(_) => "checked-cast".to_owned(),
                    ResolvedInterfaceReceiver::Dereference(_) => "dereference".to_owned(),
                    ResolvedInterfaceReceiver::OptionalBoxPayload(_) => {
                        "optional-box payload".to_owned()
                    }
                };
                self.line(
                    &format!(
                        "InterfaceCall {} {} receiver {}",
                        call.interface, call.requirement, receiver
                    ),
                    call.span,
                );
                self.indented(|dumper| {
                    match &call.receiver {
                        ResolvedInterfaceReceiver::Object(receiver) => {
                            dumper.object_receiver(receiver)
                        }
                        ResolvedInterfaceReceiver::Dereference(dereference) => {
                            dumper.dereference(dereference)
                        }
                        ResolvedInterfaceReceiver::Binding { .. }
                        | ResolvedInterfaceReceiver::Cast(_)
                        | ResolvedInterfaceReceiver::OptionalBoxPayload(_) => {}
                    }
                    for argument in &call.arguments {
                        dumper.expression(argument);
                    }
                });
            }
            ResolvedExpression::Construct(construct) => match &construct.mode {
                ResolvedConstructionMode::Initialize { arguments } => {
                    self.line(&format!("Construct {}", construct.class), construct.span);
                    self.indented(|dumper| {
                        for argument in arguments {
                            dumper.expression(argument);
                        }
                    });
                }
                ResolvedConstructionMode::Copy { copy_span, source } => {
                    self.line(
                        &format!("CopyConstruct {}", construct.class),
                        construct.span,
                    );
                    self.indented(|dumper| {
                        dumper.line("Copy", *copy_span);
                        dumper.heading("Source");
                        dumper.indented(|dumper| dumper.expression(source));
                    });
                }
            },
        }
    }

    fn object_place(&mut self, place: &ResolvedObjectPlace) {
        self.line(
            &format!("Receiver {} class {}", place.render_identity(), place.class),
            place.span,
        );
    }

    fn object_receiver(&mut self, receiver: &ResolvedObjectReceiver) {
        match receiver {
            ResolvedObjectReceiver::BindingPath(path) => self.object_place(path),
            ResolvedObjectReceiver::StaticField {
                field,
                projections,
                class,
                span,
            } => {
                self.line(&format!("StaticFieldReceiver {field} class {class}"), *span);
                self.indented(|dumper| {
                    for projection in projections {
                        match projection {
                            crate::object_path::ObjectProjection::Base(base) => {
                                dumper.heading(&format!("BaseProjection {base}"));
                            }
                            crate::object_path::ObjectProjection::Field(field) => {
                                dumper.heading(&format!("FieldProjection {field}"));
                            }
                        }
                    }
                });
            }
            ResolvedObjectReceiver::CastRelative {
                cast,
                projections,
                class,
                span,
            } => {
                self.line(&format!("CastRelativeReceiver class {class}"), *span);
                self.indented(|dumper| {
                    dumper.line(
                        &format!("CastTarget {}", dumper.render_type_kind(cast.target.kind)),
                        cast.target_span,
                    );
                    dumper.heading("Source");
                    dumper.indented(|dumper| dumper.expression(&cast.source));
                    for projection in projections {
                        match projection {
                            crate::object_path::ObjectProjection::Base(base) => {
                                dumper.heading(&format!("BaseProjection {base}"));
                            }
                            crate::object_path::ObjectProjection::Field(field) => {
                                dumper.heading(&format!("FieldProjection {field}"));
                            }
                        }
                    }
                });
            }
            ResolvedObjectReceiver::Dereference {
                dereference,
                projections,
                class,
                span,
            } => {
                self.line(&format!("DereferenceReceiver class {class}"), *span);
                self.indented(|dumper| {
                    dumper.dereference(dereference);
                    for projection in projections {
                        match projection {
                            crate::object_path::ObjectProjection::Base(base) => {
                                dumper.heading(&format!("BaseProjection {base}"));
                            }
                            crate::object_path::ObjectProjection::Field(field) => {
                                dumper.heading(&format!("FieldProjection {field}"));
                            }
                        }
                    }
                });
            }
            ResolvedObjectReceiver::OptionalPayload {
                unwrap,
                projections,
                class,
                span,
            } => {
                self.line(&format!("OptionalPayloadReceiver class {class}"), *span);
                self.indented(|dumper| {
                    dumper.heading("Optional");
                    dumper.indented(|dumper| dumper.expression(&unwrap.source));
                    for projection in projections {
                        match projection {
                            crate::object_path::ObjectProjection::Base(base) => {
                                dumper.heading(&format!("BaseProjection {base}"));
                            }
                            crate::object_path::ObjectProjection::Field(field) => {
                                dumper.heading(&format!("FieldProjection {field}"));
                            }
                        }
                    }
                });
            }
            ResolvedObjectReceiver::ArrayElement {
                projection,
                projections,
                class,
                span,
            } => {
                self.line(&format!("ArrayElementReceiver class {class}"), *span);
                self.indented(|dumper| {
                    dumper.expression(&ResolvedExpression::ArrayProjection(projection.clone()));
                    for projection in projections {
                        match projection {
                            crate::object_path::ObjectProjection::Base(base) => {
                                dumper.heading(&format!("BaseProjection {base}"));
                            }
                            crate::object_path::ObjectProjection::Field(field) => {
                                dumper.heading(&format!("FieldProjection {field}"));
                            }
                        }
                    }
                });
            }
            ResolvedObjectReceiver::Produced {
                producer,
                exact_class,
                projections,
                class,
                span,
            } => {
                self.line(
                    &format!("ProducedReceiver class {class} complete {exact_class}"),
                    *span,
                );
                self.indented(|dumper| {
                    dumper.heading("Producer");
                    dumper.indented(|dumper| dumper.expression(producer));
                    for projection in projections {
                        match projection {
                            crate::object_path::ObjectProjection::Base(base) => {
                                dumper.heading(&format!("BaseProjection {base}"));
                            }
                            crate::object_path::ObjectProjection::Field(field) => {
                                dumper.heading(&format!("FieldProjection {field}"));
                            }
                        }
                    }
                });
            }
        }
    }

    fn dereference(&mut self, dereference: &ResolvedDereferenceExpr) {
        let operator = match dereference.operator {
            ResolvedDereferenceOperator::Star => "Star",
            ResolvedDereferenceOperator::Arrow => "Arrow",
        };
        self.line(
            &format!(
                "Dereference {operator} target {}",
                self.render_shared_target(dereference.target)
            ),
            dereference.span,
        );
        self.indented(|dumper| dumper.expression(&dereference.source));
    }

    fn heading(&mut self, name: &str) {
        self.write_indentation();
        let _ = writeln!(self.output, "{name}");
    }

    fn raw_line(&mut self, text: &str) {
        self.write_indentation();
        let _ = writeln!(self.output, "{text}");
    }

    fn line(&mut self, name: &str, span: Span) {
        self.write_indentation();
        self.output.push_str(name);
        write_span(&mut self.output, span);
        self.output.push('\n');
    }

    fn write_indentation(&mut self) {
        write_indentation(&mut self.output, self.indentation);
    }

    fn indented(&mut self, write_contents: impl FnOnce(&mut Self)) {
        self.indentation += 1;
        write_contents(self);
        self.indentation -= 1;
    }

    fn render_type_kind(&self, kind: ResolvedTypeKind) -> String {
        match kind {
            ResolvedTypeKind::I64 => "i64".to_owned(),
            ResolvedTypeKind::U64 => "u64".to_owned(),
            ResolvedTypeKind::U8 => "u8".to_owned(),
            ResolvedTypeKind::F64 => "f64".to_owned(),
            ResolvedTypeKind::Bool => "bool".to_owned(),
            ResolvedTypeKind::Unit => "unit".to_owned(),
            ResolvedTypeKind::Obj => "Obj".to_owned(),
            ResolvedTypeKind::Class(class) => format!("class {class}"),
            ResolvedTypeKind::Interface(interface) => format!("interface {interface}"),
            ResolvedTypeKind::Function(function) => {
                let function = self
                    .program
                    .function_types
                    .get(function)
                    .expect("resolved function-type identities must name table entries");
                let parameters = function
                    .parameters
                    .iter()
                    .map(|parameter| {
                        format!(
                            "{}{}",
                            function_parameter_mode_prefix(parameter.mode),
                            self.render_type_kind(parameter.type_syntax.kind)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "fn({parameters}) -> {}",
                    self.render_type_kind(function.result.kind)
                )
            }
            ResolvedTypeKind::Array(array) => format!("array {array}"),
            ResolvedTypeKind::Shared(target) => {
                format!("shared {}", self.render_shared_target(target))
            }
            ResolvedTypeKind::Optional(optional) => {
                let payload = self
                    .program
                    .optional_types
                    .get(optional)
                    .expect("resolved optional identities must name table entries");
                let name = self.render_type_kind(payload.payload.kind);
                if matches!(
                    payload.payload.kind,
                    ResolvedTypeKind::Shared(_) | ResolvedTypeKind::Function(_)
                ) {
                    format!("({name})?")
                } else {
                    format!("{name}?")
                }
            }
        }
    }

    fn render_semantic_type_kind(&self, kind: ResolvedTypeKind) -> String {
        ResolvedTypeNameRenderer::new(self).render(kind)
    }

    fn render_specialization_key(&self, key: &GenericClassInstanceKey) -> String {
        let arguments = ResolvedTypeNameRenderer::new(self).render_list(&key.arguments);
        format!("{}<{arguments}>", self.template_name(key.template))
    }

    fn render_interface_specialization_key(&self, key: &GenericInterfaceInstanceKey) -> String {
        let arguments = ResolvedTypeNameRenderer::new(self).render_list(&key.arguments);
        let name = self
            .program
            .interface_templates
            .get(key.template)
            .map_or_else(
                || key.template.to_string(),
                |template| self.qualified_declaration_name(template.module, &template.name),
            );
        format!("{name}<{arguments}>")
    }

    fn render_cross_kind_specialization_key(&self, key: &GenericSpecializationKey) -> String {
        match key {
            GenericSpecializationKey::Class(key) => self.render_specialization_key(key),
            GenericSpecializationKey::Interface(key) => {
                self.render_interface_specialization_key(key)
            }
        }
    }

    fn render_shared_target(&self, target: ResolvedSharedTarget) -> String {
        match target.category() {
            ResolvedSharedTargetCategory::Object(ResolvedObjectTarget::Obj) => "Obj".to_owned(),
            ResolvedSharedTargetCategory::Object(ResolvedObjectTarget::Class(class)) => {
                format!("class {class}")
            }
            ResolvedSharedTargetCategory::Object(ResolvedObjectTarget::Interface(interface)) => {
                format!("interface {interface}")
            }
            ResolvedSharedTargetCategory::Array(array) => format!("array {array}"),
            ResolvedSharedTargetCategory::OptionalBox(target) => {
                let metadata = self
                    .program
                    .optional_box_types
                    .get(target)
                    .expect("resolved optional-box identities must name table entries");
                format!(
                    "optional-box {target} exact {}",
                    metadata
                        .optional
                        .map(|optional| optional.to_string())
                        .unwrap_or_else(|| "view-only".to_owned())
                )
            }
        }
    }

    fn template_name(&self, template: ClassTemplateId) -> String {
        self.program.class_templates.get(template).map_or_else(
            || template.to_string(),
            |template| self.qualified_declaration_name(template.module, &template.name),
        )
    }

    fn qualified_declaration_name(&self, module: ModuleId, name: &str) -> String {
        if self.program.modules.len() == 1 {
            return name.to_owned();
        }
        self.program.modules.get(module).map_or_else(
            || name.to_owned(),
            |module| format!("{}::{name}", module.module_path()),
        )
    }
}

impl ResolvedTypeNameContext for ResolvedDumper<'_> {
    fn array(&self, id: ArrayTypeId) -> Option<&ResolvedArrayType> {
        self.program.array_types.get(id)
    }

    fn function(&self, id: FunctionTypeId) -> Option<&ResolvedFunctionType> {
        self.program.function_types.get(id)
    }

    fn optional(&self, id: OptionalTypeId) -> Option<&ResolvedOptionalType> {
        self.program.optional_types.get(id)
    }

    fn optional_box(&self, id: OptionalBoxTypeId) -> Option<&ResolvedOptionalBoxType> {
        self.program.optional_box_types.get(id)
    }

    fn direct_class_name(&self, id: ClassId) -> Option<String> {
        let declaration = self.program.class(id)?;
        if self.program.generic_specializations.for_class(id).is_some() {
            return Some(declaration.name.clone());
        }
        Some(self.qualified_declaration_name(declaration.module, &declaration.name))
    }

    fn class_specialization(&self, id: ClassId) -> Option<&GenericClassInstanceKey> {
        self.program
            .generic_specializations
            .for_class(id)
            .map(|specialization| &specialization.key)
    }

    fn template_name(&self, id: ClassTemplateId) -> Option<String> {
        self.program
            .class_templates
            .get(id)
            .map(|template| self.qualified_declaration_name(template.module, &template.name))
    }

    fn interface_name(&self, id: InterfaceId) -> Option<String> {
        self.program.interface(id).map(|declaration| {
            self.qualified_declaration_name(declaration.module, &declaration.name)
        })
    }
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
