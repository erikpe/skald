//! Resolved-program metadata and top-level rendering order.

use std::fmt::Write;

use crate::dump_format::{write_quoted, write_span};

use super::super::ir::*;
use super::ResolvedDumper;

pub(super) fn dump_program(program: &ResolvedProgram) -> String {
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
        if let Some(item) = &program.iterable_language_item {
            dumper.raw_line(&format!(
                "IterableLanguageItem template {} parameters {} {} requirements {} {}",
                item.template,
                item.item_parameter,
                item.state_parameter,
                item.iter_state_requirement,
                item.iter_next_requirement
            ));
        }
        if let Some(item) = &program.operator_language_item {
            dumper.heading("OperatorLanguageItem");
            dumper.indented(|dumper| {
                for protocol in item.iter() {
                    let parameters = match protocol.parameters {
                        ResolvedOperatorProtocolParameters::Unary { output } => {
                            format!("output {output}")
                        }
                        ResolvedOperatorProtocolParameters::Predicate { rhs } => {
                            format!("rhs {rhs}")
                        }
                        ResolvedOperatorProtocolParameters::Binary { rhs, output } => {
                            format!("rhs {rhs} output {output}")
                        }
                    };
                    dumper.raw_line(&format!(
                        "{} template {} {parameters} requirement {}",
                        protocol.kind.interface_name(),
                        protocol.template,
                        protocol.requirement
                    ));
                }
            });
            dumper.heading("PrimitiveOperatorEvidence");
            dumper.indented(|dumper| {
                for evidence in primitive_operator_registry() {
                    let protocol = item.get(evidence.protocol());
                    let application = match protocol.kind.shape() {
                        CanonicalOperatorProtocolShape::Unary => format!(
                            "{}<{}>",
                            protocol.kind.interface_name(),
                            evidence.output().name()
                        ),
                        CanonicalOperatorProtocolShape::Predicate => format!(
                            "{}<{}>",
                            protocol.kind.interface_name(),
                            evidence.rhs().expect("predicate evidence has an RHS").name()
                        ),
                        CanonicalOperatorProtocolShape::Binary => format!(
                            "{}<{}, {}>",
                            protocol.kind.interface_name(),
                            evidence.rhs().expect("binary evidence has an RHS").name(),
                            evidence.output().name()
                        ),
                    };
                    dumper.raw_line(&format!(
                        "{} canonical {} template {} -> {}",
                        evidence.receiver().name(),
                        application,
                        protocol.template,
                        evidence.operation.semantic_name()
                    ));
                }
            });
        }
        if let Some(item) = &program.range_language_item {
            dumper.raw_line(&format!(
                "RangeLanguageItem successor-template {} output-parameter {} requirement {} range-template {} parameter {} initializer-member {} bounds {} {} iterable-claim {}",
                item.successor_template,
                item.successor_output_parameter,
                item.successor_requirement,
                item.range_template,
                item.range_parameter,
                item.range_initializer_member,
                item.range_ordering_bound,
                item.range_successor_bound,
                item.range_iterable_claim
            ));
            dumper.heading("PrimitiveSuccessorEvidence");
            dumper.indented(|dumper| {
                for evidence in primitive_successor_registry() {
                    dumper.raw_line(&format!(
                        "{} canonical Successor<{}> template {} -> {}",
                        evidence.primitive().name(),
                        evidence.primitive().name(),
                        item.successor_template,
                        evidence.operation().semantic_name()
                    ));
                }
            });
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
        super::generic::dump_generic_metadata(dumper);
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
