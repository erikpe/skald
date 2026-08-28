//! Deterministic textual rendering of typed HIR.

use std::fmt::{Display, Write};

use crate::{
    dump_format::{write_indentation, write_quoted, write_span},
    source::Span,
};

use super::ir::*;

pub fn dump_hir(program: &HirProgram) -> String {
    let mut dumper = HirDumper::new(
        &program.function_types,
        &program.optional_types,
        &program.optional_box_types,
    );
    dumper.line("HirProgram", program.span);
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
        if !program.optional_types.is_empty() {
            dumper.heading("OptionalTypes");
            dumper.indented(|dumper| {
                for optional in program.optional_types.iter() {
                    dumper.raw_line(&format!(
                        "OptionalType {} payload {} storage={:?} representation={:?}",
                        optional.id,
                        dumper.type_name(optional.payload),
                        optional.storage,
                        optional.representation,
                    ));
                    dumper.indented(|dumper| {
                        dumper.raw_line(&format!(
                            "Lifecycle initialization={:?} injection={:?} copy={:?} assignment={:?} destruction={:?} presence={:?} unwrap={:?} checked={:?}",
                            optional.lifecycle.initialization,
                            optional.lifecycle.injection,
                            optional.lifecycle.copy,
                            optional.lifecycle.assignment,
                            optional.lifecycle.destruction,
                            optional.lifecycle.presence_test,
                            optional.lifecycle.unwrap,
                            optional.checked_access,
                        ));
                        dumper.raw_line(&format!(
                            "Boundaries argument={:?} result={:?} static={:?} array-element={:?}",
                            optional.boundaries.argument,
                            optional.boundaries.result,
                            optional.boundaries.static_storage,
                            optional.boundaries.array_element,
                        ));
                    });
                }
            });
        }
        if !program.optional_box_types.is_empty() {
            dumper.heading("OptionalBoxTypes");
            dumper.indented(|dumper| {
                for target in program.optional_box_types.iter() {
                    dumper.raw_line(&format!(
                        "OptionalBoxType {} exact={} dynamic={:?} depth={} view={:?}",
                        target.id,
                        target
                            .exact_optional
                            .map(|optional| optional.to_string())
                            .unwrap_or_else(|| "view-only".to_owned()),
                        target.exact_dynamic_class,
                        target.optional_depth,
                        target.object_view,
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
        dumper.write_indentation();
        let _ = writeln!(dumper.output, "Entry {}", program.entry_function);
        if !program.function_types.is_empty() {
            dumper.heading("FunctionTypes");
            dumper.indented(|dumper| {
                for function in program.function_types.iter() {
                    dumper.line(
                        &format!(
                            "FunctionType {} -> {}",
                            function.id,
                            dumper.type_name(function.result)
                        ),
                        function.span,
                    );
                    dumper.indented(|dumper| {
                        for parameter in &function.parameters {
                            dumper.line(
                                &format!(
                                    "Parameter {:?} {}",
                                    parameter.mode,
                                    dumper.type_name(parameter.ty)
                                ),
                                parameter.span,
                            );
                        }
                    });
                }
            });
        }
        if !program.array_types.is_empty() {
            dumper.heading("ArrayTypes");
            dumper.indented(|dumper| {
                for array in program.array_types.iter() {
                    dumper.array_type(array);
                }
            });
        }
        if !program.classes.is_empty() {
            dumper.heading("Classes");
            dumper.indented(|dumper| {
                for class in program.classes.iter() {
                    dumper.class_declaration(class);
                }
            });
            dumper.heading("ClassDefinitions");
            dumper.indented(|dumper| {
                for class in program.class_definitions.iter() {
                    dumper.class_definition(class);
                }
            });
        }
        if !program.interfaces.is_empty() {
            dumper.heading("Interfaces");
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
    });
    dumper.output
}

struct HirDumper<'types> {
    output: String,
    indentation: usize,
    function_types: &'types HirFunctionTypeTable,
    optional_types: &'types HirOptionalTypeTable,
    optional_box_types: &'types HirOptionalBoxTypeTable,
}

impl<'types> HirDumper<'types> {
    fn new(
        function_types: &'types HirFunctionTypeTable,
        optional_types: &'types HirOptionalTypeTable,
        optional_box_types: &'types HirOptionalBoxTypeTable,
    ) -> Self {
        Self {
            output: String::new(),
            indentation: 0,
            function_types,
            optional_types,
            optional_box_types,
        }
    }

    fn type_name(&self, ty: Type) -> String {
        match ty {
            Type::Function(function) => {
                let function = self
                    .function_types
                    .get(function)
                    .expect("HIR dump function identity must name metadata");
                let parameters = function
                    .parameters
                    .iter()
                    .map(|parameter| {
                        let mode = match parameter.mode {
                            HirFunctionTypeParameterMode::Value => "",
                            HirFunctionTypeParameterMode::ReadOnlyAlias => "ref ",
                            HirFunctionTypeParameterMode::MutableAlias => "mut ref ",
                        };
                        format!("{mode}{}", self.type_name(parameter.ty))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("fn({parameters}) -> {}", self.type_name(function.result))
            }
            Type::Optional(optional) => {
                let payload = self
                    .optional_types
                    .get(optional)
                    .expect("HIR dump optional identity must name metadata")
                    .payload;
                let payload_name = self.type_name(payload);
                if matches!(payload, Type::Shared(_)) {
                    format!("({payload_name})?")
                } else {
                    format!("{payload_name}?")
                }
            }
            Type::Shared(HirSharedTarget::OptionalBox(target)) => {
                format!("shared {}", self.optional_box_target_name(target))
            }
            _ => ty.name().into_owned(),
        }
    }

    fn optional_box_target_name(&self, target: crate::identity::OptionalBoxTypeId) -> String {
        let target = self
            .optional_box_types
            .get(target)
            .expect("HIR dump optional-box identity must name metadata");
        if let Some(view) = target.object_view {
            let leaf = match view {
                HirViewTarget::Obj => "Obj".to_owned(),
                HirViewTarget::Class(class) => format!("class {class}"),
                HirViewTarget::Interface(interface) => format!("interface {interface}"),
            };
            return format!("{}{}", leaf, "?".repeat(target.optional_depth));
        }
        let optional = target
            .exact_optional
            .expect("non-object box target must retain an exact optional identity");
        self.type_name(Type::Optional(optional))
    }

    fn array_type(&mut self, array: &HirArrayType) {
        self.raw_line(&format!(
            "ArrayType {} element {}",
            array.id,
            self.type_name(array.element)
        ));
        self.indented(|dumper| {
            dumper.raw_line(&format!(
                "Default {}",
                array
                    .lifecycle
                    .default
                    .map(array_default_name)
                    .unwrap_or_else(|| "unavailable".to_owned())
            ));
            dumper.raw_line(&format!(
                "Copy {}",
                array
                    .lifecycle
                    .copy
                    .map(array_copy_name)
                    .unwrap_or_else(|| "unavailable".to_owned())
            ));
            dumper.raw_line(&format!(
                "Assignment {}",
                array
                    .lifecycle
                    .assignment
                    .map(array_assignment_name)
                    .unwrap_or_else(|| "unavailable".to_owned())
            ));
            dumper.raw_line(&format!(
                "Destruction {}",
                array_destruction_name(array.lifecycle.destruction)
            ));
        });
    }

    fn interface_declaration(&mut self, interface: &HirInterfaceDeclaration) {
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
                let access = match requirement.receiver_access {
                    HirAccess::ReadOnly => "readonly",
                    HirAccess::Mutable => "mutable",
                };
                dumper.write_indentation();
                let _ = write!(dumper.output, "Requirement {} {access} ", requirement.id);
                write_quoted(&mut dumper.output, &requirement.name);
                let _ = write!(dumper.output, " -> {}", requirement.return_type.name());
                write_span(&mut dumper.output, requirement.span);
                dumper.output.push('\n');
                dumper.indented(|dumper| {
                    for parameter in &requirement.parameters {
                        dumper.write_indentation();
                        let _ = write!(
                            dumper.output,
                            "Parameter {} ",
                            parameter_mode_name(parameter.mode)
                        );
                        write_quoted(&mut dumper.output, &parameter.name);
                        let _ = write!(dumper.output, " : {}", dumper.type_name(parameter.ty));
                        write_span(&mut dumper.output, parameter.span);
                        dumper.output.push('\n');
                    }
                });
            }
        });
    }

    fn class_declaration(&mut self, class: &HirClassDeclaration) {
        self.write_indentation();
        let _ = write!(self.output, "Class {} module {} ", class.id, class.module);
        write_quoted(&mut self.output, &class.name);
        write_span(&mut self.output, class.span);
        self.output.push('\n');
        self.indented(|dumper| {
            if let Some(base) = &class.direct_base {
                dumper.line(&format!("DirectBase {}", base.class), base.span);
            }
            if !class.conformances.is_empty() {
                dumper.heading("Conformances");
                dumper.indented(|dumper| {
                    for conformance in &class.conformances {
                        dumper.raw_line(&format!("Interface {}", conformance.interface));
                        dumper.indented(|dumper| {
                            for implementation in &conformance.implementations {
                                dumper.raw_line(&format!(
                                    "{} -> {}",
                                    implementation.requirement, implementation.method
                                ));
                            }
                        });
                    }
                });
            }
            dumper.heading("Fields");
            dumper.indented(|dumper| {
                for field in &class.fields {
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Field {} ", field.id);
                    if field.cell_span.is_some() {
                        dumper.output.push_str("cell ");
                    }
                    if field.final_span.is_some() {
                        dumper.output.push_str("final ");
                    }
                    write_quoted(&mut dumper.output, &field.name);
                    let _ = write!(dumper.output, " : {}", dumper.type_name(field.ty));
                    write_span(&mut dumper.output, field.span);
                    dumper.output.push('\n');
                    if let Some(span) = field.cell_span {
                        dumper.indented(|dumper| dumper.line("Cell", span));
                    }
                    if let Some(span) = field.final_span {
                        dumper.indented(|dumper| dumper.line("Final", span));
                    }
                }
            });
            if !class.static_fields.is_empty() {
                dumper.heading("StaticFields");
                dumper.indented(|dumper| {
                    for field in &class.static_fields {
                        dumper.write_indentation();
                        let _ = write!(dumper.output, "StaticField {} ", field.id);
                        if field.final_span.is_some() {
                            dumper.output.push_str("final ");
                        }
                        write_quoted(&mut dumper.output, &field.name);
                        let _ = write!(dumper.output, " : {}", dumper.type_name(field.ty));
                        write_span(&mut dumper.output, field.span);
                        dumper.output.push('\n');
                        dumper.indented(|dumper| {
                            if let Some(span) = field.final_span {
                                dumper.line("Final", span);
                            }
                            match &field.initializer {
                                Some(initializer) => {
                                    dumper.line(
                                        &format!(
                                            "DeclarationInitializer {} destination {}",
                                            initializer.id,
                                            dumper.type_name(field.ty)
                                        ),
                                        initializer.span,
                                    );
                                    dumper.indented(|dumper| {
                                        dumper.line("Equal", initializer.equal_span);
                                        dumper.stored_value_initialization(&initializer.value);
                                    });
                                }
                                None => dumper.raw_line("ZeroDefaultInitialization"),
                            }
                        });
                    }
                });
            }
            dumper.heading("Initializers");
            dumper.indented(|dumper| {
                for initializer in &class.initializers {
                    dumper.line(&format!("Initializer {}", initializer.id), initializer.span);
                    dumper.indented(|dumper| {
                        for parameter in &initializer.parameters {
                            dumper.parameter(parameter);
                        }
                    });
                }
            });
            dumper.heading("CopyConstructor");
            dumper.indented(|dumper| {
                dumper.copy_capability(&class.copy_constructor);
                if let Some(declaration) = &class.copy_constructor_declaration {
                    for parameter in &declaration.parameters {
                        dumper.parameter(parameter);
                    }
                }
            });
            dumper.heading("CopyAssignment");
            dumper.indented(|dumper| {
                dumper.copy_capability(&class.copy_assignment);
                if let Some(declaration) = &class.copy_assignment_declaration {
                    dumper.parameter(&declaration.parameter);
                }
            });
            if let Some(destructor) = &class.destructor {
                let access = match destructor.receiver_access {
                    HirAccess::ReadOnly => "readonly",
                    HirAccess::Mutable => "mutable",
                };
                dumper.line(
                    &format!("Destructor {} {access} -> unit", destructor.id),
                    destructor.span,
                );
            }
            if !class.destruction.steps.is_empty() {
                dumper.heading("DestructionPlan");
                dumper.indented(|dumper| {
                    for step in &class.destruction.steps {
                        match step {
                            HirDestructionStep::UserBody(destructor) => {
                                dumper.raw_line(&format!("UserBody {destructor}"));
                            }
                            HirDestructionStep::Field(field) => {
                                dumper.raw_line(&format!("Field {field}"));
                            }
                            HirDestructionStep::SharedField(field) => {
                                dumper.raw_line(&format!("SharedField {field}"));
                            }
                            HirDestructionStep::OptionalSharedField(field) => {
                                dumper.raw_line(&format!("OptionalSharedField {field}"));
                            }
                            HirDestructionStep::OptionalClassField(field) => {
                                dumper.raw_line(&format!("OptionalClassField {field}"));
                            }
                            HirDestructionStep::OptionalField { field, optional } => {
                                dumper.raw_line(&format!("OptionalField {field} {optional}"));
                            }
                            HirDestructionStep::ArrayField(field) => {
                                dumper.raw_line(&format!("ArrayField {field}"));
                            }
                            HirDestructionStep::Base(base) => {
                                dumper.raw_line(&format!("Base {base}"));
                            }
                        }
                    }
                });
            }
            dumper.heading("Methods");
            dumper.indented(|dumper| {
                for method in &class.methods {
                    let kind = match method.kind {
                        HirMethodKind::Instance {
                            receiver_access: HirAccess::ReadOnly,
                            ..
                        } => "readonly",
                        HirMethodKind::Instance {
                            receiver_access: HirAccess::Mutable,
                            ..
                        } => "mutable",
                        HirMethodKind::Static => "static",
                    };
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Method {} ", method.id);
                    write_quoted(&mut dumper.output, &method.name);
                    let _ = write!(dumper.output, " {kind} -> {}", method.return_type.name());
                    write_span(&mut dumper.output, method.span);
                    dumper.output.push('\n');
                    dumper.indented(|dumper| {
                        if let Some(dispatch) = method.kind.dispatch() {
                            dumper.method_dispatch(dispatch);
                        }
                        for parameter in &method.parameters {
                            dumper.parameter(parameter);
                        }
                    });
                }
            });
        });
    }

    fn method_dispatch(&mut self, dispatch: HirMethodDispatch) {
        match dispatch {
            HirMethodDispatch::Direct => {}
            HirMethodDispatch::VirtualRoot { family, slot } => {
                self.raw_line(&format!("Dispatch VirtualRoot {family} slot {slot}"));
            }
            HirMethodDispatch::Override {
                family,
                slot,
                root,
                overridden,
            } => self.raw_line(&format!(
                "Dispatch Override {family} slot {slot} root {root} overridden {overridden}"
            )),
        }
    }

    fn class_definition(&mut self, class: &HirClassDefinition) {
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

    fn member_definition(&mut self, definition: &HirMemberDefinition) {
        self.line(
            &format!("MemberDefinition {}", definition.callable),
            definition.span,
        );
        self.indented(|dumper| {
            dumper.locals(&definition.locals);
            dumper.block(&definition.body);
        });
    }

    fn copy_capability<I: Copy + Display>(&mut self, capability: &HirCopyCapability<I>) {
        match capability {
            HirCopyCapability::User(copy) => {
                self.raw_line(&format!("User {}", copy.operation));
                if let Some(base) = copy.base {
                    self.indented(|dumper| {
                        dumper.raw_line(&format!("Base {}", base.base));
                        dumper.indented(|dumper| dumper.selected_copy_operation(base.operation));
                    });
                }
            }
            HirCopyCapability::Unavailable => self.raw_line("Unavailable"),
            HirCopyCapability::Synthesized(operation) => {
                self.raw_line(&format!("Synthesized {}", operation.class));
                self.indented(|dumper| {
                    if let Some(base) = operation.base {
                        dumper.raw_line(&format!("Base {}", base.base));
                        dumper.indented(|dumper| dumper.selected_copy_operation(base.operation));
                    }
                    for field in &operation.final_fields {
                        dumper.raw_line(&format!("FinalField {field}"));
                    }
                    for field in &operation.fields {
                        match field {
                            HirSynthesizedFieldCopy::Scalar { field } => {
                                dumper.raw_line(&format!("Scalar {field}"));
                            }
                            HirSynthesizedFieldCopy::OptionalPrimitive { field, payload } => {
                                dumper.raw_line(&format!(
                                    "OptionalPrimitive {field} : {}?",
                                    payload.name()
                                ));
                            }
                            HirSynthesizedFieldCopy::Shared { field } => {
                                dumper.raw_line(&format!("Shared {field}"));
                            }
                            HirSynthesizedFieldCopy::OptionalShared { field, target } => {
                                dumper.raw_line(&format!(
                                    "OptionalShared {field} : {}",
                                    optional_shared_target_name(*target)
                                ));
                            }
                            HirSynthesizedFieldCopy::Optional { field, optional } => {
                                dumper.raw_line(&format!("Optional {field} : {optional}"));
                            }
                            HirSynthesizedFieldCopy::OptionalClass {
                                field,
                                class,
                                operation,
                            } => {
                                dumper.raw_line(&format!("OptionalClass {field} : class {class}?"));
                                dumper
                                    .indented(|dumper| dumper.selected_copy_operation(*operation));
                            }
                            HirSynthesizedFieldCopy::Class { field, operation } => {
                                let selected = match operation {
                                    HirSelectedCopyOperation::User(id) => format!("User {id}"),
                                    HirSelectedCopyOperation::Synthesized(class) => {
                                        format!("Synthesized {class}")
                                    }
                                };
                                dumper.raw_line(&format!("Class {field} using {selected}"));
                            }
                            HirSynthesizedFieldCopy::Array { field, array } => {
                                dumper.raw_line(&format!("Array {field} : {array}"));
                            }
                        }
                    }
                });
            }
        }
    }

    fn declaration(&mut self, declaration: &HirFunctionDeclaration) {
        self.write_indentation();
        let _ = write!(
            self.output,
            "Declaration {} module {} ",
            declaration.id, declaration.module
        );
        write_quoted(&mut self.output, &declaration.name);
        match &declaration.linkage {
            HirFunctionLinkage::Internal => self.output.push_str(" internal"),
            HirFunctionLinkage::External { link } => {
                let _ = write!(self.output, " external {link}");
            }
            HirFunctionLinkage::Intrinsic { intrinsic } => {
                let _ = write!(self.output, " intrinsic {intrinsic:?}");
            }
        }
        write_span(&mut self.output, declaration.span);
        self.output.push('\n');

        self.indented(|dumper| {
            dumper.heading("Parameters");
            dumper.indented(|dumper| {
                for parameter in &declaration.parameters {
                    dumper.parameter(parameter);
                }
            });

            dumper.write_indentation();
            let _ = writeln!(
                dumper.output,
                "ReturnType {}",
                declaration.return_type.name()
            );
        });
    }

    fn definition(&mut self, definition: &HirFunctionDefinition) {
        self.line(
            &format!("Definition {}", definition.function),
            definition.span,
        );

        self.indented(|dumper| {
            dumper.locals(&definition.locals);
            dumper.block(&definition.body);
        });
    }

    fn parameter(&mut self, parameter: &HirParameter) {
        self.write_indentation();
        let _ = write!(self.output, "Parameter {} ", parameter.id);
        write_quoted(&mut self.output, &parameter.name);
        let mode = match parameter.mode {
            HirParameterMode::Value => "value",
            HirParameterMode::ReadOnlyAlias => "ref",
            HirParameterMode::MutableAlias => "mut-ref",
        };
        let _ = write!(self.output, " {mode} : {}", self.type_name(parameter.ty));
        write_span(&mut self.output, parameter.span);
        self.output.push('\n');
    }

    fn locals(&mut self, locals: &[HirLocal]) {
        self.heading("Locals");
        self.indented(|dumper| {
            for local in locals {
                dumper.write_indentation();
                let _ = write!(dumper.output, "Local {} ", local.id);
                write_quoted(&mut dumper.output, &local.name);
                let _ = write!(dumper.output, " : {}", dumper.type_name(local.ty));
                write_span(&mut dumper.output, local.span);
                dumper.output.push('\n');
            }
        });
    }

    fn block(&mut self, block: &HirBlock) {
        self.line("Block", block.span);
        self.indented(|dumper| {
            for statement in &block.statements {
                dumper.statement(statement);
            }
        });
    }

    fn statement(&mut self, statement: &HirStatement) {
        match statement {
            HirStatement::BaseInitialization(statement) => {
                self.line(
                    &format!(
                        "BaseInitialization {} via {}",
                        statement.base, statement.initializer
                    ),
                    statement.span,
                );
                self.indented(|dumper| {
                    for argument in &statement.arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
            HirStatement::Local(local) => {
                self.line(&format!("LocalDeclaration {}", local.local), local.span);
                self.indented(|dumper| match &local.initializer {
                    HirLocalInitializer::Value(expression) => dumper.expression(expression),
                    HirLocalInitializer::Object(initialization) => {
                        dumper.line("ObjectInitialization", initialization.span);
                        dumper.indented(|dumper| {
                            dumper.object_place(&initialization.destination);
                            dumper.object_producer(&initialization.producer);
                            if let Some(operation) = initialization.elided_copy {
                                dumper.raw_line("ElidedCopy");
                                dumper.indented(|dumper| dumper.selected_copy_operation(operation));
                            }
                        });
                    }
                    HirLocalInitializer::Copy(copy) => dumper.copy_construction(copy),
                    HirLocalInitializer::Shared(value) => dumper.shared_transfer(value),
                    HirLocalInitializer::Optional(source) => dumper.optional_source(source),
                    HirLocalInitializer::ClassOptional(value) => dumper.class_optional_value(value),
                    HirLocalInitializer::OptionalShared(value) => {
                        dumper.optional_shared_value(value)
                    }
                    HirLocalInitializer::AggregateOptional(value) => {
                        dumper.aggregate_optional_value(value)
                    }
                    HirLocalInitializer::Array(value) => dumper.array_initialize(value),
                });
            }
            HirStatement::Return(statement) => {
                self.line("Return", statement.span);
                if let Some(value) = &statement.value {
                    self.indented(|dumper| match value {
                        HirReturnValue::Scalar(value) => dumper.expression(value),
                        HirReturnValue::Object(HirObjectReturn::Copy {
                            source,
                            operation,
                            class,
                            span,
                        }) => {
                            dumper.line(&format!("ObjectResult {class}"), *span);
                            dumper.indented(|dumper| {
                                dumper.object_source(source);
                                dumper.selected_copy_operation(*operation);
                            });
                        }
                        HirReturnValue::Object(HirObjectReturn::Construct {
                            construction,
                            omitted_copy,
                        }) => {
                            let heading = if omitted_copy.is_some() {
                                "ElidedObjectResult"
                            } else {
                                "ObjectResult"
                            };
                            dumper.line(heading, construction.span);
                            dumper.indented(|dumper| {
                                dumper.construction(construction);
                                if let Some(operation) = omitted_copy {
                                    dumper.raw_line("ElidedCopy");
                                    dumper.indented(|dumper| {
                                        dumper.selected_copy_operation(*operation)
                                    });
                                }
                            });
                        }
                        HirReturnValue::Shared(value) => dumper.shared_transfer(value),
                        HirReturnValue::Optional(source) => dumper.optional_source(source),
                        HirReturnValue::ClassOptional(value) => dumper.class_optional_value(value),
                        HirReturnValue::OptionalShared(value) => {
                            dumper.optional_shared_value(value)
                        }
                        HirReturnValue::AggregateOptional(value) => {
                            dumper.aggregate_optional_value(value)
                        }
                        HirReturnValue::Array(value) => dumper.array_initialize(value),
                    });
                }
            }
            HirStatement::Break(statement) => {
                self.line(&format!("Break {}", statement.target), statement.span);
            }
            HirStatement::Continue(statement) => {
                self.line(&format!("Continue {}", statement.target), statement.span);
            }
            HirStatement::Call(statement) => {
                self.line("CallStatement", statement.span);
                self.indented(|dumper| dumper.expression(&statement.call));
            }
            HirStatement::Panic(statement) => {
                self.line("Panic", statement.span);
                self.indented(|dumper| dumper.object_source(&statement.message.source));
            }
            HirStatement::Conditional(statement) => self.conditional(statement),
            HirStatement::While(statement) => {
                self.line(&format!("While {}", statement.loop_id), statement.span);
                self.indented(|dumper| {
                    dumper.line("Condition", statement.condition.span);
                    dumper.indented(|dumper| dumper.expression(&statement.condition));
                    dumper.block(&statement.body);
                });
            }
            HirStatement::ForIn(statement) => self.for_in(statement),
            HirStatement::Block(block) => self.block(block),
            HirStatement::ScalarAssignment(assignment) => {
                match assignment.destination.storage {
                    crate::hir::HirScalarStorage::Binding(binding) => self.line(
                        &format!("ScalarBindingAssignment {binding}"),
                        assignment.span,
                    ),
                    crate::hir::HirScalarStorage::Static(place) => self.line(
                        &format!("ScalarStaticAssignment {}", place.field),
                        assignment.span,
                    ),
                }
                self.indented(|dumper| dumper.expression(&assignment.source));
            }
            HirStatement::FieldAssignment(statement) => {
                self.line("FieldAssignment", statement.span);
                self.indented(|dumper| {
                    dumper.field_place(&statement.place);
                    dumper.expression(&statement.value);
                });
            }
            HirStatement::FieldConstruction(statement) => {
                self.line("FieldConstruction", statement.span);
                self.indented(|dumper| {
                    dumper.field_place(&statement.place);
                    dumper.construction(&statement.construction);
                });
            }
            HirStatement::FieldCopyConstruction(statement) => {
                self.line("FieldCopyConstruction", statement.span);
                self.indented(|dumper| {
                    dumper.field_place(&statement.place);
                    dumper.object_source(&statement.source);
                    dumper.selected_copy_operation(statement.operation);
                });
            }
            HirStatement::FieldCopyAssignment(statement) => {
                self.line("FieldCopyAssignment", statement.span);
                self.indented(|dumper| {
                    dumper.field_place(&statement.place);
                    dumper.object_source(&statement.source);
                    dumper.selected_copy_operation(statement.operation);
                });
            }
            HirStatement::CopyAssignment(statement) => {
                self.line("CopyAssignmentStatement", statement.span);
                self.indented(|dumper| {
                    dumper.object_place(&statement.destination);
                    dumper.object_source(&statement.source);
                    dumper.selected_copy_operation(statement.operation);
                });
            }
            HirStatement::SharedFieldWrite(statement) => {
                self.line(
                    match statement.kind {
                        HirSharedFieldWriteKind::Initialize => "SharedFieldInitialization",
                        HirSharedFieldWriteKind::Assign => "SharedFieldAssignment",
                    },
                    statement.span,
                );
                self.indented(|dumper| {
                    dumper.field_place(&statement.place);
                    dumper.shared_transfer(&statement.value);
                });
            }
            HirStatement::SharedStaticAssignment(statement) => {
                self.line("SharedStaticAssignment", statement.span);
                self.indented(|dumper| {
                    dumper.raw_line(&format!("StaticField {}", statement.destination.field));
                    dumper.shared_transfer(&statement.value);
                });
            }
            HirStatement::SharedAssignment(assignment) => {
                self.line(
                    &format!("SharedAssignment {}", assignment.destination),
                    assignment.span,
                );
                self.indented(|dumper| dumper.shared_transfer(&assignment.value));
            }
            HirStatement::OptionalAssignment(assignment) => {
                self.line(
                    &format!(
                        "OptionalAssignment {:?} : {}?",
                        assignment.kind,
                        assignment.payload.name()
                    ),
                    assignment.span,
                );
                self.indented(|dumper| {
                    dumper.optional_place(&assignment.destination);
                    dumper.optional_source(&assignment.source);
                });
            }
            HirStatement::ClassOptionalAssignment(assignment) => {
                self.line(
                    &format!(
                        "ClassOptionalAssignment class {}?",
                        assignment.destination.class
                    ),
                    assignment.span,
                );
                self.indented(|dumper| {
                    dumper.class_optional_place(&assignment.destination);
                    dumper.class_optional_source(&assignment.source);
                });
            }
            HirStatement::OptionalSharedAssignment(assignment) => {
                self.line(
                    &format!(
                        "OptionalSharedAssignment {}",
                        optional_shared_target_name(assignment.destination.target)
                    ),
                    assignment.span,
                );
                self.indented(|dumper| {
                    dumper.optional_shared_place(&assignment.destination);
                    dumper.optional_shared_source(&assignment.source);
                });
            }
            HirStatement::AggregateOptionalAssignment(assignment) => {
                self.line(
                    &format!("AggregateOptionalAssignment {}", assignment.value.optional),
                    assignment.span,
                );
                self.indented(|dumper| dumper.aggregate_optional_value(&assignment.value));
            }
            HirStatement::ArrayFieldInitialize(statement) => {
                self.line("ArrayFieldInitialization", statement.span);
                self.indented(|dumper| {
                    dumper.field_place(&statement.place);
                    dumper.array_initialize(&statement.value);
                });
            }
            HirStatement::ArrayAssignment(statement) => {
                self.line(
                    &format!("ArrayReplacement {:?}", statement.evaluation),
                    statement.span,
                );
                self.indented(|dumper| {
                    dumper.array_place(&statement.destination);
                    dumper.array_initialize(&statement.value);
                });
            }
            HirStatement::ArrayElementAssignment(statement) => {
                self.line(
                    &format!(
                        "ArrayElementAssignment {} {:?}",
                        array_assignment_name(statement.operation),
                        statement.evaluation
                    ),
                    statement.span,
                );
                self.indented(|dumper| {
                    dumper.array_element(&statement.destination);
                    dumper.array_element_value(&statement.value);
                });
            }
            HirStatement::ArraySliceAssignment(statement) => {
                self.line(
                    &format!(
                        "ArraySliceAssignment {} failure={:?} {:?}",
                        array_assignment_name(statement.operation),
                        statement.failure,
                        statement.evaluation
                    ),
                    statement.span,
                );
                self.indented(|dumper| {
                    dumper.array_slice(&statement.destination);
                    dumper.array_source(&statement.source);
                });
            }
        }
    }

    fn for_in(&mut self, statement: &HirForIn) {
        self.line(
            &format!("ForIn {} binding {}", statement.loop_id, statement.binding),
            statement.spans.span,
        );
        self.indented(|dumper| {
            match &statement.plan {
                HirForInPlan::Protocol(plan) => dumper.protocol_iteration_plan(plan),
                HirForInPlan::PrimitiveRange(plan) => dumper.primitive_range_plan(plan),
            }
            dumper.raw_line(&format!("Effects {:?}", statement.effects));
            dumper.block(&statement.body);
        });
    }

    fn protocol_iteration_plan(&mut self, plan: &HirProtocolIterationPlan) {
        self.raw_line(&format!(
            "Protocol interface={} item={} state={} result={}",
            plan.protocol.interface,
            self.type_name(plan.protocol.item),
            self.type_name(plan.protocol.state),
            plan.protocol.result,
        ));
        self.raw_line(&format!(
            "Requirements iter_state={} iter_next={}",
            plan.protocol.iter_state, plan.protocol.iter_next,
        ));
        self.raw_line(&format!(
            "Receiver iterable={} lifetime={:?}",
            self.type_name(plan.receiver.iterable),
            plan.receiver.lifetime,
        ));
        self.indented(|dumper| match &plan.receiver.carrier {
            HirIterationReceiverCarrier::View(view) => dumper.object_view("LoopReceiver", view),
            HirIterationReceiverCarrier::Checked(view) => {
                let kind = match view.kind {
                    HirCheckedObjectViewKind::Static => "static",
                    HirCheckedObjectViewKind::RuntimeTerminate => "runtime-terminate",
                };
                dumper.object_view(&format!("LoopReceiver Checked {kind}"), &view.view);
            }
        });
        self.raw_line(&format!(
            "State copy={:?} destruction={:?}",
            plan.state.value.copy, plan.state.value.destruction,
        ));
        self.raw_line(&format!(
            "IterState target={}:{} receiver={} result={}",
            plan.state.initialize.target.interface,
            plan.state.initialize.target.requirement,
            access_name(plan.state.initialize.receiver_access),
            self.type_name(plan.state.initialize.result),
        ));
        self.raw_line(&format!(
            "IterNext target={}:{} receiver={} state-alias={} {} result={}",
            plan.state.advance.target.interface,
            plan.state.advance.target.requirement,
            access_name(plan.state.advance.receiver_access),
            access_name(plan.state.advance.state_alias.access),
            self.type_name(plan.state.advance.state_alias.ty),
            self.type_name(plan.state.advance.result),
        ));
        self.raw_line(&format!(
            "Result optional={} payload={} presence={:?} unwrap={:?} destruction={:?}",
            plan.result.optional,
            self.type_name(plan.result.payload),
            plan.result.presence,
            plan.result.unwrap,
            plan.result.destruction,
        ));
        self.iteration_item(&plan.item);
    }

    fn primitive_range_plan(&mut self, plan: &HirPrimitiveRangeIterationPlan) {
        self.raw_line(&format!(
            "PrimitiveRange endpoint={} compare={:?} increment={:?}",
            plan.integer.name(),
            plan.comparison,
            plan.increment,
        ));
        self.raw_line(&format!(
            "CanonicalRangeSyntax template={} class={} initializer={} iterable={}",
            plan.origin.range_template,
            plan.origin.range_class,
            plan.origin.initializer,
            plan.origin.iterable,
        ));
        self.indented(|dumper| {
            dumper.range_protocol_evidence(
                "Ordering",
                plan.origin.ordering,
                plan.origin.operator_span,
            );
            dumper.range_protocol_evidence(
                "Successor",
                plan.origin.successor,
                plan.origin.operator_span,
            );
            dumper.line("Lower", plan.lower.span);
            dumper.indented(|dumper| dumper.expression(&plan.lower));
            dumper.line("Upper", plan.upper.span);
            dumper.indented(|dumper| dumper.expression(&plan.upper));
        });
        self.iteration_item(&plan.item);
    }

    fn iteration_item(&mut self, item: &HirIterationItemPlan) {
        self.raw_line(&format!(
            "Item binding={} access={} copy={:?} destruction={:?}",
            item.binding,
            access_name(item.access),
            item.value.copy,
            item.value.destruction,
        ));
    }

    fn copy_construction(&mut self, copy: &crate::hir::HirCopyConstruction) {
        self.line("CopyConstruction", copy.span);
        self.indented(|dumper| {
            dumper.object_place(&copy.destination);
            dumper.object_source(&copy.source);
            dumper.selected_copy_operation(copy.operation);
        });
    }

    fn selected_copy_operation<I: Display>(&mut self, operation: HirSelectedCopyOperation<I>) {
        match operation {
            HirSelectedCopyOperation::User(id) => self.raw_line(&format!("Operation User {id}")),
            HirSelectedCopyOperation::Synthesized(class) => {
                self.raw_line(&format!("Operation Synthesized {class}"));
            }
        }
    }

    fn conditional(&mut self, statement: &HirConditional) {
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

    fn expression(&mut self, expression: &HirExpression) {
        match &expression.kind {
            HirExpressionKind::Binding(binding) => {
                self.typed_line(&format!("Binding {binding}"), expression);
            }
            HirExpressionKind::FunctionReference(reference) => {
                self.typed_line(
                    &format!(
                        "FunctionReference {} signature {}",
                        reference.target, reference.function_type
                    ),
                    expression,
                );
            }
            HirExpressionKind::I64(value) => {
                self.typed_line(&format!("Integer {value}"), expression);
            }
            HirExpressionKind::U64(value) => {
                self.typed_line(&format!("U64 {value}"), expression);
            }
            HirExpressionKind::U8(value) => {
                self.typed_line(&format!("U8 {value}"), expression);
            }
            HirExpressionKind::F64Bits(bits) => {
                self.typed_line(&format!("F64 0x{bits:016x}"), expression);
            }
            HirExpressionKind::Boolean(value) => {
                self.typed_line(&format!("Boolean {value}"), expression);
            }
            HirExpressionKind::Unary { operation, operand } => {
                let operation = match operation {
                    HirUnaryOperation::NegateI64 => "NegateI64".to_owned(),
                    HirUnaryOperation::NegateF64 => "NegateF64".to_owned(),
                    HirUnaryOperation::LogicalNotBool => "LogicalNotBool".to_owned(),
                    HirUnaryOperation::BitwiseComplement(integer) => {
                        format!("BitwiseComplement.{}", integer.name())
                    }
                };
                self.typed_line(&format!("Unary {operation}"), expression);
                self.indented(|dumper| dumper.expression(operand));
            }
            HirExpressionKind::Binary {
                operation,
                left,
                right,
            } => {
                let operation = match operation {
                    HirBinaryOperation::AddI64 => "AddI64".to_owned(),
                    HirBinaryOperation::SubtractI64 => "SubtractI64".to_owned(),
                    HirBinaryOperation::MultiplyI64 => "MultiplyI64".to_owned(),
                    HirBinaryOperation::AddU64 => "AddU64".to_owned(),
                    HirBinaryOperation::SubtractU64 => "SubtractU64".to_owned(),
                    HirBinaryOperation::MultiplyU64 => "MultiplyU64".to_owned(),
                    HirBinaryOperation::AddU8 => "AddU8".to_owned(),
                    HirBinaryOperation::SubtractU8 => "SubtractU8".to_owned(),
                    HirBinaryOperation::MultiplyU8 => "MultiplyU8".to_owned(),
                    HirBinaryOperation::AddF64 => "AddF64".to_owned(),
                    HirBinaryOperation::SubtractF64 => "SubtractF64".to_owned(),
                    HirBinaryOperation::MultiplyF64 => "MultiplyF64".to_owned(),
                    HirBinaryOperation::DivideF64 => "DivideF64".to_owned(),
                    HirBinaryOperation::IntegerBitwise { operation, operand } => {
                        format!("Bitwise{}.{}", bitwise_title(*operation), operand.name())
                    }
                };
                self.typed_line(&format!("Binary {operation}"), expression);
                self.indented(|dumper| {
                    dumper.expression(left);
                    dumper.expression(right);
                });
            }
            HirExpressionKind::CheckedShift(shift) => {
                self.typed_line(
                    &format!(
                        "CheckedShift {}.{} count=u64 width={} failure=shift-count-out-of-range",
                        shift.operation.mnemonic(),
                        shift.operation.left.name(),
                        shift.operation.width()
                    ),
                    expression,
                );
                self.indented(|dumper| {
                    dumper.heading("Left");
                    dumper.indented(|dumper| dumper.expression(&shift.left));
                    dumper.heading("Count");
                    dumper.indented(|dumper| dumper.expression(&shift.count));
                });
            }
            HirExpressionKind::CheckedIntegerDivision(division) => {
                division.validate(expression.ty);
                let signed_semantics = division
                    .operation
                    .signed_semantics()
                    .map(|semantics| {
                        let minimum_pair_result = match semantics.minimum_pair_result {
                            crate::hir::HirSignedMinimumPairResult::Minimum => "minimum",
                            crate::hir::HirSignedMinimumPairResult::Zero => "zero",
                        };
                        format!(
                            " signed-quotient=floor signed-remainder-sign=divisor minimum-pair={minimum_pair_result}"
                        )
                    })
                    .unwrap_or_default();
                self.typed_line(
                    &format!(
                        "CheckedIntegerDivision {}.{}{} failure={}",
                        division.operation.mnemonic(),
                        division.operation.operand.name(),
                        signed_semantics,
                        division.operation.failure().mnemonic(),
                    ),
                    expression,
                );
                self.indented(|dumper| {
                    dumper.heading("Dividend");
                    dumper.indented(|dumper| dumper.expression(&division.dividend));
                    dumper.heading("Divisor");
                    dumper.indented(|dumper| dumper.expression(&division.divisor));
                });
            }
            HirExpressionKind::Logical(logical) => {
                let operation = match logical.operation {
                    crate::hir::HirLogicalOperation::And => "And",
                    crate::hir::HirLogicalOperation::Or => "Or",
                };
                self.typed_line(&format!("Logical {operation}"), expression);
                self.indented(|dumper| {
                    dumper.heading("Left");
                    dumper.indented(|dumper| dumper.expression(&logical.left));
                    dumper.heading("Right");
                    dumper.indented(|dumper| dumper.expression(&logical.right));
                });
            }
            HirExpressionKind::PrimitiveComparison {
                operation,
                left,
                right,
            } => {
                let family = match operation.operand {
                    HirComparisonOperand::Integer(_) => "IntegerComparison",
                    HirComparisonOperand::F64 => "FloatingComparison",
                    HirComparisonOperand::Bool => "BooleanComparison",
                };
                self.typed_line(
                    &format!(
                        "{family} {}.{}",
                        operation.predicate.mnemonic(),
                        operation.operand.name()
                    ),
                    expression,
                );
                self.indented(|dumper| {
                    dumper.expression(left);
                    dumper.expression(right);
                });
            }
            HirExpressionKind::PrimitiveCast { operation, operand } => {
                let failure = if operation.may_terminate() {
                    " failure=primitive-cast-out-of-range"
                } else {
                    ""
                };
                self.typed_line(
                    &format!(
                        "PrimitiveCast {} {}.{}{}",
                        operation.kind().mnemonic(),
                        operation.source.name(),
                        operation.target.name(),
                        failure,
                    ),
                    expression,
                );
                self.indented(|dumper| dumper.expression(operand));
            }
            HirExpressionKind::Io(operation) => self.io_operation(expression, operation),
            HirExpressionKind::DirectCall {
                function,
                arguments,
            } => {
                self.typed_line(&format!("DirectCall {function}"), expression);
                self.indented(|dumper| {
                    for argument in arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
            HirExpressionKind::IndirectCall(call) => {
                self.typed_line(
                    &format!("IndirectCall type {}", call.function_type),
                    expression,
                );
                self.indented(|dumper| {
                    dumper.heading("Callee");
                    dumper.indented(|dumper| dumper.expression(&call.callee));
                    dumper.heading("Arguments");
                    dumper.indented(|dumper| {
                        for argument in &call.arguments {
                            dumper.call_argument(argument);
                        }
                    });
                });
            }
            HirExpressionKind::StaticCall { method, arguments } => {
                self.typed_line(&format!("StaticCall {method}"), expression);
                self.indented(|dumper| {
                    for argument in arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
            HirExpressionKind::Grouped(inner) => {
                self.typed_line("Grouped", expression);
                self.indented(|dumper| dumper.expression(inner));
            }
            HirExpressionKind::FieldRead(place) => {
                self.typed_line(&format!("FieldRead {}", place.field), expression);
                self.indented(|dumper| match place.receiver.inspection_place() {
                    Some(receiver) => dumper.object_place(receiver),
                    None => dumper.field_place(place),
                });
            }
            HirExpressionKind::StaticRead(place) => {
                self.typed_line(&format!("StaticRead {}", place.field), expression);
            }
            HirExpressionKind::MethodCall {
                receiver,
                target,
                arguments,
            } => {
                self.typed_line(&format!("MethodCall {}", method_target(target)), expression);
                self.indented(|dumper| {
                    dumper.method_receiver(receiver);
                    for argument in arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
            HirExpressionKind::InterfaceCall {
                receiver,
                target,
                arguments,
            } => {
                self.typed_line(
                    &format!("InterfaceCall {} {}", target.interface, target.requirement),
                    expression,
                );
                self.indented(|dumper| {
                    dumper.interface_receiver(receiver);
                    for argument in arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
            HirExpressionKind::TypeTest(test) => {
                let kind = match test.kind {
                    HirTypeTestKind::StaticSuccess => "static-success",
                    HirTypeTestKind::StaticFailure => "static-failure",
                    HirTypeTestKind::Runtime => "runtime",
                };
                self.typed_line(
                    &format!("TypeTest -> {} {kind}", view_target_name(test.target)),
                    expression,
                );
                self.indented(|dumper| dumper.object_view("ObjectView", &test.source));
            }
            HirExpressionKind::PresenceTest { source, kind } => {
                let kind = match kind {
                    crate::hir::HirPresenceTestKind::Some => "Some",
                    crate::hir::HirPresenceTestKind::None => "None",
                };
                self.typed_line(&format!("PresenceTest {kind}"), expression);
                self.indented(|dumper| dumper.optional_operand(source));
            }
            HirExpressionKind::Unwrap(source) => {
                self.typed_line("OptionalUnwrap", expression);
                self.indented(|dumper| dumper.optional_operand(source));
            }
            HirExpressionKind::NestedOptionalUnwrap(unwrap) => {
                self.typed_line(
                    &format!(
                        "NestedOptionalUnwrap optional={} payload={}",
                        unwrap.optional, unwrap.payload
                    ),
                    expression,
                );
                self.indented(|dumper| dumper.optional_operand(&unwrap.source));
            }
            HirExpressionKind::OptionalArrayUnwrap(unwrap) => {
                self.typed_line(
                    &format!(
                        "OptionalArrayUnwrap optional={} array={}",
                        unwrap.optional, unwrap.array
                    ),
                    expression,
                );
                self.indented(|dumper| dumper.optional_operand(&unwrap.source));
            }
            HirExpressionKind::ArrayConstruction(construction) => {
                self.typed_line("ArrayConstruction", expression);
                self.indented(|dumper| dumper.array_construction(construction));
            }
            HirExpressionKind::ArrayLength(length) => {
                self.typed_line("ArrayLength", expression);
                self.indented(|dumper| dumper.array_receiver(&length.receiver));
            }
            HirExpressionKind::ArrayElement(place) => {
                self.typed_line("ArrayElement", expression);
                self.indented(|dumper| dumper.array_element(place));
            }
            HirExpressionKind::ArraySlice(slice) => {
                self.typed_line("CopiedArraySlice", expression);
                self.indented(|dumper| dumper.array_slice(slice));
            }
            HirExpressionKind::OptionalBoxPresence(presence) => {
                self.typed_line(
                    &format!(
                        "OptionalBoxPresence {:?} target={}",
                        presence.kind, presence.box_target
                    ),
                    expression,
                );
                self.indented(|dumper| dumper.shared_source(&presence.source));
            }
        }
    }

    fn io_operation(&mut self, expression: &HirExpression, operation: &HirIoOperation) {
        let name = match operation {
            HirIoOperation::StandardHandle { .. } => "StandardHandle",
            HirIoOperation::Open { .. } => "Open",
            HirIoOperation::Read { .. } => "Read",
            HirIoOperation::Write { .. } => "Write",
            HirIoOperation::Close { .. } => "Close",
        };
        self.typed_line(&format!("Io {name}"), expression);
        self.indented(|dumper| match operation {
            HirIoOperation::StandardHandle { stream } => dumper.expression(stream),
            HirIoOperation::Open { path, mode } => {
                dumper.call_argument(&HirCallArgument::ArrayAlias(path.clone()));
                dumper.expression(mode);
            }
            HirIoOperation::Read {
                handle,
                destination,
                offset,
            } => {
                dumper.expression(handle);
                dumper.call_argument(&HirCallArgument::ArrayAlias(destination.clone()));
                dumper.expression(offset);
            }
            HirIoOperation::Write {
                handle,
                source,
                offset,
            } => {
                dumper.expression(handle);
                dumper.call_argument(&HirCallArgument::ArrayAlias(source.clone()));
                dumper.expression(offset);
            }
            HirIoOperation::Close { handle } => dumper.expression(handle),
        });
    }

    fn array_receiver(&mut self, receiver: &crate::hir::HirArrayReceiver) {
        self.line(
            &format!(
                "ArrayReceiver {} {:?} access={:?} anchor={:?}",
                receiver.array, receiver.ownership, receiver.access, receiver.anchor
            ),
            receiver.span,
        );
        self.indented(|dumper| match &receiver.source {
            crate::hir::HirArrayReceiverSource::Inline(expression) => dumper.expression(expression),
            crate::hir::HirArrayReceiverSource::Shared(source) => dumper.shared_source(source),
        });
    }

    fn array_element(&mut self, place: &crate::hir::HirArrayElementPlace) {
        self.line(
            &format!(
                "ArrayElementPlace : {} {:?}",
                place.element.name(),
                place.evaluation
            ),
            place.span,
        );
        self.indented(|dumper| {
            dumper.array_receiver(&place.receiver);
            dumper.line(
                &format!(
                    "Index normalization={:?} failure={:?}",
                    place.index.normalization, place.index.failure
                ),
                place.index.span,
            );
            dumper.indented(|dumper| dumper.expression(&place.index.value));
        });
    }

    fn array_slice(&mut self, slice: &crate::hir::HirArraySlice) {
        self.line(
            &format!(
                "ArraySlice {} copy={} normalization={:?} failure={:?} {:?}",
                slice.array,
                slice
                    .element_copy
                    .map(array_copy_name)
                    .unwrap_or_else(|| "destination-only".to_owned()),
                slice.bounds.normalization,
                slice.bounds.failure,
                slice.evaluation
            ),
            slice.span,
        );
        self.indented(|dumper| {
            dumper.array_receiver(&slice.receiver);
            if let Some(start) = &slice.bounds.start {
                dumper.heading("Start");
                dumper.indented(|dumper| dumper.expression(start));
            } else {
                dumper.raw_line("Start omitted");
            }
            if let Some(end) = &slice.bounds.end {
                dumper.heading("End");
                dumper.indented(|dumper| dumper.expression(end));
            } else {
                dumper.raw_line("End omitted");
            }
        });
    }

    fn array_place(&mut self, place: &crate::hir::HirArrayPlace) {
        match place {
            crate::hir::HirArrayPlace::Binding {
                binding,
                array,
                access,
                span,
            } => self.line(
                &format!("ArrayPlace binding {binding} {array} access={access:?}"),
                *span,
            ),
            crate::hir::HirArrayPlace::Field {
                place,
                array,
                access,
                span,
            } => {
                self.line(
                    &format!("ArrayPlace field {array} access={access:?}"),
                    *span,
                );
                self.indented(|dumper| dumper.field_place(place));
            }
            crate::hir::HirArrayPlace::Static { place, array, span } => {
                self.line(&format!("ArrayPlace static {} {array}", place.field), *span)
            }
            crate::hir::HirArrayPlace::Element(place) => self.array_element(place),
        }
    }

    fn array_element_value(&mut self, value: &crate::hir::HirArrayElementValue) {
        match value {
            crate::hir::HirArrayElementValue::Value(value) => self.expression(value),
            crate::hir::HirArrayElementValue::Array(value) => self.array_initialize(value),
            crate::hir::HirArrayElementValue::Shared(value) => self.shared_transfer(value),
            crate::hir::HirArrayElementValue::OptionalShared(value) => {
                self.optional_shared_value(value)
            }
            crate::hir::HirArrayElementValue::Optional { source, .. } => {
                self.optional_source(source)
            }
            crate::hir::HirArrayElementValue::ClassOptional(value) => {
                self.class_optional_value(value)
            }
            crate::hir::HirArrayElementValue::AggregateOptional(value) => {
                self.aggregate_optional_value(value)
            }
            crate::hir::HirArrayElementValue::Object { source, operation } => {
                self.object_source(source);
                self.selected_copy_operation(*operation);
            }
        }
    }

    fn array_construction(&mut self, construction: &HirArrayConstruction) {
        let ownership = match construction.ownership {
            HirArrayOwnership::Inline => "inline",
            HirArrayOwnership::Shared => "shared",
        };
        self.line(
            &format!("ArrayAllocation {ownership} {}", construction.array),
            construction.span,
        );
        self.indented(|dumper| match &construction.mode {
            HirArrayConstructionMode::Empty => dumper.raw_line("Empty"),
            HirArrayConstructionMode::DefaultLength { length, element } => {
                dumper.raw_line(&format!("DefaultElements {}", array_default_name(*element)));
                dumper.indented(|dumper| dumper.expression(length));
            }
            HirArrayConstructionMode::Copy { source, element } => {
                dumper.raw_line(&format!("CopyElements {}", array_copy_name(*element)));
                dumper.indented(|dumper| dumper.array_source(source));
            }
            HirArrayConstructionMode::Elements(list) => {
                dumper.raw_line(&format!(
                    "ElementList count={} commas={}",
                    list.elements.len(),
                    list.comma_spans.len()
                ));
                dumper.indented(|dumper| {
                    for (index, element) in list.elements.iter().enumerate() {
                        dumper.line(
                            &format!("Element {index} : {}", element.element.name()),
                            element.span,
                        );
                        dumper
                            .indented(|dumper| dumper.stored_value_initialization(&element.value));
                    }
                });
            }
        });
    }

    fn stored_value_initialization(&mut self, value: &crate::hir::HirStoredValueInitialization) {
        match value {
            crate::hir::HirStoredValueInitialization::Scalar(value) => {
                self.raw_line("ScalarInitialization");
                self.indented(|dumper| dumper.expression(value));
            }
            crate::hir::HirStoredValueInitialization::Class(value) => {
                self.object_destination_initialization(value)
            }
            crate::hir::HirStoredValueInitialization::OptionalPrimitive { source, payload } => {
                self.raw_line(&format!(
                    "OptionalPrimitiveInitialization {}?",
                    payload.name()
                ));
                self.indented(|dumper| dumper.optional_source(source));
            }
            crate::hir::HirStoredValueInitialization::OptionalClass(value) => {
                self.class_optional_destination_initialization(value)
            }
            crate::hir::HirStoredValueInitialization::Array(value) => self.array_initialize(value),
            crate::hir::HirStoredValueInitialization::Shared(value) => self.shared_transfer(value),
            crate::hir::HirStoredValueInitialization::OptionalShared(value) => {
                self.optional_shared_value(value)
            }
            crate::hir::HirStoredValueInitialization::Optional(value) => {
                self.aggregate_optional_value(value)
            }
            crate::hir::HirStoredValueInitialization::OptionalBoxPointeeCopy {
                source,
                optional,
                operation,
                span,
            } => {
                self.line(
                    &format!("OptionalBoxPointeeCopy {optional} via {operation:?}"),
                    *span,
                );
                self.indented(|dumper| dumper.shared_source(source));
            }
        }
    }

    fn object_destination_initialization(
        &mut self,
        value: &crate::hir::HirObjectDestinationInitialization,
    ) {
        match value {
            crate::hir::HirObjectDestinationInitialization::Direct { producer, .. } => {
                self.raw_line("ClassInitialization direct");
                self.indented(|dumper| dumper.object_producer(producer));
            }
            crate::hir::HirObjectDestinationInitialization::Copy {
                source, operation, ..
            } => {
                self.raw_line("ClassInitialization copy");
                self.indented(|dumper| {
                    dumper.object_source(source);
                    dumper.selected_copy_operation(*operation);
                });
            }
        }
    }

    fn class_optional_destination_initialization(
        &mut self,
        value: &crate::hir::HirClassOptionalDestinationInitialization,
    ) {
        match value {
            crate::hir::HirClassOptionalDestinationInitialization::Absent { class, span } => self
                .line(
                    &format!("ClassOptionalInitialization class {class}? absent"),
                    *span,
                ),
            crate::hir::HirClassOptionalDestinationInitialization::Direct {
                class,
                producer,
                span,
            } => {
                self.line(
                    &format!("ClassOptionalInitialization class {class}? direct"),
                    *span,
                );
                self.indented(|dumper| dumper.object_producer(producer));
            }
            crate::hir::HirClassOptionalDestinationInitialization::Copy {
                class,
                source,
                operation,
                span,
            } => {
                self.line(
                    &format!("ClassOptionalInitialization class {class}? copy"),
                    *span,
                );
                self.indented(|dumper| {
                    dumper.class_optional_source(source);
                    dumper.selected_copy_operation(*operation);
                });
            }
        }
    }

    fn array_source(&mut self, source: &HirArraySource) {
        let provenance = match source.provenance {
            HirArrayProvenance::Named => "named",
            HirArrayProvenance::Produced => "produced",
        };
        self.line(
            &format!("ArraySource {provenance} {}", source.array),
            source.span,
        );
        self.indented(|dumper| dumper.array_receiver(&source.receiver));
    }

    fn array_initialize(&mut self, value: &HirArrayInitialize) {
        let operation = match value.operation {
            HirArrayTransfer::DeepCopy(element) => {
                format!("deep-copy {}", array_copy_name(element))
            }
            HirArrayTransfer::Adopt => "adopt".to_owned(),
        };
        self.line(&format!("ArrayInitialization {operation}"), value.span);
        self.indented(|dumper| dumper.array_source(&value.source));
    }

    fn optional_source(&mut self, source: &crate::hir::HirOptionalSource) {
        match source {
            crate::hir::HirOptionalSource::Absent { span } => self.line("OptionalAbsent", *span),
            crate::hir::HirOptionalSource::Present(value) => {
                self.line("OptionalPresent", value.span);
                self.indented(|dumper| dumper.expression(value));
            }
            crate::hir::HirOptionalSource::Copy(place) => {
                self.line("OptionalCopy", place.span);
                self.indented(|dumper| dumper.optional_place(place));
            }
            crate::hir::HirOptionalSource::Produced(expression) => {
                self.line("OptionalProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
        }
    }

    fn class_optional_value(&mut self, value: &crate::hir::HirClassOptionalInitialize) {
        self.line(
            &format!("ClassOptionalInitialization class {}?", value.class),
            value.span,
        );
        self.indented(|dumper| dumper.class_optional_source(&value.source));
    }

    fn class_optional_source(&mut self, source: &crate::hir::HirClassOptionalSource) {
        match source {
            crate::hir::HirClassOptionalSource::Absent { span } => {
                self.line("ClassOptionalAbsent", *span)
            }
            crate::hir::HirClassOptionalSource::Present(source) => {
                self.line("ClassOptionalPresent", source.span());
                self.indented(|dumper| dumper.object_source(source));
            }
            crate::hir::HirClassOptionalSource::Copy(place) => {
                self.line(
                    &format!("ClassOptionalCopy class {}?", place.class),
                    place.span,
                );
            }
            crate::hir::HirClassOptionalSource::Produced(expression) => {
                self.line("ClassOptionalProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
        }
    }

    fn optional_operand(&mut self, operand: &crate::hir::HirOptionalOperand) {
        match operand {
            crate::hir::HirOptionalOperand::Place(place) => self.optional_place(place),
            crate::hir::HirOptionalOperand::Produced(expression) => {
                self.line("OptionalProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
            crate::hir::HirOptionalOperand::ClassPlace(place) => {
                self.line(
                    &format!("ClassOptionalPlace class {}?", place.class),
                    place.span,
                );
                if let crate::hir::HirOptionalStorage::SharedPointee(pointee) = &place.storage {
                    self.indented(|dumper| {
                        dumper.optional_box_pointee("ClassOptionalBoxPointee", pointee)
                    });
                }
            }
            crate::hir::HirOptionalOperand::ClassProduced(expression) => {
                self.line("ClassOptionalProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
            crate::hir::HirOptionalOperand::SharedPlace(place) => self.optional_shared_place(place),
            crate::hir::HirOptionalOperand::SharedProduced(expression) => {
                self.line("OptionalSharedProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
            crate::hir::HirOptionalOperand::AggregatePlace(place) => {
                self.line(
                    &format!("AggregateOptionalPlace {}", place.optional),
                    place.span,
                );
                if let crate::hir::HirOptionalStorage::SharedPointee(pointee) = &place.storage {
                    self.indented(|dumper| {
                        dumper.optional_box_pointee("AggregateOptionalBoxPointee", pointee)
                    });
                }
            }
            crate::hir::HirOptionalOperand::AggregateProduced(expression) => {
                self.line("AggregateOptionalProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
        }
    }

    fn aggregate_optional_value(&mut self, value: &crate::hir::HirOptionalValue) {
        self.line(
            &format!("AggregateOptionalInitialization {}", value.optional),
            value.span,
        );
        self.indented(|dumper| match &value.source {
            crate::hir::HirOptionalValueSource::Absent => dumper.raw_line("Absent"),
            crate::hir::HirOptionalValueSource::Present(payload) => {
                dumper.raw_line("Present");
                dumper.indented(|dumper| dumper.stored_value_initialization(payload));
            }
            crate::hir::HirOptionalValueSource::Copy(place) => {
                dumper.line(&format!("Copy {}", place.optional), place.span);
            }
            crate::hir::HirOptionalValueSource::Produced(expression) => {
                dumper.line("Produced", expression.span);
                dumper.indented(|dumper| dumper.expression(expression));
            }
        });
    }

    fn optional_place(&mut self, place: &crate::hir::HirOptionalPlace) {
        match &place.storage {
            crate::hir::HirOptionalStorage::Binding(binding) => {
                self.line(&format!("OptionalPlace {binding}"), place.span);
            }
            crate::hir::HirOptionalStorage::Static(root) => {
                self.line(&format!("OptionalStaticPlace {}", root.field), place.span);
            }
            crate::hir::HirOptionalStorage::Field(field) => {
                self.line("OptionalFieldPlace", place.span);
                self.indented(|dumper| dumper.field_place(field));
            }
            crate::hir::HirOptionalStorage::ArrayElement(element) => {
                self.line("OptionalArrayElementPlace", place.span);
                self.indented(|dumper| dumper.array_element(element));
            }
            crate::hir::HirOptionalStorage::SharedPointee(pointee) => {
                self.optional_box_pointee("OptionalBoxPointee", pointee);
            }
        }
    }

    fn class_optional_place(&mut self, place: &crate::hir::HirClassOptionalPlace) {
        match &place.storage {
            crate::hir::HirOptionalStorage::Binding(binding) => {
                self.line(&format!("ClassOptionalPlace {binding}"), place.span);
            }
            crate::hir::HirOptionalStorage::Static(root) => {
                self.line(
                    &format!("ClassOptionalStaticPlace {}", root.field),
                    place.span,
                );
            }
            crate::hir::HirOptionalStorage::Field(field) => {
                self.line("ClassOptionalFieldPlace", place.span);
                self.indented(|dumper| dumper.field_place(field));
            }
            crate::hir::HirOptionalStorage::ArrayElement(element) => {
                self.line("ClassOptionalArrayElementPlace", place.span);
                self.indented(|dumper| dumper.array_element(element));
            }
            crate::hir::HirOptionalStorage::SharedPointee(pointee) => {
                self.optional_box_pointee("ClassOptionalBoxPointee", pointee);
            }
        }
    }

    fn construction(&mut self, construction: &HirConstruction) {
        if let crate::hir::HirConstructionOrigin::CanonicalRangeSyntax(origin) =
            &construction.origin
        {
            self.line(
                &format!(
                    "CanonicalRangeSyntax template={} class={} initializer={} endpoint={} iterable={}",
                    origin.range_template,
                    origin.range_class,
                    origin.initializer,
                    origin.endpoint_type.name(),
                    origin.iterable,
                ),
                origin.operator_span,
            );
            self.indented(|dumper| {
                dumper.range_protocol_evidence("Ordering", origin.ordering, origin.operator_span);
                dumper.range_protocol_evidence("Successor", origin.successor, origin.operator_span);
            });
        }
        match &construction.mode {
            HirConstructionMode::Initialize {
                initializer,
                arguments,
            } => {
                self.line(
                    &format!("Construct {} via {initializer}", construction.class),
                    construction.span,
                );
                self.indented(|dumper| {
                    for argument in arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
            HirConstructionMode::Copy { source, operation } => {
                self.line(
                    &format!("ExplicitCopyConstruct {}", construction.class),
                    construction.span,
                );
                self.indented(|dumper| {
                    dumper.object_source(source);
                    dumper.selected_copy_operation(*operation);
                });
            }
        }
    }

    fn range_protocol_evidence(
        &mut self,
        name: &str,
        evidence: crate::hir::HirRangeProtocolEvidence,
        span: Span,
    ) {
        let realization = match evidence.realization {
            crate::hir::HirRangeProtocolRealization::ClassWitness => "class-witness".to_owned(),
            crate::hir::HirRangeProtocolRealization::PrimitiveIntrinsic(ty) => {
                format!("primitive-{}", ty.name())
            }
        };
        self.line(
            &format!(
                "{name} interface={} requirement={} realization={realization}",
                evidence.interface, evidence.requirement,
            ),
            span,
        );
    }

    fn object_call(&mut self, call: &HirObjectCall) {
        let target = match call.target {
            HirObjectCallTarget::Direct(function) => format!("function {function}"),
            HirObjectCallTarget::Static(method) => format!("static {method}"),
            HirObjectCallTarget::Method { target, .. } => {
                format!("method {}", method_target(&target))
            }
            HirObjectCallTarget::Interface { target, .. } => {
                format!("interface {} {}", target.interface, target.requirement)
            }
        };
        self.line(&format!("ObjectCall {target} -> {}", call.class), call.span);
        self.indented(|dumper| {
            if let HirObjectCallTarget::Method { receiver, .. } = &call.target {
                dumper.method_receiver(receiver);
            }
            if let HirObjectCallTarget::Interface { receiver, .. } = &call.target {
                dumper.interface_receiver(receiver);
            }
            for argument in &call.arguments {
                dumper.call_argument(argument);
            }
        });
    }

    fn interface_receiver(&mut self, receiver: &HirInterfaceReceiver) {
        match receiver {
            HirInterfaceReceiver::View(view) => {
                self.call_argument(&HirCallArgument::View(view.clone()))
            }
            HirInterfaceReceiver::Checked(view) => {
                self.call_argument(&HirCallArgument::CheckedView(view.clone()))
            }
        }
    }

    fn call_argument(&mut self, argument: &HirCallArgument) {
        match argument {
            HirCallArgument::Value(expression) => {
                self.line("ValueArgument", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
            HirCallArgument::Optional { source, payload } => {
                self.line(
                    &format!("OptionalArgument {}?", payload.name()),
                    source.span(),
                );
                self.indented(|dumper| dumper.optional_source(source));
            }
            HirCallArgument::ClassOptional(value) => {
                self.line(
                    &format!("ClassOptionalArgument class {}?", value.class),
                    value.span,
                );
                self.indented(|dumper| dumper.class_optional_source(&value.source));
            }
            HirCallArgument::OptionalShared(value) => {
                self.line(
                    &format!(
                        "OptionalSharedArgument {}",
                        optional_shared_target_name(value.target)
                    ),
                    value.span,
                );
                self.indented(|dumper| dumper.optional_shared_source(&value.source));
            }
            HirCallArgument::AggregateOptional(value) => {
                self.line(
                    &format!("AggregateOptionalArgument {}", value.optional),
                    value.span,
                );
                self.indented(|dumper| dumper.aggregate_optional_value(value));
            }
            HirCallArgument::OptionalPlace(place) => match place {
                crate::hir::HirOptionalAliasPlace::Primitive(place) => {
                    self.line(
                        &format!("OptionalPlaceArgument {}?", place.payload.name()),
                        place.span,
                    );
                    self.indented(|dumper| dumper.optional_place(place));
                }
                crate::hir::HirOptionalAliasPlace::Class(place) => {
                    self.line(
                        &format!("OptionalPlaceArgument class {}?", place.class),
                        place.span,
                    );
                    self.indented(|dumper| dumper.class_optional_place(place));
                }
                crate::hir::HirOptionalAliasPlace::Nested(place) => {
                    self.line(
                        &format!("OptionalPlaceArgument {}", place.optional),
                        place.span,
                    );
                }
            },
            HirCallArgument::OptionalSharedPlace(place) => {
                self.line("OptionalSharedPlaceArgument", place.span);
                self.indented(|dumper| dumper.optional_shared_place(place));
            }
            HirCallArgument::Place(place) => {
                self.line("PlaceArgument", place.span());
                self.indented(|dumper| dumper.object_place(place));
            }
            HirCallArgument::View(view) => {
                self.object_view("ViewArgument", view);
            }
            HirCallArgument::CheckedView(view) => {
                let kind = match view.kind {
                    HirCheckedObjectViewKind::Static => "static",
                    HirCheckedObjectViewKind::RuntimeTerminate => "runtime-terminate",
                };
                self.object_view(&format!("CheckedViewArgument {kind}"), &view.view);
            }
            HirCallArgument::Copy(copy) => {
                self.line("CopyArgument", copy.span);
                self.indented(|dumper| {
                    dumper.object_source(&copy.source);
                    dumper.selected_copy_operation(copy.operation);
                });
            }
            HirCallArgument::Shared(value) => {
                self.line("SharedArgument", value.span);
                self.indented(|dumper| dumper.shared_transfer(value));
            }
            HirCallArgument::SharedPlace(place) => {
                self.line("SharedPlaceArgument", place.span());
                self.indented(|dumper| {
                    dumper.shared_source(&crate::hir::HirSharedSource::Place(place.clone()));
                });
            }
            HirCallArgument::Array(value) => {
                self.line("ArrayArgument", value.span);
                self.indented(|dumper| dumper.array_initialize(value));
            }
            HirCallArgument::ArrayAlias(value) => {
                self.line(
                    &format!(
                        "ArrayAliasArgument : {} access={:?}",
                        value.target.name(),
                        value.access
                    ),
                    value.span,
                );
                self.indented(|dumper| match &value.source {
                    crate::hir::HirArrayAliasSource::Whole(receiver) => {
                        dumper.array_receiver(receiver)
                    }
                    crate::hir::HirArrayAliasSource::Element(place) => dumper.array_element(place),
                    crate::hir::HirArrayAliasSource::OptionalPayload {
                        source,
                        optional,
                        array,
                    } => {
                        dumper.line(
                            &format!(
                                "CheckedOptionalArrayPayload optional={optional} array={array}"
                            ),
                            value.span,
                        );
                        dumper.indented(|dumper| dumper.optional_operand(source));
                    }
                });
            }
            HirCallArgument::PrimitivePlace(place) => {
                let storage = match place.storage {
                    crate::hir::HirPrimitiveStorage::Binding(binding) => {
                        format!("binding {binding}")
                    }
                    crate::hir::HirPrimitiveStorage::Static(place) => {
                        format!("static {}", place.field)
                    }
                };
                self.line(&format!("PrimitivePlaceArgument {storage}"), place.span);
            }
            HirCallArgument::ProducedPrimitiveAlias(expression) => {
                self.line(
                    &format!("ProducedPrimitiveAliasArgument : {}", expression.ty.name()),
                    expression.span,
                );
                self.indented(|dumper| dumper.expression(expression));
            }
        }
    }

    fn shared_transfer(&mut self, value: &HirSharedTransfer) {
        let operation = match value.operation {
            HirOwnerTransfer::Copy => "Copy",
            HirOwnerTransfer::Adopt => "Adopt",
        };
        self.line(
            &format!(
                "SharedTransfer {operation} -> {}",
                shared_target_name(value.target)
            ),
            value.span,
        );
        self.indented(|dumper| dumper.shared_source(&value.source));
    }

    fn shared_source(&mut self, source: &HirSharedSource) {
        match source {
            HirSharedSource::Place(HirSharedPlace::Binding {
                binding,
                target,
                span,
            }) => self.line(
                &format!("SharedBinding {binding} : {}", shared_target_name(*target)),
                *span,
            ),
            HirSharedSource::Place(HirSharedPlace::Field {
                place,
                target,
                span,
            }) => {
                self.line(
                    &format!(
                        "SharedField {} : {}",
                        place.field,
                        shared_target_name(*target)
                    ),
                    *span,
                );
                self.indented(|dumper| {
                    if let Some(receiver) = place.receiver.inspection_place() {
                        dumper.object_place(receiver);
                    } else {
                        dumper.field_place(place);
                    }
                });
            }
            HirSharedSource::Place(HirSharedPlace::ArrayElement {
                place,
                target,
                span,
            }) => {
                self.line(
                    &format!("SharedArrayElement : {}", shared_target_name(*target)),
                    *span,
                );
                self.indented(|dumper| dumper.array_element(place));
            }
            HirSharedSource::Place(HirSharedPlace::Static {
                place,
                target,
                span,
            }) => self.line(
                &format!(
                    "SharedStatic {} : {}",
                    place.field,
                    shared_target_name(*target)
                ),
                *span,
            ),
            HirSharedSource::Produced(HirSharedProducer::Allocation(allocation)) => {
                match &allocation.mode {
                    crate::hir::HirSharedAllocationMode::Initialize {
                        initializer,
                        arguments,
                    } => {
                        self.line(
                            &format!(
                                "SharedAllocation {} initialize via {}",
                                allocation.class, initializer
                            ),
                            allocation.span,
                        );
                        self.indented(|dumper| {
                            for argument in arguments {
                                dumper.call_argument(argument);
                            }
                        });
                    }
                    crate::hir::HirSharedAllocationMode::Copy { source, operation } => {
                        self.line(
                            &format!("SharedAllocation {} copy", allocation.class),
                            allocation.span,
                        );
                        self.indented(|dumper| {
                            dumper.selected_copy_operation(*operation);
                            dumper.object_source(source);
                        });
                    }
                }
            }
            HirSharedSource::Produced(HirSharedProducer::Call(call)) => {
                self.line("SharedCallResult", call.span);
                self.indented(|dumper| dumper.expression(call));
            }
            HirSharedSource::Produced(HirSharedProducer::Cast(cast)) => {
                let kind = match cast.kind {
                    crate::hir::HirSharedCastKind::Static => "static",
                    crate::hir::HirSharedCastKind::RuntimeTerminate => "runtime-terminate",
                };
                self.line(
                    &format!("SharedCast {kind} -> {}", shared_target_name(cast.target)),
                    cast.span,
                );
                self.indented(|dumper| dumper.shared_source(&cast.source));
            }
            HirSharedSource::Produced(HirSharedProducer::OptionalUnwrap { operand, .. }) => {
                self.line("OptionalSharedUnwrap", operand.span());
                self.indented(|dumper| dumper.optional_operand(operand));
            }
            HirSharedSource::Produced(HirSharedProducer::ArrayAllocation(construction)) => {
                self.array_construction(construction);
            }
            HirSharedSource::Produced(HirSharedProducer::OptionalBoxAllocation(allocation)) => {
                self.line(
                    &format!(
                        "OptionalBoxAllocation exact={} target={} static={} order={:?} owner={:?}",
                        allocation.exact_optional,
                        allocation.exact_target,
                        allocation.static_target,
                        allocation.evaluation,
                        allocation.produced_owner,
                    ),
                    allocation.span,
                );
                self.indented(|dumper| {
                    dumper.stored_value_initialization(&allocation.initialization)
                });
            }
        }
    }

    fn optional_shared_value(&mut self, value: &crate::hir::HirOptionalSharedInitialize) {
        self.line(
            &format!(
                "OptionalSharedInitialization {}",
                optional_shared_target_name(value.target)
            ),
            value.span,
        );
        self.indented(|dumper| dumper.optional_shared_source(&value.source));
    }

    fn optional_shared_source(&mut self, source: &crate::hir::HirOptionalSharedSource) {
        match source {
            crate::hir::HirOptionalSharedSource::Absent { span } => {
                self.line("OptionalSharedAbsent", *span)
            }
            crate::hir::HirOptionalSharedSource::Present(source) => {
                self.line("OptionalSharedPresent", source.span());
                self.indented(|dumper| dumper.shared_source(source));
            }
            crate::hir::HirOptionalSharedSource::Copy(place) => {
                self.line("OptionalSharedCopy", place.span);
                self.indented(|dumper| dumper.optional_shared_place(place));
            }
            crate::hir::HirOptionalSharedSource::Produced(expression) => {
                self.line("OptionalSharedProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
        }
    }

    fn optional_shared_place(&mut self, place: &crate::hir::HirOptionalSharedPlace) {
        match &place.storage {
            crate::hir::HirOptionalStorage::Binding(binding) => self.line(
                &format!(
                    "OptionalSharedPlace {binding} : {}",
                    optional_shared_target_name(place.target)
                ),
                place.span,
            ),
            crate::hir::HirOptionalStorage::Static(root) => self.line(
                &format!(
                    "OptionalSharedStaticPlace {} : {}",
                    root.field,
                    optional_shared_target_name(place.target)
                ),
                place.span,
            ),
            crate::hir::HirOptionalStorage::Field(field) => {
                self.line(
                    &format!(
                        "OptionalSharedFieldPlace : {}",
                        optional_shared_target_name(place.target)
                    ),
                    place.span,
                );
                self.indented(|dumper| dumper.field_place(field));
            }
            crate::hir::HirOptionalStorage::ArrayElement(element) => {
                self.line(
                    &format!(
                        "OptionalSharedArrayElementPlace : {}",
                        optional_shared_target_name(place.target)
                    ),
                    place.span,
                );
                self.indented(|dumper| dumper.array_element(element));
            }
            crate::hir::HirOptionalStorage::SharedPointee(pointee) => {
                self.optional_box_pointee("OptionalSharedBoxPointee", pointee);
            }
        }
    }

    fn optional_box_pointee(&mut self, label: &str, pointee: &crate::hir::HirOptionalBoxPointee) {
        self.line(
            &format!("{label} {} -> {}", pointee.target, pointee.optional),
            pointee.span,
        );
        self.indented(|dumper| dumper.shared_source(&pointee.source));
    }

    fn object_view(&mut self, label: &str, view: &HirObjectView) {
        self.line(
            &format!(
                "{label} -> {} {}",
                view_target_name(view.target),
                access_name(view.access)
            ),
            view.span,
        );
        self.indented(|dumper| {
            match &view.source {
                HirViewSource::Place(place) => dumper.object_place(place),
                HirViewSource::Static { place, projections } => {
                    dumper.line(&format!("StaticView {}", place.field), place.span);
                    dumper.indented(|dumper| {
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
                HirViewSource::ArrayElement(element) => dumper.array_element(element),
                HirViewSource::Produced {
                    producer,
                    projections,
                } => {
                    dumper.line("ProducedView", producer.span());
                    dumper.indented(|dumper| {
                        dumper.object_producer(producer);
                        for projection in projections {
                            match projection {
                                crate::object_path::ObjectProjection::Base(base) => {
                                    dumper.line(&format!("BaseProjection {base}"), producer.span());
                                }
                                crate::object_path::ObjectProjection::Field(field) => {
                                    dumper
                                        .line(&format!("FieldProjection {field}"), producer.span());
                                }
                            }
                        }
                    });
                }
                HirViewSource::Forwarded {
                    binding,
                    target,
                    access,
                    span,
                    ..
                } => dumper.line(
                    &format!(
                        "ForwardedView {binding} : {} {}",
                        view_target_name(*target),
                        access_name(*access)
                    ),
                    *span,
                ),
                HirViewSource::Shared {
                    binding,
                    target,
                    access,
                    span,
                    ..
                } => dumper.line(
                    &format!(
                        "SharedPointee {binding} : {} {}",
                        view_target_name(*target),
                        access_name(*access)
                    ),
                    *span,
                ),
                HirViewSource::AnchoredShared {
                    source,
                    target,
                    access,
                    span,
                    ..
                } => {
                    dumper.line(
                        &format!(
                            "AnchoredSharedPointee : {} {}",
                            view_target_name(*target),
                            access_name(*access)
                        ),
                        *span,
                    );
                    dumper.indented(|dumper| dumper.shared_source(source));
                }
                HirViewSource::OptionalPayload { view, projections } => {
                    dumper.line(
                        &format!(
                            "CheckedOptionalPayload class {} {}",
                            match &view.source {
                                HirOptionalOperand::ClassPlace(place) => place.class,
                                HirOptionalOperand::ClassProduced(expression) => {
                                    let Type::Optional(optional) = expression.ty else {
                                        unreachable!()
                                    };
                                    match self
                                        .optional_types
                                        .get(optional)
                                        .expect("optional view must name metadata")
                                        .storage
                                    {
                                        HirOptionalStorageCategory::InlineClass(class) => class,
                                        _ => unreachable!(
                                            "optional object view must use class metadata"
                                        ),
                                    }
                                }
                                _ => unreachable!("optional object view must use a class operand"),
                            },
                            access_name(view.access)
                        ),
                        view.span,
                    );
                    dumper.indented(|dumper| {
                        dumper.optional_operand(&view.source);
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
                HirViewSource::OptionalBoxPayload { view, projections } => {
                    dumper.line(
                        &format!(
                            "CheckedOptionalBoxPayload {} -> {} {}",
                            view.box_target,
                            view_target_name(view.target),
                            access_name(view.access)
                        ),
                        view.span,
                    );
                    dumper.indented(|dumper| {
                        dumper.shared_source(&view.source);
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
            dumper.object_origin(&view.origin);
        });
    }

    fn object_source(&mut self, source: &crate::hir::HirObjectSource) {
        match source {
            crate::hir::HirObjectSource::Place(place) => self.object_place(place),
            crate::hir::HirObjectSource::Static { place, class } => {
                self.line(
                    &format!("StaticObjectSource {} : {class}", place.field),
                    place.span,
                );
            }
            crate::hir::HirObjectSource::ArrayElement(place) => self.array_element(place),
            crate::hir::HirObjectSource::Produced(producer) => {
                self.line("MaterializedSource", producer.span());
                self.indented(|dumper| dumper.object_producer(producer));
            }
            crate::hir::HirObjectSource::Checked(view) => {
                let kind = match view.kind {
                    HirCheckedObjectViewKind::Static => "static",
                    HirCheckedObjectViewKind::RuntimeTerminate => "runtime-terminate",
                };
                self.line(
                    &format!(
                        "CheckedSource {kind} -> {} {}",
                        view_target_name(view.consumer_target),
                        access_name(view.consumer_access)
                    ),
                    view.span,
                );
                self.indented(|dumper| dumper.object_view("SelectedView", &view.view));
            }
            crate::hir::HirObjectSource::Slice(slice) => {
                let path = slice
                    .bases
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" -> ");
                self.line(
                    &format!("SliceSource [{path}] -> {}", slice.target),
                    slice.span,
                );
                self.indented(|dumper| dumper.object_source(&slice.source));
            }
        }
    }

    fn object_producer(&mut self, producer: &crate::hir::HirObjectProducer) {
        match producer {
            crate::hir::HirObjectProducer::StringLiteral(literal) => {
                self.line(
                    &format!("StringLiteral {} class {}", literal.data, literal.class),
                    literal.span,
                );
            }
            crate::hir::HirObjectProducer::Construct(construction) => {
                self.construction(construction)
            }
            crate::hir::HirObjectProducer::Call(call) => self.object_call(call),
            crate::hir::HirObjectProducer::IndirectCall(call) => {
                self.line(
                    &format!(
                        "IndirectObjectCall type {} -> {}",
                        call.function_type,
                        call.result.name()
                    ),
                    call.span,
                );
                self.indented(|dumper| {
                    dumper.heading("Callee");
                    dumper.indented(|dumper| dumper.expression(&call.callee));
                    dumper.heading("Arguments");
                    dumper.indented(|dumper| {
                        for argument in &call.arguments {
                            dumper.call_argument(argument);
                        }
                    });
                });
            }
        }
    }

    fn field_place(&mut self, place: &HirFieldPlace) {
        self.line(&format!("FieldPlace {}", place.field), place.span);
        self.indented(|dumper| {
            if place.write_authorization == Some(HirFieldWriteAuthorization::DeclaringClassCell) {
                dumper.raw_line(&format!(
                    "WriteAuthorization DeclaringClassCell {}",
                    place.field
                ));
            }
            if let Some(HirFieldWriteAuthorization::DeclaringClassFinalAssignment(operation)) =
                place.write_authorization
            {
                dumper.raw_line(&format!(
                    "WriteAuthorization DeclaringClassFinalAssignment {} {}",
                    place.field, operation
                ));
            }
            match &place.receiver {
                HirObjectReceiver::ArrayElement {
                    element,
                    place: receiver,
                    ..
                } => {
                    dumper.array_element(element);
                    dumper.object_place(receiver);
                }
                HirObjectReceiver::View { view, .. } => {
                    let label = receiver_view_label(view, "FieldReceiver");
                    dumper.object_view(&label, view);
                }
                HirObjectReceiver::Place {
                    place: receiver, ..
                }
                | HirObjectReceiver::Checked {
                    place: receiver, ..
                } => dumper.object_place(receiver),
            }
        });
    }

    fn object_place(&mut self, place: &HirObjectPlace) {
        let access = match place.access {
            HirAccess::ReadOnly => "readonly",
            HirAccess::Mutable => "mutable",
        };
        self.line(
            &format!(
                "ObjectPlace {} : class {} {access}",
                place.path.render_identity(),
                place.class()
            ),
            place.span(),
        );
    }

    fn method_receiver(&mut self, receiver: &HirMethodReceiver) {
        self.heading("Receiver");
        self.indented(|dumper| match receiver {
            HirObjectReceiver::ArrayElement {
                element,
                place,
                origin,
            } => {
                dumper.array_element(element);
                dumper.object_place(place);
                dumper.object_origin(origin);
            }
            HirObjectReceiver::View { view, .. } => {
                let label = receiver_view_label(view, "MethodReceiver");
                dumper.object_view(&label, view);
            }
            HirObjectReceiver::Place { place, origin }
            | HirObjectReceiver::Checked { place, origin, .. } => {
                dumper.object_place(place);
                dumper.object_origin(origin);
            }
        });
    }

    fn object_origin(&mut self, origin: &HirObjectOrigin) {
        match origin {
            HirObjectOrigin::Exact {
                complete,
                dynamic_class,
            } => {
                self.heading(&format!("Origin Exact dynamic {dynamic_class}"));
                self.indented(|dumper| dumper.object_place(complete));
            }
            HirObjectOrigin::Static {
                place,
                dynamic_class,
            } => self.line(
                &format!("Origin Static {} dynamic {dynamic_class}", place.field),
                place.span,
            ),
            HirObjectOrigin::Forwarded {
                binding,
                static_target,
                access,
                dispatch_limit,
                span,
            } => {
                let limit = dispatch_limit
                    .map(|class| format!(" limit {class}"))
                    .unwrap_or_default();
                self.line(
                    &format!(
                        "Origin Forwarded {binding} : {} {}{limit}",
                        view_target_name(*static_target),
                        access_name(*access)
                    ),
                    *span,
                );
            }
            HirObjectOrigin::Produced {
                dynamic_class,
                span,
            } => self.line(&format!("Origin Produced dynamic {dynamic_class}"), *span),
            HirObjectOrigin::Shared {
                binding,
                static_target,
                access,
                span,
            } => self.line(
                &format!(
                    "Origin Shared {binding} : {} {}",
                    view_target_name(*static_target),
                    access_name(*access)
                ),
                *span,
            ),
            HirObjectOrigin::AnchoredShared {
                static_target,
                access,
                span,
            } => self.line(
                &format!(
                    "Origin AnchoredShared : {} {}",
                    view_target_name(*static_target),
                    access_name(*access)
                ),
                *span,
            ),
        }
    }

    fn typed_line(&mut self, name: &str, expression: &HirExpression) {
        self.write_indentation();
        let _ = write!(self.output, "{name} : {}", self.type_name(expression.ty));
        write_span(&mut self.output, expression.span);
        self.output.push('\n');
    }

    fn heading(&mut self, name: &str) {
        self.write_indentation();
        let _ = writeln!(self.output, "{name}");
    }

    fn raw_line(&mut self, text: &str) {
        self.heading(text);
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

const fn bitwise_title(operation: crate::hir::HirIntegerBitwiseOperation) -> &'static str {
    match operation {
        crate::hir::HirIntegerBitwiseOperation::And => "And",
        crate::hir::HirIntegerBitwiseOperation::Or => "Or",
        crate::hir::HirIntegerBitwiseOperation::Xor => "Xor",
    }
}

fn method_target(target: &HirMethodCallTarget) -> String {
    match target {
        HirMethodCallTarget::Direct(method) => format!("Direct {method}"),
        HirMethodCallTarget::Virtual {
            family,
            slot,
            selected,
        } => format!("Virtual {family} slot {slot} selected {selected}"),
    }
}

fn view_target_name(target: HirViewTarget) -> String {
    match target {
        HirViewTarget::Class(class) => format!("class {class}"),
        HirViewTarget::Interface(interface) => format!("interface {interface}"),
        HirViewTarget::Obj => "Obj".to_owned(),
    }
}

fn shared_target_name(target: HirSharedTarget) -> String {
    match target {
        HirSharedTarget::Class(class) => format!("shared class {class}"),
        HirSharedTarget::Interface(interface) => format!("shared interface {interface}"),
        HirSharedTarget::Obj => "shared Obj".to_owned(),
        HirSharedTarget::Array(array) => format!("shared array {array}"),
        HirSharedTarget::OptionalBox(target) => format!("shared optional-box {target}"),
    }
}

fn optional_shared_target_name(target: HirSharedTarget) -> String {
    shared_target_name(target).replacen("shared ", "shared? ", 1)
}

fn array_default_name(element: HirArrayDefaultElement) -> String {
    match element {
        HirArrayDefaultElement::Primitive => "primitive-zero".to_owned(),
        HirArrayDefaultElement::OptionalAbsent => "optional-absent".to_owned(),
        HirArrayDefaultElement::Class { class, initializer } => {
            format!("class {class} via {initializer}")
        }
        HirArrayDefaultElement::ArrayEmpty(array) => format!("empty-array {array}"),
        HirArrayDefaultElement::SharedClass { class, initializer } => {
            format!("shared-class {class} via {initializer}")
        }
        HirArrayDefaultElement::SharedArrayEmpty(array) => {
            format!("shared-empty-array {array}")
        }
        HirArrayDefaultElement::SharedOptionalBoxAbsent(target) => {
            format!("shared-optional-box-absent {target}")
        }
    }
}

fn array_copy_name(element: HirArrayCopyElement) -> String {
    match element {
        HirArrayCopyElement::Primitive => "primitive".to_owned(),
        HirArrayCopyElement::OptionalPrimitive => "optional-primitive".to_owned(),
        HirArrayCopyElement::Class { class, operation } => {
            format!("class {class} via {}", selected_operation_name(operation))
        }
        HirArrayCopyElement::OptionalClass { class, operation } => {
            format!(
                "optional-class {class} via {}",
                selected_operation_name(operation)
            )
        }
        HirArrayCopyElement::Array(array) => format!("array {array}"),
        HirArrayCopyElement::Shared(target) => shared_target_name(target),
        HirArrayCopyElement::OptionalShared(target) => optional_shared_target_name(target),
        HirArrayCopyElement::Optional(optional) => format!("optional {optional}"),
    }
}

fn array_assignment_name(element: HirArrayAssignElement) -> String {
    match element {
        HirArrayAssignElement::Primitive => "primitive".to_owned(),
        HirArrayAssignElement::OptionalPrimitive => "optional-primitive".to_owned(),
        HirArrayAssignElement::Class { class, operation } => {
            format!("class {class} via {}", selected_operation_name(operation))
        }
        HirArrayAssignElement::OptionalClass {
            class,
            copy_constructor,
            copy_assignment,
        } => {
            format!(
                "optional-class {class} construct-via {} assign-via {}",
                selected_operation_name(copy_constructor),
                selected_operation_name(copy_assignment)
            )
        }
        HirArrayAssignElement::Array(array) => format!("array {array}"),
        HirArrayAssignElement::Shared(target) => shared_target_name(target),
        HirArrayAssignElement::OptionalShared(target) => optional_shared_target_name(target),
        HirArrayAssignElement::Optional(optional) => format!("optional {optional}"),
    }
}

fn selected_operation_name<I: Display>(operation: HirSelectedCopyOperation<I>) -> String {
    match operation {
        HirSelectedCopyOperation::User(id) => format!("user {id}"),
        HirSelectedCopyOperation::Synthesized(class) => format!("synthesized {class}"),
    }
}

fn array_destruction_name(element: HirArrayDestroyElement) -> String {
    match element {
        HirArrayDestroyElement::Trivial => "trivial".to_owned(),
        HirArrayDestroyElement::Class(class) => format!("class {class}"),
        HirArrayDestroyElement::OptionalClass(class) => format!("optional-class {class}"),
        HirArrayDestroyElement::Array(array) => format!("array {array}"),
        HirArrayDestroyElement::Shared(target) => shared_target_name(target),
        HirArrayDestroyElement::OptionalShared(target) => optional_shared_target_name(target),
        HirArrayDestroyElement::Optional(optional) => format!("optional {optional}"),
    }
}

fn receiver_view_label(view: &HirObjectView, suffix: &str) -> String {
    let provenance = match &view.source {
        HirViewSource::Shared { .. } | HirViewSource::AnchoredShared { .. } => "Shared",
        HirViewSource::OptionalPayload { .. } | HirViewSource::OptionalBoxPayload { .. } => {
            "Optional"
        }
        HirViewSource::Produced { .. } => "Produced",
        HirViewSource::ArrayElement(_) => "ArrayElement",
        HirViewSource::Static { .. } => "Static",
        HirViewSource::Place(_) | HirViewSource::Forwarded { .. } => "Object",
    };
    format!("{provenance}{suffix}")
}

const fn parameter_mode_name(mode: HirParameterMode) -> &'static str {
    match mode {
        HirParameterMode::Value => "value",
        HirParameterMode::ReadOnlyAlias => "ref",
        HirParameterMode::MutableAlias => "mut-ref",
    }
}

const fn access_name(access: HirAccess) -> &'static str {
    match access {
        HirAccess::ReadOnly => "readonly",
        HirAccess::Mutable => "mutable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::{BindingId, ClassId, FieldId, FunctionId, LocalId},
        object_path::ObjectPath,
        source::SourceDatabase,
    };

    #[test]
    fn object_place_dump_renders_the_complete_identity_path_exactly() {
        let mut sources = SourceDatabase::new();
        let source = sources.add("place.ska", "root.link.leaf");
        let span = sources.get(source).unwrap().span(0, 14).unwrap();
        let root = BindingId::Local(LocalId::new(FunctionId::new(0), 0));
        let path = ObjectPath::root(root, ClassId::new(2), span)
            .project_field(FieldId::new(ClassId::new(2), 0), ClassId::new(1), span)
            .project_field(FieldId::new(ClassId::new(1), 3), ClassId::new(0), span);
        let place = HirObjectPlace {
            path,
            access: HirAccess::ReadOnly,
        };
        let optional_types = HirOptionalTypeTable::default();
        let optional_box_types = HirOptionalBoxTypeTable::default();
        let function_types = HirFunctionTypeTable::default();
        let mut dumper = HirDumper::new(&function_types, &optional_types, &optional_box_types);

        dumper.object_place(&place);

        assert_eq!(
            dumper.output,
            "ObjectPlace f0:l0 -> c2:field0 -> c1:field3 : class c0 readonly @0..14\n"
        );
    }
}
