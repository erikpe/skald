//! Class, interface, and function declarations and definitions.

use std::fmt::Write;

use crate::dump_format::{write_quoted, write_span};

use super::super::ir::*;
use super::generic::render_interface_type;
use super::ResolvedDumper;

impl ResolvedDumper<'_> {
    pub(super) fn interface_declaration(&mut self, interface: &ResolvedInterfaceDeclaration) {
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

    pub(super) fn class_declaration(
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
                for (index, selection) in specialization.closed_bound_members.iter().enumerate() {
                    if let Some(selection) = selection {
                        match selection {
                            ClosedGenericBoundMember::Interface {
                                interface,
                                requirement,
                            } => dumper.raw_line(&format!(
                                "ClosedBoundSelection {index} class-witness interface {interface} requirement {requirement}",
                            )),
                            ClosedGenericBoundMember::PrimitiveIntrinsic { operation } => {
                                dumper.raw_line(&format!(
                                    "ClosedBoundSelection {index} primitive-intrinsic {}",
                                    operation.semantic_name(),
                                ));
                            }
                        }
                    }
                }
                for (index, selection) in specialization
                    .closed_operator_selections
                    .iter()
                    .enumerate()
                {
                    if let Some(selection) = selection {
                        match selection {
                            ClosedGenericOperatorSelection::ClassWitness {
                                interface,
                                requirement,
                                ..
                            } => dumper.raw_line(&format!(
                                "ClosedOperatorSelection {index} class-witness interface {interface} requirement {requirement}",
                            )),
                            ClosedGenericOperatorSelection::PrimitiveIntrinsic { operation } => {
                                dumper.raw_line(&format!(
                                    "ClosedOperatorSelection {index} primitive-intrinsic {}",
                                    operation.semantic_name(),
                                ));
                            }
                        }
                    }
                }
                for (index, selection) in specialization
                    .closed_iteration_selections
                    .iter()
                    .enumerate()
                {
                    if let Some(selection) = selection {
                        dumper.raw_line(&format!(
                            "ClosedIterationSelection {index} interface {} item {} state {} iter_state {} iter_next {}",
                            selection.interface,
                            dumper.render_type_kind(selection.item),
                            dumper.render_type_kind(selection.state),
                            selection.iter_state,
                            selection.iter_next,
                        ));
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

    pub(super) fn class_definition(&mut self, class: &ResolvedClassDefinition) {
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

    pub(super) fn declaration(&mut self, declaration: &ResolvedFunctionDeclaration) {
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

    pub(super) fn definition(&mut self, definition: &ResolvedFunctionDefinition) {
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
}
