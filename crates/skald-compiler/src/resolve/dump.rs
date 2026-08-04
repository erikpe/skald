//! Deterministic textual rendering of the resolved program.

use std::fmt::Write;

use crate::{
    dump_format::{write_indentation, write_quoted, write_span},
    source::Span,
};

use super::ir::*;

pub fn dump_resolved(program: &ResolvedProgram) -> String {
    let mut dumper = ResolvedDumper::default();
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
                "StringLanguageItem class {} fields {} {} {}",
                item.class, item.storage_field, item.start_field, item.length_field
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
                                ResolvedTopLevelId::Interface(interface) => interface.to_string(),
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
                            ResolvedTopLevelId::Interface(interface) => interface.to_string(),
                        };
                        let _ = write!(dumper.output, "{visibility} {identity} ");
                        write_quoted(&mut dumper.output, &declaration.name);
                        write_span(&mut dumper.output, declaration.name_span);
                        dumper.output.push('\n');
                    }
                });
            }
        });
        dumper.write_indentation();
        match program.entry_function {
            Some(function) => {
                let _ = writeln!(dumper.output, "Entry {function}");
            }
            None => dumper.output.push_str("Entry <none>\n"),
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
                    dumper.class_declaration(class);
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

#[derive(Default)]
struct ResolvedDumper {
    output: String,
    indentation: usize,
}

impl ResolvedDumper {
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

    fn class_declaration(&mut self, class: &ResolvedClassDeclaration) {
        self.write_indentation();
        let _ = write!(self.output, "Class {} module {} ", class.id, class.module);
        write_quoted(&mut self.output, &class.name);
        write_span(&mut self.output, class.span);
        self.output.push('\n');
        self.indented(|dumper| {
            if let Some(base) = class.direct_base {
                dumper.line(&format!("DirectBase {}", base.class), base.span);
            }
            for claim in &class.implemented_interfaces {
                dumper.line(&format!("Implements {}", claim.interface), claim.span);
            }
            dumper.heading("Fields");
            dumper.indented(|dumper| {
                for field in &class.fields {
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Field {} ", field.id);
                    if field.visibility.private_span().is_some() {
                        dumper.output.push_str("private ");
                    }
                    write_quoted(&mut dumper.output, &field.name);
                    write_span(&mut dumper.output, field.span);
                    dumper.output.push('\n');
                    dumper.indented(|dumper| {
                        if let Some(span) = field.visibility.private_span() {
                            dumper.line("Private", span);
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
                        write_quoted(&mut dumper.output, &field.name);
                        write_span(&mut dumper.output, field.span);
                        dumper.output.push('\n');
                        dumper.indented(|dumper| {
                            if let Some(span) = field.visibility.private_span() {
                                dumper.line("Private", span);
                            }
                            dumper.line("Static", field.static_span);
                            dumper.type_syntax(&field.type_syntax);
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
            ResolvedTypeKind::Array(array) => {
                self.line(&format!("Type Array {array}"), type_syntax.span);
                return;
            }
            ResolvedTypeKind::Shared(target) => {
                self.line(
                    &format!("Type Shared {}", render_shared_target(target)),
                    type_syntax.span,
                );
                return;
            }
            ResolvedTypeKind::Optional {
                payload,
                payload_span,
                question_span,
            } => {
                self.line(
                    &format!("Type Optional {}", render_optional_payload(payload)),
                    type_syntax.span,
                );
                self.indented(|dumper| {
                    dumper.line("Payload", payload_span);
                    dumper.line("Question", question_span);
                });
                return;
            }
            ResolvedTypeKind::OptionalShared {
                target,
                shared_span,
                question_span,
                target_span,
            } => {
                self.line(
                    &format!("Type OptionalShared {}", render_shared_target(target)),
                    type_syntax.span,
                );
                self.indented(|dumper| {
                    dumper.line("Shared", shared_span);
                    dumper.line("Question", question_span);
                    dumper.line("Target", target_span);
                });
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
            ResolvedStatement::PrimitiveBindingAssignment(assignment) => {
                self.line(
                    &format!("PrimitiveBindingAssignment {}", assignment.destination),
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
                        render_type_kind(assignment.target)
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
            ResolvedExpression::Binding(binding) => {
                self.line(&format!("Binding {}", binding.binding), binding.span);
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
                    &format!("TypeTest target {}", render_type_kind(test.target.kind)),
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
                    &format!("{mode} target {}", render_type_kind(cast.target.kind)),
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
            ResolvedExpression::ArrayConstruction(construction) => {
                self.line(
                    &format!(
                        "ArrayConstruction {} {}",
                        if construction.new_span.is_some() {
                            "shared"
                        } else {
                            "inline"
                        },
                        render_type_kind(construction.array_type.kind)
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
                    ResolvedInterfaceReceiver::Cast(_) => "checked-cast".to_owned(),
                    ResolvedInterfaceReceiver::Dereference(_) => "dereference".to_owned(),
                };
                self.line(
                    &format!(
                        "InterfaceCall {} {} receiver {}",
                        call.interface, call.requirement, receiver
                    ),
                    call.span,
                );
                self.indented(|dumper| {
                    if let ResolvedInterfaceReceiver::Dereference(dereference) = &call.receiver {
                        dumper.dereference(dereference);
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
            ResolvedObjectReceiver::CastRelative {
                cast,
                projections,
                class,
                span,
            } => {
                self.line(&format!("CastRelativeReceiver class {class}"), *span);
                self.indented(|dumper| {
                    dumper.line(
                        &format!("CastTarget {}", render_type_kind(cast.target.kind)),
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
                render_shared_target(dereference.target)
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
}

fn render_type_kind(kind: ResolvedTypeKind) -> String {
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
        ResolvedTypeKind::Array(array) => format!("array {array}"),
        ResolvedTypeKind::Shared(target) => format!("shared {}", render_shared_target(target)),
        ResolvedTypeKind::Optional { payload, .. } => {
            format!("{}?", render_optional_payload(payload))
        }
        ResolvedTypeKind::OptionalShared { target, .. } => {
            format!("shared? {}", render_shared_target(target))
        }
    }
}

fn render_optional_payload(payload: ResolvedOptionalPayload) -> String {
    match payload {
        ResolvedOptionalPayload::I64 => "i64".to_owned(),
        ResolvedOptionalPayload::U64 => "u64".to_owned(),
        ResolvedOptionalPayload::U8 => "u8".to_owned(),
        ResolvedOptionalPayload::F64 => "f64".to_owned(),
        ResolvedOptionalPayload::Bool => "bool".to_owned(),
        ResolvedOptionalPayload::Class(class) => format!("class {class}"),
    }
}

fn render_shared_target(target: ResolvedSharedTarget) -> String {
    match target {
        ResolvedSharedTarget::Obj => "Obj".to_owned(),
        ResolvedSharedTarget::Class(class) => format!("class {class}"),
        ResolvedSharedTarget::Interface(interface) => format!("interface {interface}"),
        ResolvedSharedTarget::Array(array) => format!("array {array}"),
    }
}
