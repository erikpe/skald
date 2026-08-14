//! Deterministic textual rendering of MIR.

use std::fmt::Write;

use crate::dump_format::{write_quoted, write_span};

use super::model::*;

pub fn dump_mir(program: &MirProgram) -> String {
    dump_program(program, "MirProgram")
}

pub fn dump_preliminary_mir(program: &PreliminaryMirProgram) -> String {
    let mut output = dump_program(program.program(), "PreliminaryMirProgram");
    output.push_str("  StaticInitializationModes\n");
    for field in program.static_fields() {
        output.push_str("    StaticField ");
        write_static_field_reference(&mut output, program.program(), field.field);
        output.push(' ');
        match field.initializer {
            Some(initializer) => {
                let _ = write!(output, "explicit {initializer}");
            }
            None => output.push_str("zero-default"),
        }
        let _ = write!(output, " : {}", field.ty);
        write_span(&mut output, field.span);
        output.push('\n');
    }
    if program.has_static_initializers() {
        output.push_str("  StaticInitializers\n");
        for initializer in program.static_initializers() {
            let _ = write!(
                output,
                "    StaticInitializer {} destination ",
                initializer.id
            );
            write_static_field_reference(&mut output, program.program(), initializer.field);
            let _ = write!(output, " : {}", initializer.destination_type);
            dump_executable_body(&mut output, initializer.into());
            let _ = write!(
                output,
                "      Publication {} -> {}",
                initializer.publication.initialization_exit, initializer.publication.cleanup_entry,
            );
            write_span(&mut output, initializer.publication.span);
            output.push('\n');
        }
    }
    output
}

fn dump_program(program: &MirProgram, heading: &str) -> String {
    let mut output = String::new();
    output.push_str(heading);
    write_span(&mut output, program.span);
    output.push('\n');
    let _ = writeln!(output, "  SelectedModule {}", program.modules.selected());
    output.push_str("  Modules\n");
    for module in program.modules.iter() {
        let _ = writeln!(
            output,
            "    Module {} {} source {} provider {} package {}",
            module.module_id(),
            module.module_path(),
            module.source_id().index(),
            module.provider_id(),
            module.package_id()
        );
    }
    if !program.external_links.is_empty() {
        output.push_str("  ExternalLinks\n");
        for link in program.external_links.iter() {
            let _ = write!(output, "    Link {} ", link.id);
            write_quoted(&mut output, &link.symbol);
            output.push_str(" declarations");
            for declaration in &link.declarations {
                let _ = write!(output, " {declaration}");
            }
            output.push('\n');
        }
    }
    let _ = writeln!(output, "  Entry {}", program.entry_function);
    if !program.array_types.is_empty() {
        output.push_str("  ArrayTypes\n");
        for array in program.array_types.iter() {
            let _ = writeln!(
                output,
                "    Array {} element {} default {:?} copy {:?} assign {:?} destroy {:?}",
                array.id,
                array.element,
                array.lifecycle.default,
                array.lifecycle.copy,
                array.lifecycle.assignment,
                array.lifecycle.destruction
            );
        }
    }
    if !program.optional_types.is_empty() {
        output.push_str("  OptionalTypes\n");
        for optional in program.optional_types.iter() {
            let _ = writeln!(
                output,
                "    Optional {} payload {} storage {:?} representation {:?} initialize {:?} inject {:?} copy {:?} assign {:?} cleanup {:?} presence {:?} unwrap {:?} access {:?} argument {:?} result {:?} static {:?} array-element {:?}",
                optional.id,
                optional.payload,
                optional.storage,
                optional.representation,
                optional.lifecycle.initialization,
                optional.lifecycle.injection,
                optional.lifecycle.copy,
                optional.lifecycle.assignment,
                optional.lifecycle.cleanup,
                optional.lifecycle.presence,
                optional.lifecycle.unwrap,
                optional.checked_access,
                optional.boundaries.argument,
                optional.boundaries.result,
                optional.boundaries.static_storage,
                optional.boundaries.array_element,
            );
        }
    }
    if !program.optional_box_types.is_empty() {
        output.push_str("  OptionalBoxTypes\n");
        for target in program.optional_box_types.iter() {
            let _ = write!(
                output,
                "    OptionalBox {} exact {} dynamic {:?} depth {} view ",
                target.id,
                target
                    .exact_optional
                    .map(|optional| optional.to_string())
                    .unwrap_or_else(|| "view-only".to_owned()),
                target.exact_dynamic_class,
                target.optional_depth,
            );
            match target.object_view {
                Some(view) => dump_view_target(&mut output, view),
                None => output.push_str("none"),
            }
            write_span(&mut output, target.span);
            output.push('\n');
        }
    }
    if let Some(item) = program.string_language_item {
        let _ = writeln!(
            output,
            "  StringLanguageItem class {} storage {} start {} length {} storage-array {}",
            item.class, item.storage_field, item.start_field, item.length_field, item.storage_array
        );
    }
    if !program.literal_data.is_empty() {
        output.push_str("  LiteralData\n");
        for data in program.literal_data.iter() {
            let _ = write!(
                output,
                "    Literal {} array {} length {} {:?} {:?} bytes",
                data.id, data.array, data.length, data.mutability, data.origin
            );
            for byte in &data.bytes {
                let _ = write!(output, " {byte:02x}");
            }
            write_span(&mut output, data.span);
            output.push('\n');
        }
    }
    if !program.virtual_families.is_empty() {
        output.push_str("  VirtualFamilies\n");
        for family in program.virtual_families.iter() {
            let _ = write!(
                output,
                "    Family {} slot {} root {} members",
                family.id, family.slot, family.root
            );
            for member in &family.members {
                let _ = write!(output, " {member}");
            }
            output.push('\n');
        }
    }
    if !program.interfaces.is_empty() {
        output.push_str("  Interfaces\n");
        for interface in program.interfaces.iter() {
            let _ = write!(
                output,
                "    Interface {} module {} ",
                interface.id, interface.module
            );
            write_quoted(&mut output, &interface.name);
            write_span(&mut output, interface.span);
            output.push('\n');
            for requirement in &interface.requirements {
                let _ = write!(output, "      Requirement {} ", requirement.id);
                write_quoted(&mut output, &requirement.name);
                let _ = write!(output, " {} (", requirement.receiver_access);
                dump_parameters(&mut output, &requirement.parameters);
                let _ = write!(output, ") -> {}", requirement.return_type);
                write_span(&mut output, requirement.span);
                output.push('\n');
            }
        }
    }
    output.push_str("  Classes\n");
    for class in program.classes.iter() {
        dump_class(&mut output, class);
    }
    output.push_str("  Declarations\n");
    for declaration in program.declarations.iter() {
        dump_declaration(&mut output, declaration);
    }
    output.push_str("  Definitions\n");
    for definition in program.definitions.iter() {
        dump_definition(&mut output, definition);
    }
    if !program.member_definitions.is_empty() {
        output.push_str("  MemberDefinitions\n");
        for definition in program.member_definitions.iter() {
            dump_member_definition(&mut output, definition);
        }
    }
    if let Some(coordinator) = &program.static_lifecycle {
        dump_static_lifecycle_coordinator(&mut output, program, coordinator);
    }
    output
}

fn dump_static_lifecycle_coordinator(
    output: &mut String,
    program: &MirProgram,
    coordinator: &MirStaticLifecycleCoordinator,
) {
    output.push_str("  StaticLifecycleCoordinator\n");
    let lifecycle = coordinator.lifecycle();
    let _ = writeln!(
        output,
        "    Certificate summaries={} dependencies={}",
        lifecycle.certificate().effects().summaries().len(),
        lifecycle.certificate().dependencies().len()
    );
    output.push_str("    ActivationRegions\n");
    for region in coordinator.activation() {
        output.push_str("      Field ");
        write_static_field_reference(output, program, region.field);
        let _ = writeln!(output, " {:?}", region.work);
        for transition in &region.transitions {
            let _ = write!(output, "        {:?}", transition.kind);
            write_span(output, transition.span);
            output.push('\n');
        }
    }
    if !coordinator.initializers().is_empty() {
        output.push_str("    InitializerBodies\n");
        for initializer in coordinator.initializers() {
            let _ = write!(
                output,
                "      StaticInitializer {} destination ",
                initializer.id
            );
            write_static_field_reference(output, program, initializer.field);
            let _ = write!(output, " : {}", initializer.destination_type);
            dump_executable_body(output, initializer.into());
            let _ = write!(
                output,
                "        Publication {} -> {}",
                initializer.publication.initialization_exit, initializer.publication.cleanup_entry,
            );
            write_span(output, initializer.publication.span);
            output.push('\n');
        }
    }
    output.push_str("    DestructionRegions\n");
    for region in coordinator.shutdown() {
        output.push_str("      Field ");
        write_static_field_reference(output, program, region.field);
        output.push('\n');
        let _ = write!(output, "        {:?}", region.begin.kind);
        write_span(output, region.begin.span);
        output.push('\n');
        dump_static_cleanup(output, &region.cleanup);
        let _ = write!(output, "        {:?}", region.finish.kind);
        write_span(output, region.finish.span);
        output.push('\n');
    }
}

fn write_static_field_reference(
    output: &mut String,
    program: &MirProgram,
    field: crate::identity::StaticFieldId,
) {
    let _ = write!(output, "{field}");
    if let Some(name) = program.static_field_qualified_name(field) {
        output.push(' ');
        write_quoted(output, &name);
    }
}

fn dump_static_cleanup(output: &mut String, cleanup: &MirStaticValueCleanup) {
    match cleanup {
        MirStaticValueCleanup::None => output.push_str("        Cleanup none\n"),
        MirStaticValueCleanup::CompleteObject(cleanup) => {
            let _ = write!(output, "        Cleanup class {} ", cleanup.target);
            dump_place(output, &cleanup.destination);
            write_span(output, cleanup.span);
            output.push('\n');
        }
        MirStaticValueCleanup::OptionalClass(cleanup) => {
            let _ = write!(output, "        Cleanup optional-class {} ", cleanup.class);
            dump_place(output, &cleanup.destination);
            write_span(output, cleanup.span);
            output.push('\n');
        }
        MirStaticValueCleanup::AggregateOptional(cleanup) => {
            let _ = write!(
                output,
                "        Cleanup aggregate-optional {} ",
                cleanup.optional
            );
            dump_place(output, &cleanup.destination);
            write_span(output, cleanup.span);
            output.push('\n');
        }
        MirStaticValueCleanup::Shared(cleanup) => {
            let _ = write!(output, "        Cleanup shared {} ", cleanup.target);
            dump_place(output, &cleanup.destination);
            write_span(output, cleanup.span);
            output.push('\n');
        }
        MirStaticValueCleanup::OptionalShared(cleanup) => {
            let _ = write!(
                output,
                "        Cleanup optional-shared {} ",
                cleanup.target
            );
            dump_place(output, &cleanup.destination);
            write_span(output, cleanup.span);
            output.push('\n');
        }
        MirStaticValueCleanup::Array(MirArrayInstruction::Release { owner, array, span }) => {
            let _ = write!(output, "        Cleanup array {array} ");
            dump_place(output, owner);
            write_span(output, *span);
            output.push('\n');
        }
        MirStaticValueCleanup::Array(_) => {
            output.push_str("        Cleanup malformed-array-operation\n");
        }
    }
}

fn dump_class(output: &mut String, class: &MirClassDeclaration) {
    let _ = write!(output, "    Class {} module {} ", class.id, class.module);
    write_quoted(output, &class.name);
    write_span(output, class.span);
    output.push('\n');
    if let Some(base) = class.direct_base {
        let _ = write!(output, "      DirectBase {}", base.class);
        write_span(output, base.span);
        output.push('\n');
    }
    for conformance in &class.conformances {
        let _ = writeln!(output, "      Conformance {}", conformance.interface);
        for implementation in &conformance.implementations {
            let _ = writeln!(
                output,
                "        {} -> {}",
                implementation.requirement, implementation.method
            );
        }
    }
    for field in &class.fields {
        let _ = write!(output, "      Field {} ", field.id);
        write_quoted(output, &field.name);
        let _ = write!(output, " : {}", field.ty);
        write_span(output, field.span);
        output.push('\n');
    }
    for field in &class.static_fields {
        let _ = write!(output, "      StaticField {} ", field.id);
        write_quoted(output, &field.name);
        let _ = write!(output, " : {} {}", field.ty, field.initialization);
        if let Some(indices) = field.lifecycle {
            let _ = write!(
                output,
                " activation={} shutdown={}",
                indices.activation, indices.shutdown
            );
        }
        write_span(output, field.span);
        output.push('\n');
    }
    for initializer in &class.initializers {
        let _ = write!(output, "      Initializer {}(", initializer.id);
        dump_parameters(output, &initializer.parameters);
        output.push(')');
        write_span(output, initializer.span);
        output.push('\n');
    }
    dump_copy_capability(output, "CopyConstructor", &class.copy_constructor);
    dump_copy_capability(output, "CopyAssignment", &class.copy_assignment);
    if !class.destruction.steps.is_empty() {
        output.push_str("      DestructionPlan\n");
        if let Some(destructor) = &class.destruction.destructor {
            let _ = write!(
                output,
                "        Destructor {} {}",
                destructor.id, destructor.receiver_access
            );
            write_span(output, destructor.span);
            output.push('\n');
        }
        for step in &class.destruction.steps {
            match step {
                MirDestructionStep::UserBody(destructor) => {
                    let _ = writeln!(output, "        UserBody {destructor}");
                }
                MirDestructionStep::Field(field) => {
                    let _ = writeln!(output, "        Field {field}");
                }
                MirDestructionStep::SharedField(field) => {
                    let _ = writeln!(output, "        SharedField {field}");
                }
                MirDestructionStep::OptionalSharedField(field) => {
                    let _ = writeln!(output, "        OptionalSharedField {field}");
                }
                MirDestructionStep::OptionalClassField(field) => {
                    let _ = writeln!(output, "        OptionalClassField {field}");
                }
                MirDestructionStep::OptionalField { field, optional } => {
                    let _ = writeln!(output, "        OptionalField {field} : {optional}");
                }
                MirDestructionStep::ArrayField(field) => {
                    let _ = writeln!(output, "        ArrayField {field}");
                }
                MirDestructionStep::Base(base) => {
                    let _ = writeln!(output, "        Base {base}");
                }
            }
        }
    }
    for method in &class.methods {
        let _ = write!(output, "      Method {} ", method.id);
        write_quoted(output, &method.name);
        match method.kind {
            MirMethodKind::Instance { receiver_access } => {
                let _ = write!(output, " {receiver_access} (");
            }
            MirMethodKind::Static => output.push_str(" static ("),
        }
        dump_parameters(output, &method.parameters);
        let _ = write!(output, ") -> {}", method.return_type);
        write_span(output, method.span);
        output.push('\n');
    }
}

fn dump_copy_capability<I: Copy + std::fmt::Display>(
    output: &mut String,
    label: &str,
    capability: &MirCopyCapability<I>,
) {
    let _ = writeln!(output, "      {label}");
    match capability {
        MirCopyCapability::User(copy) => {
            let _ = writeln!(output, "        User {}", copy.operation);
            dump_base_copy(output, copy.base);
        }
        MirCopyCapability::Synthesized(copy) => {
            let _ = writeln!(output, "        Synthesized {}", copy.class);
            dump_base_copy(output, copy.base);
            for field in &copy.fields {
                match field {
                    MirSynthesizedFieldCopy::Primitive { field } => {
                        let _ = writeln!(output, "          Primitive {field}");
                    }
                    MirSynthesizedFieldCopy::OptionalPrimitive { field, payload } => {
                        let _ =
                            writeln!(output, "          OptionalPrimitive {field} : {payload}?");
                    }
                    MirSynthesizedFieldCopy::Shared { field } => {
                        let _ = writeln!(output, "          Shared {field}");
                    }
                    MirSynthesizedFieldCopy::OptionalShared { field, target } => {
                        let _ = writeln!(
                            output,
                            "          OptionalShared {field} : shared? {target}"
                        );
                    }
                    MirSynthesizedFieldCopy::Optional { field, optional } => {
                        let _ = writeln!(output, "          Optional {field} : {optional}");
                    }
                    MirSynthesizedFieldCopy::OptionalClass {
                        field,
                        class,
                        operation,
                    } => {
                        let _ = write!(
                            output,
                            "          OptionalClass {field} : class {class}? via "
                        );
                        dump_copy_operation(output, *operation);
                        output.push('\n');
                    }
                    MirSynthesizedFieldCopy::Class { field, operation } => {
                        let _ = write!(output, "          Class {field} via ");
                        dump_copy_operation(output, *operation);
                        output.push('\n');
                    }
                    MirSynthesizedFieldCopy::Array { field, array } => {
                        let _ = writeln!(output, "          Array {field} : {array}");
                    }
                }
            }
        }
        MirCopyCapability::Unavailable => output.push_str("        Unavailable\n"),
    }
}

fn dump_base_copy<I: Copy + std::fmt::Display>(output: &mut String, copy: Option<MirBaseCopy<I>>) {
    if let Some(copy) = copy {
        let _ = write!(output, "          Base {} via ", copy.base);
        dump_copy_operation(output, copy.operation);
        output.push('\n');
    }
}

fn dump_copy_operation<I: std::fmt::Display>(
    output: &mut String,
    operation: MirSelectedCopyOperation<I>,
) {
    match operation {
        MirSelectedCopyOperation::User(id) => {
            let _ = write!(output, "user {id}");
        }
        MirSelectedCopyOperation::Synthesized(class) => {
            let _ = write!(output, "synthesized {class}");
        }
    }
}

fn dump_parameters(output: &mut String, parameters: &[MirParameter]) {
    for (index, parameter) in parameters.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        match parameter.mode {
            MirParameterMode::Value => {}
            MirParameterMode::ReadOnlyAlias => output.push_str("ref "),
            MirParameterMode::MutableAlias => output.push_str("mut ref "),
        }
        let _ = write!(output, "{}", parameter.ty);
    }
}

fn dump_declaration(output: &mut String, declaration: &MirFunctionDeclaration) {
    let _ = write!(
        output,
        "    Declaration {} module {} ",
        declaration.id, declaration.module
    );
    write_quoted(output, &declaration.name);
    match &declaration.linkage {
        MirFunctionLinkage::Internal => output.push_str(" internal"),
        MirFunctionLinkage::External { link } => {
            let _ = write!(output, " external {link}");
        }
        MirFunctionLinkage::Intrinsic { intrinsic } => {
            let _ = write!(output, " intrinsic {intrinsic:?}");
        }
    }
    write_span(output, declaration.span);
    output.push('\n');
    output.push_str("      Signature (");
    dump_parameters(output, &declaration.parameters);
    let _ = writeln!(output, ") -> {}", declaration.return_type);
}

fn dump_definition(output: &mut String, function: &MirFunctionDefinition) {
    let _ = write!(output, "    Definition {}", function.function);
    dump_executable_body(output, function.into());
}

fn dump_member_definition(output: &mut String, function: &MirMemberDefinition) {
    let _ = write!(output, "    MemberDefinition {}", function.callable);
    dump_executable_body(output, function.into());
}

fn dump_executable_body(output: &mut String, function: MirDefinitionRef<'_>) {
    write_span(output, function.span());
    output.push('\n');
    if let Some(receiver) = function.receiver() {
        let _ = writeln!(output, "      Receiver {receiver}");
    }
    if let Some(storage) = function.return_storage() {
        let _ = writeln!(output, "      ReturnStorage {storage}");
    }
    output.push_str("      Parameters");
    for parameter in function.parameters() {
        let _ = write!(output, " {parameter}");
    }
    output.push('\n');
    output.push_str("      Storage\n");
    for storage in function.storage_entries() {
        let kind = match storage.kind {
            MirStorageKind::Return => "return",
            MirStorageKind::Receiver => "receiver",
            MirStorageKind::Parameter => "parameter",
            MirStorageKind::AliasParameter(MirAliasAccess::ReadOnly) => "ref-parameter",
            MirStorageKind::AliasParameter(MirAliasAccess::Mutable) => "mut-ref-parameter",
            MirStorageKind::CheckedView(MirAliasAccess::ReadOnly) => "checked-view",
            MirStorageKind::CheckedView(MirAliasAccess::Mutable) => "checked-mut-view",
            MirStorageKind::Local => "local",
            MirStorageKind::Argument => "argument",
            MirStorageKind::Temporary => "temporary",
            MirStorageKind::SharedAnchor => "shared-anchor",
            MirStorageKind::ScalarSpill => "scalar-spill",
            MirStorageKind::PathCondition => "path-condition",
            MirStorageKind::OptionalUnwrap => "optional-unwrap",
            MirStorageKind::SharedAllocation => "shared-allocation",
            MirStorageKind::ArrayBacking => "array-backing",
            MirStorageKind::ArrayProduced => "array-produced",
            MirStorageKind::ArraySlice => "array-slice",
            MirStorageKind::ArrayPosition => "array-position",
            MirStorageKind::ArrayAnchor(_) => "array-anchor",
            MirStorageKind::ArrayAlias(MirAliasAccess::ReadOnly) => "array-alias",
            MirStorageKind::ArrayAlias(MirAliasAccess::Mutable) => "array-mut-alias",
        };
        let _ = write!(output, "        {} {kind} ", storage.id);
        match storage.source {
            Some(source) => {
                let _ = write!(output, "{source} ");
            }
            None => match storage.kind {
                MirStorageKind::Return => output.push_str("<return> "),
                MirStorageKind::Argument => output.push_str("<argument> "),
                MirStorageKind::Temporary => output.push_str("<temporary> "),
                MirStorageKind::SharedAnchor => output.push_str("<shared-anchor> "),
                MirStorageKind::CheckedView(_) => output.push_str("<checked-view> "),
                MirStorageKind::ScalarSpill => output.push_str("<scalar-spill> "),
                MirStorageKind::PathCondition => output.push_str("<path-condition> "),
                MirStorageKind::OptionalUnwrap => output.push_str("<optional-unwrap> "),
                MirStorageKind::SharedAllocation => output.push_str("<shared-allocation> "),
                MirStorageKind::ArrayBacking => output.push_str("<array-backing> "),
                MirStorageKind::ArrayProduced => output.push_str("<array-produced> "),
                MirStorageKind::ArraySlice => output.push_str("<array-slice> "),
                MirStorageKind::ArrayPosition => output.push_str("<array-position> "),
                MirStorageKind::ArrayAnchor(_) => output.push_str("<array-anchor> "),
                MirStorageKind::ArrayAlias(_) => output.push_str("<array-alias> "),
                _ => unreachable!("verified language storage has a source binding"),
            },
        }
        write_quoted(output, &storage.name);
        let _ = write!(output, " : {}", storage.ty);
        write_span(output, storage.span);
        output.push('\n');
    }
    output.push_str("      Values\n");
    for value in function.values() {
        let _ = write!(output, "        {} : {}", value.id, value.ty);
        write_span(output, value.span);
        output.push('\n');
    }
    if !function.path_conditions().is_empty() {
        output.push_str("      PathConditions\n");
        for condition in function.path_conditions() {
            let _ = write!(output, "        {} parent ", condition.id,);
            match condition.parent {
                Some(parent) => {
                    let _ = write!(output, "{parent}");
                }
                None => output.push_str("<root>"),
            }
            let _ = write!(
                output,
                " activation {} active {} inactive {} merge {}",
                condition.activation,
                condition.active_predecessor,
                condition.inactive_predecessor,
                condition.merge,
            );
            write_span(output, condition.span);
            output.push('\n');
        }
    }
    if !function.logical_expressions().is_empty() {
        output.push_str("      LogicalExpressions\n");
        for logical in function.logical_expressions() {
            let operation = match logical.operation {
                MirLogicalOperation::And => "and",
                MirLogicalOperation::Or => "or",
            };
            let _ = write!(
                output,
                "        {operation} condition {} result {} left {} split {} selection {} right {}..{} value {} short {} join {} selected {}",
                logical.condition,
                logical.result,
                logical.left_result,
                logical.split,
                logical.selection,
                logical.right_entry,
                logical.right_exit,
                logical.right_result,
                logical.short,
                logical.join,
                logical.selected_result,
            );
            write_span(output, logical.span);
            output.push('\n');
        }
    }
    let _ = writeln!(output, "      EntryBlock {}", function.body().entry);
    output.push_str("      Blocks\n");
    for block in &function.body().blocks {
        dump_block(output, block);
    }
}

fn dump_block(output: &mut String, block: &MirBasicBlock) {
    let _ = write!(output, "        {}", block.id);
    write_span(output, block.span);
    output.push('\n');
    for instruction in &block.instructions {
        output.push_str("          ");
        match instruction {
            MirInstruction::StorageLive(lifetime) => {
                let _ = write!(output, "storage-live {}", lifetime.storage);
                write_span(output, lifetime.span);
            }
            MirInstruction::StorageDead(lifetime) => {
                let _ = write!(output, "storage-dead {}", lifetime.storage);
                write_span(output, lifetime.span);
            }
            MirInstruction::Assign(assignment) => {
                let _ = write!(output, "{} = ", assignment.result);
                dump_rvalue(output, &assignment.rvalue);
                write_span(output, assignment.span);
            }
            MirInstruction::Call(call) => {
                if let Some(destination) = &call.destination {
                    dump_place(output, destination);
                    output.push_str(" <- ");
                } else if let Some(result) = call.shared_result {
                    let _ = write!(output, "{result} = shared-result ");
                } else if let Some(result) = call.result {
                    let _ = write!(output, "{result} = ");
                }
                match call.target {
                    MirCallTarget::Direct(target) => {
                        let _ = write!(output, "call {target}");
                    }
                    MirCallTarget::Static(target) => {
                        let _ = write!(output, "call static {target}");
                    }
                    MirCallTarget::Method(MirMethodCallTarget::Direct(target)) => {
                        let _ = write!(output, "call direct {target}");
                    }
                    MirCallTarget::Method(MirMethodCallTarget::Virtual {
                        family,
                        slot,
                        selected,
                    }) => {
                        let _ = write!(
                            output,
                            "call virtual {family} slot {slot} selected {selected}"
                        );
                    }
                    MirCallTarget::Interface(target) => {
                        let _ = write!(
                            output,
                            "call interface {} {}",
                            target.interface, target.requirement
                        );
                    }
                }
                if let Some(receiver) = &call.receiver {
                    output.push_str(" on ");
                    match receiver {
                        MirCallReceiver::Method(receiver) => {
                            dump_place(output, &receiver.place);
                            let _ = write!(output, " {}", receiver.access);
                            if receiver.provenance == MirViewProvenance::Produced {
                                output.push_str(" produced");
                            }
                            output.push_str(" origin ");
                            dump_object_origin(output, &receiver.origin);
                        }
                        MirCallReceiver::Interface(receiver) => {
                            dump_object_view(output, receiver);
                        }
                    }
                }
                output.push('(');
                for (index, argument) in call.arguments.iter().enumerate() {
                    if index != 0 {
                        output.push_str(", ");
                    }
                    dump_argument(output, argument);
                }
                output.push(')');
                write_span(output, call.span);
            }
            MirInstruction::Cleanup(cleanup) => {
                output.push_str("cleanup ");
                dump_place(output, &cleanup.destination);
                let _ = write!(output, " as {}", cleanup.target);
                write_span(output, cleanup.span);
            }
            MirInstruction::Initialize(initialize) => {
                output.push_str("initialize ");
                dump_place(output, &initialize.destination);
                let _ = write!(output, " with {}(", initialize.target);
                for (index, argument) in initialize.arguments.iter().enumerate() {
                    if index != 0 {
                        output.push_str(", ");
                    }
                    dump_argument(output, argument);
                }
                output.push(')');
                write_span(output, initialize.span);
            }
            MirInstruction::Store(store) => {
                output.push_str("store ");
                dump_place(output, &store.destination);
                let _ = write!(output, ", {}", store.value);
                write_span(output, store.span);
            }
            MirInstruction::CopyConstruct(copy) => {
                output.push_str("copy-construct ");
                dump_place(output, &copy.destination);
                output.push_str(" from ");
                dump_place(output, &copy.source);
                let _ = write!(output, " as {} via ", copy.class);
                dump_copy_operation(output, copy.operation);
                write_span(output, copy.span);
            }
            MirInstruction::CopyAssign(copy) => {
                output.push_str("copy-assign ");
                dump_place(output, &copy.destination);
                output.push_str(" from ");
                dump_place(output, &copy.source);
                let _ = write!(output, " as {} via ", copy.class);
                dump_copy_operation(output, copy.operation);
                write_span(output, copy.span);
            }
            MirInstruction::EndFullExpression(end) => {
                output.push_str("end-full-expression");
                for cleanup in &end.temporaries {
                    output.push_str(" cleanup ");
                    dump_place(output, &cleanup.destination);
                    let _ = write!(output, " as {}", cleanup.target);
                }
                write_span(output, end.span);
            }
            MirInstruction::BindCheckedView(binding) => {
                let _ = write!(output, "bind-checked-view {} = ", binding.destination);
                dump_object_view(output, &binding.view);
                write_span(output, binding.span);
            }
            MirInstruction::EndCheckedView(end) => {
                let _ = write!(output, "end-checked-view {}", end.carrier);
                write_span(output, end.span);
            }
            MirInstruction::SharedAllocate(allocation) => {
                let origin = match allocation.origin {
                    MirSharedAllocationOrigin::New => "new",
                    MirSharedAllocationOrigin::OptionalBox => "optional-box",
                    MirSharedAllocationOrigin::Unspecified => "unspecified",
                };
                let _ = write!(
                    output,
                    "shared-allocate {} exact {} from {origin}",
                    allocation.allocation, allocation.target,
                );
                match &allocation.mode {
                    MirSharedAllocationMode::Initialize => output.push_str(" initialize"),
                    MirSharedAllocationMode::Copy { source } => {
                        output.push_str(" copy ");
                        dump_place(output, source);
                    }
                    MirSharedAllocationMode::OptionalBox { completion } => {
                        let _ = write!(output, " complete-with {completion:?}");
                    }
                }
                write_span(output, allocation.span);
            }
            MirInstruction::SharedInitialize(initialize) => {
                let _ = write!(
                    output,
                    "shared-initialize {} with {}(",
                    initialize.allocation, initialize.target
                );
                for (index, argument) in initialize.arguments.iter().enumerate() {
                    if index != 0 {
                        output.push_str(", ");
                    }
                    dump_argument(output, argument);
                }
                output.push(')');
                write_span(output, initialize.span);
            }
            MirInstruction::SharedPublish(publish) => {
                let _ = write!(output, "shared-publish {}", publish.allocation);
                write_span(output, publish.span);
            }
            MirInstruction::SharedStatic(static_owner) => {
                let _ = write!(
                    output,
                    "shared-static {} from {} : {} {:?}",
                    static_owner.destination,
                    static_owner.data,
                    static_owner.target,
                    static_owner.origin
                );
                write_span(output, static_owner.span);
            }
            MirInstruction::SharedAdopt(adopt) => {
                let _ = write!(
                    output,
                    "shared-adopt {} from {}",
                    adopt.destination, adopt.allocation
                );
                write_span(output, adopt.span);
            }
            MirInstruction::SharedCopy(copy) => {
                let _ = write!(
                    output,
                    "shared-copy {} from {}",
                    copy.destination, copy.source
                );
                write_span(output, copy.span);
            }
            MirInstruction::SharedFieldCopy(copy) => {
                let _ = write!(output, "shared-field-copy {} from ", copy.destination);
                dump_place(output, &copy.source);
                write_span(output, copy.span);
            }
            MirInstruction::SharedCast(cast) => {
                output.push_str("shared-cast-static ");
                dump_shared_cast(output, cast);
                write_span(output, cast.span);
            }
            MirInstruction::SharedMove(transfer) => {
                let _ = write!(
                    output,
                    "shared-move {} from {}",
                    transfer.destination, transfer.source
                );
                write_span(output, transfer.span);
            }
            MirInstruction::SharedRelease(release) => {
                let _ = write!(output, "shared-release {}", release.owner);
                write_span(output, release.span);
            }
            MirInstruction::SharedFieldInitialize(initialize) => {
                output.push_str("shared-field-initialize ");
                dump_place(output, &initialize.destination);
                let _ = write!(output, " from {}", initialize.source);
                write_span(output, initialize.span);
            }
            MirInstruction::SharedFieldReplace(replace) => {
                output.push_str("shared-field-replace ");
                dump_place(output, &replace.destination);
                let _ = write!(output, " from {}", replace.source);
                write_span(output, replace.span);
            }
            MirInstruction::StringInitialize(initialize) => {
                output.push_str("string-initialize ");
                dump_place(output, &initialize.destination);
                let _ = write!(
                    output,
                    " from {} backing {} : class {} fields [{}, {}, {}] start {} length {}",
                    initialize.data,
                    initialize.backing,
                    initialize.class,
                    initialize.storage_field,
                    initialize.start_field,
                    initialize.length_field,
                    initialize.start,
                    initialize.length
                );
                write_span(output, initialize.span);
            }
            MirInstruction::OptionalInitialize(initialize) => {
                output.push_str("optional-initialize ");
                dump_place(output, &initialize.destination);
                output.push_str(" from ");
                dump_optional_source(output, &initialize.source);
                write_span(output, initialize.span);
            }
            MirInstruction::OptionalAssign(assignment) => {
                output.push_str("optional-assign ");
                dump_place(output, &assignment.destination);
                output.push_str(" from ");
                dump_optional_source(output, &assignment.source);
                write_span(output, assignment.span);
            }
            MirInstruction::AggregateOptionalInitialize(initialize) => {
                let _ = write!(
                    output,
                    "aggregate-optional-initialize {} ",
                    initialize.optional
                );
                dump_place(output, &initialize.destination);
                output.push_str(" from ");
                dump_aggregate_optional_source(output, &initialize.source);
                write_span(output, initialize.span);
            }
            MirInstruction::AggregateOptionalAssign(assignment) => {
                let _ = write!(output, "aggregate-optional-assign {} ", assignment.optional);
                dump_place(output, &assignment.destination);
                output.push_str(" from ");
                dump_aggregate_optional_source(output, &assignment.source);
                write_span(output, assignment.span);
            }
            MirInstruction::AggregateOptionalPublish(publish) => {
                let _ = write!(output, "aggregate-optional-publish {} ", publish.optional);
                dump_place(output, &publish.destination);
                write_span(output, publish.span);
            }
            MirInstruction::AggregateOptionalCleanup(cleanup) => {
                let _ = write!(output, "aggregate-optional-cleanup {} ", cleanup.optional);
                dump_place(output, &cleanup.destination);
                write_span(output, cleanup.span);
            }
            MirInstruction::OptionalSharedInitialize(initialize) => {
                let _ = write!(
                    output,
                    "optional-shared-initialize {} ",
                    initialize.optional
                );
                dump_place(output, &initialize.destination);
                output.push_str(" from ");
                dump_optional_shared_source(output, &initialize.source);
                write_span(output, initialize.span);
            }
            MirInstruction::OptionalSharedAssign(assignment) => {
                let _ = write!(output, "optional-shared-assign {} ", assignment.optional);
                dump_place(output, &assignment.destination);
                output.push_str(" from ");
                dump_optional_shared_source(output, &assignment.source);
                write_span(output, assignment.span);
            }
            MirInstruction::OptionalSharedCleanup(cleanup) => {
                let _ = write!(output, "optional-shared-cleanup {} ", cleanup.optional);
                dump_place(output, &cleanup.destination);
                write_span(output, cleanup.span);
            }
            MirInstruction::ClassOptionalInitialize(initialize) => {
                let _ = write!(output, "class-optional-initialize {} ", initialize.optional);
                dump_place(output, &initialize.destination);
                let _ = write!(output, " : class {}?", initialize.class);
                write_span(output, initialize.span);
            }
            MirInstruction::ClassOptionalAssign(assignment) => {
                let _ = write!(output, "class-optional-assign {} ", assignment.optional);
                dump_place(output, &assignment.destination);
                let _ = write!(output, " : class {}?", assignment.class);
                write_span(output, assignment.span);
            }
            MirInstruction::ClassOptionalPublish(publish) => {
                let _ = write!(output, "class-optional-publish {} ", publish.optional);
                dump_place(output, &publish.destination);
                write_span(output, publish.span);
            }
            MirInstruction::ClassOptionalCleanup(cleanup) => {
                let _ = write!(output, "class-optional-cleanup {} ", cleanup.optional);
                dump_place(output, &cleanup.destination);
                write_span(output, cleanup.span);
            }
            MirInstruction::EndOptionalView(end) => {
                let _ = write!(
                    output,
                    "end-optional-view {} optional {} ",
                    end.guard, end.optional
                );
                dump_place(output, &end.source);
                let _ = write!(output, " : payload {:?}", end.payload);
                write_span(output, end.span);
            }
            MirInstruction::EndOptionalBoxView(end) => {
                let _ = write!(
                    output,
                    "end-optional-box-view {} {} layer {} owner {}",
                    end.guard, end.box_target, end.layer, end.owner
                );
                write_span(output, end.span);
            }
            MirInstruction::Array(instruction) => dump_array_instruction(output, instruction),
            MirInstruction::Io(instruction) => dump_io_instruction(output, instruction),
        }
        output.push('\n');
    }
    output.push_str("          ");
    match &block.terminator {
        Some(MirTerminator::Return { value, span }) => {
            output.push_str("return");
            if let Some(value) = value {
                let _ = write!(output, " {value}");
            }
            write_span(output, *span);
        }
        Some(MirTerminator::ReturnShared { owner, span }) => {
            let _ = write!(output, "return-shared {owner}");
            write_span(output, *span);
        }
        Some(MirTerminator::ReturnOptionalShared { owner, span }) => {
            let _ = write!(output, "return-optional-shared {owner}");
            write_span(output, *span);
        }
        Some(MirTerminator::Goto { target, span }) => {
            let _ = write!(output, "goto {target}");
            write_span(output, *span);
        }
        Some(MirTerminator::Branch {
            condition,
            true_target,
            false_target,
            span,
        }) => {
            let _ = write!(
                output,
                "branch {condition}, true {true_target}, false {false_target}"
            );
            write_span(output, *span);
        }
        Some(MirTerminator::ShiftCountCheck {
            check,
            success_target,
            failure_target,
            span,
        }) => {
            let _ = write!(
                output,
                "shift-count-check {}.{} left {} count {} result {} width {} -> {success_target} else {failure_target}",
                check.operation.mnemonic(),
                check.operation.left.name(),
                check.left,
                check.count,
                check.result,
                check.operation.width(),
            );
            write_span(output, *span);
        }
        Some(MirTerminator::IntegerDivisorCheck {
            check,
            success_target,
            failure_target,
            span,
        }) => {
            let _ = write!(
                output,
                "integer-divisor-check {}.{} dividend={} divisor={} result={} -> {success_target} else {failure_target}",
                check.operation.mnemonic(),
                check.operation.operand.name(),
                check.dividend,
                check.divisor,
                check.result,
            );
            write_span(output, *span);
        }
        Some(MirTerminator::PrimitiveCastRangeCheck {
            check,
            success_target,
            failure_target,
            span,
        }) => {
            let _ = write!(
                output,
                "primitive-cast-range-check f64.{} source={} result={} finite trunc=toward-zero -> {success_target} else {failure_target}",
                check.relation.target.name(),
                check.source,
                check.result,
            );
            write_span(output, *span);
        }
        Some(MirTerminator::CheckedCast {
            binding,
            success_target,
            failure_target,
            span,
        }) => {
            let _ = write!(output, "checked-cast {} = ", binding.destination);
            dump_object_view(output, &binding.view);
            let _ = write!(
                output,
                ", success {success_target}, failure {failure_target}"
            );
            write_span(output, *span);
        }
        Some(MirTerminator::SharedCast {
            cast,
            success_target,
            failure_target,
            span,
        }) => {
            output.push_str("shared-cast-runtime ");
            dump_shared_cast(output, cast);
            let _ = write!(
                output,
                ", success {success_target}, failure {failure_target}"
            );
            write_span(output, *span);
        }
        Some(MirTerminator::OptionalUnwrap {
            source,
            destination,
            success_target,
            failure_target,
            span,
        }) => {
            output.push_str("optional-unwrap ");
            dump_place(output, source);
            let _ = write!(
                output,
                " into {destination}, success {success_target}, failure {failure_target}"
            );
            write_span(output, *span);
        }
        Some(MirTerminator::OptionalSharedUnwrap {
            unwrap,
            success_target,
            failure_target,
            span,
        }) => {
            let _ = write!(output, "optional-shared-unwrap {} ", unwrap.optional);
            dump_place(output, &unwrap.source);
            let _ = write!(
                output,
                " into {}, success {success_target}, failure {failure_target}",
                unwrap.destination
            );
            write_span(output, *span);
        }
        Some(MirTerminator::BeginOptionalView {
            begin,
            success_target,
            absent_target,
            overflow_target,
            span,
        }) => {
            let _ = write!(
                output,
                "begin-optional-view {} optional {} ",
                begin.guard, begin.optional
            );
            dump_place(output, &begin.source);
            let _ = write!(
                output,
                " : payload {}, success {success_target}, absent {absent_target}, overflow {overflow_target}",
                format_args!("{:?}", begin.payload)
            );
            write_span(output, *span);
        }
        Some(MirTerminator::BeginOptionalBoxView {
            begin,
            success_target,
            absent_target,
            overflow_target,
            span,
        }) => {
            let _ = write!(
                output,
                "begin-optional-box-view {} {} layer {} owner {}, success {success_target}, absent {absent_target}, overflow {overflow_target}",
                begin.guard, begin.box_target, begin.layer, begin.owner
            );
            write_span(output, *span);
        }
        Some(MirTerminator::CheckOptionalMutation {
            source,
            success_target,
            failure_target,
            span,
        }) => {
            output.push_str("check-optional-mutation ");
            dump_place(output, source);
            let _ = write!(
                output,
                ", success {success_target}, failure {failure_target}"
            );
            write_span(output, *span);
        }
        Some(MirTerminator::ArrayPositionCheck {
            position,
            kind,
            success_target,
            failure_target,
            span,
        }) => {
            let _ = write!(
                output,
                "array-position-check {position} {kind:?} -> {success_target} else {failure_target}"
            );
            write_span(output, *span);
        }
        Some(MirTerminator::ArrayOperationCheck {
            failure,
            success_target,
            failure_target,
            span,
        }) => {
            let _ = write!(
                output,
                "array-operation-check {failure:?} -> {success_target} else {failure_target}"
            );
            write_span(output, *span);
        }
        Some(MirTerminator::ArrayLoop {
            backing,
            index,
            length,
            body_target,
            complete_target,
            span,
        }) => {
            let _ = write!(
                output,
                "array-loop {backing}[{index}] < {length} -> {body_target} else {complete_target}"
            );
            write_span(output, *span);
        }
        Some(MirTerminator::Terminate { reason, span }) => {
            let _ = write!(output, "terminate {}", reason.mnemonic());
            write_span(output, *span);
        }
        Some(MirTerminator::Panic { message, span }) => {
            let _ = write!(output, "panic ");
            dump_place(output, message);
            write_span(output, *span);
        }
        None => output.push_str("<unterminated>"),
    }
    output.push('\n');
}

fn dump_shared_cast(output: &mut String, cast: &MirSharedCast) {
    let transfer = match cast.transfer {
        MirSharedCastTransfer::Copy => "copy",
        MirSharedCastTransfer::Adopt => "adopt",
    };
    let _ = write!(output, "{} = {transfer} ", cast.destination,);
    match &cast.source {
        MirSharedCastSource::Owner {
            storage, target, ..
        } => {
            let _ = write!(output, "{storage}: shared {target}");
        }
        MirSharedCastSource::Field { place, target } => {
            dump_place(output, place);
            let _ = write!(output, ": shared {target}");
        }
    }
    let _ = write!(output, " -> shared {}", cast.target);
}

fn dump_object_origin(output: &mut String, origin: &MirObjectOrigin) {
    match origin {
        MirObjectOrigin::Exact {
            complete,
            dynamic_class,
        } => {
            output.push_str("exact(");
            dump_place(output, complete);
            let _ = write!(output, " : {dynamic_class})");
        }
        MirObjectOrigin::Forwarded {
            carrier,
            static_target,
            access,
            dispatch_limit,
            ..
        } => {
            let _ = write!(output, "forwarded({carrier} : ");
            dump_view_target(output, *static_target);
            let _ = write!(output, " {access}");
            if let Some(limit) = dispatch_limit {
                let _ = write!(output, " limit {limit}");
            }
            output.push(')');
        }
        MirObjectOrigin::Shared {
            owner,
            static_target,
            access,
            exact_dynamic_class,
            ..
        } => {
            let _ = write!(output, "shared({owner} : ");
            dump_view_target(output, *static_target);
            let _ = write!(output, " {access}");
            if let Some(class) = exact_dynamic_class {
                let _ = write!(output, " exact {class}");
            }
            output.push(')');
        }
    }
}

fn dump_view_target(output: &mut String, target: MirViewTarget) {
    match target {
        MirViewTarget::Class(class) => {
            let _ = write!(output, "class {class}");
        }
        MirViewTarget::Interface(interface) => {
            let _ = write!(output, "interface {interface}");
        }
        MirViewTarget::Obj => output.push_str("Obj"),
    }
}

fn dump_rvalue(output: &mut String, rvalue: &MirRvalue) {
    match &rvalue.kind {
        MirRvalueKind::ConstantI64(value) => {
            let _ = write!(output, "const.i64 {value}");
        }
        MirRvalueKind::ConstantU64(value) => {
            let _ = write!(output, "const.u64 {value}");
        }
        MirRvalueKind::ConstantU8(value) => {
            let _ = write!(output, "const.u8 {value}");
        }
        MirRvalueKind::ConstantF64Bits(bits) => {
            let _ = write!(output, "const.f64 0x{bits:016x}");
        }
        MirRvalueKind::ConstantBool(value) => {
            let _ = write!(output, "const.bool {value}");
        }
        MirRvalueKind::PathCondition(condition) => {
            let _ = write!(
                output,
                "path-condition {} from {}",
                condition.condition, condition.activation
            );
        }
        MirRvalueKind::Load(place) => {
            output.push_str("load ");
            dump_place(output, place);
        }
        MirRvalueKind::Unary { operation, operand } => {
            let operation = match operation {
                MirUnaryOperation::NegateI64 => "neg.i64".to_owned(),
                MirUnaryOperation::NegateF64 => "neg.f64".to_owned(),
                MirUnaryOperation::LogicalNotBool => "not.bool".to_owned(),
                MirUnaryOperation::BitwiseComplement(integer) => {
                    format!("not.{}", integer.name())
                }
            };
            let _ = write!(output, "{operation} {operand}");
        }
        MirRvalueKind::Binary {
            operation,
            left,
            right,
        } => {
            let operation = match operation {
                MirBinaryOperation::AddI64 => "add.i64".to_owned(),
                MirBinaryOperation::SubtractI64 => "sub.i64".to_owned(),
                MirBinaryOperation::MultiplyI64 => "mul.i64".to_owned(),
                MirBinaryOperation::AddU64 => "add.u64".to_owned(),
                MirBinaryOperation::SubtractU64 => "sub.u64".to_owned(),
                MirBinaryOperation::MultiplyU64 => "mul.u64".to_owned(),
                MirBinaryOperation::AddU8 => "add.u8".to_owned(),
                MirBinaryOperation::SubtractU8 => "sub.u8".to_owned(),
                MirBinaryOperation::MultiplyU8 => "mul.u8".to_owned(),
                MirBinaryOperation::AddF64 => "add.f64".to_owned(),
                MirBinaryOperation::SubtractF64 => "sub.f64".to_owned(),
                MirBinaryOperation::MultiplyF64 => "mul.f64".to_owned(),
                MirBinaryOperation::DivideF64 => "div.f64".to_owned(),
                MirBinaryOperation::IntegerBitwise { operation, operand } => {
                    format!("{}.{}", operation.mnemonic(), operand.name())
                }
            };
            let _ = write!(output, "{operation} {left}, {right}");
        }
        MirRvalueKind::IntegerDivision {
            operation,
            dividend,
            divisor,
        } => {
            let _ = write!(
                output,
                "{}.{} {dividend}, {divisor}",
                operation.mnemonic(),
                operation.operand.name()
            );
        }
        MirRvalueKind::Shift {
            operation,
            left,
            count,
        } => {
            let _ = write!(
                output,
                "{}.{} {left}, {count}",
                operation.mnemonic(),
                operation.left.name()
            );
        }
        MirRvalueKind::PrimitiveComparison {
            operation,
            left,
            right,
        } => {
            let _ = write!(
                output,
                "{}.{} {left}, {right}",
                operation.predicate.mnemonic(),
                operation.operand.name()
            );
        }
        MirRvalueKind::PrimitiveCast { operation, operand } => {
            let _ = write!(
                output,
                "cast.{}.{} {} {operand}",
                operation.source.name(),
                operation.target.name(),
                operation.kind().mnemonic()
            );
        }
        MirRvalueKind::CheckedF64ToInteger { relation, operand } => {
            let _ = write!(
                output,
                "checked-cast.f64.{} trunc=toward-zero {operand}",
                relation.target.name()
            );
        }
        MirRvalueKind::TypeTest { source, target } => {
            output.push_str("type-test ");
            dump_object_view(output, source);
            output.push_str(" is ");
            dump_view_target(output, *target);
        }
        MirRvalueKind::OptionalPresence { source, kind } => {
            let kind = match kind {
                MirPresenceTestKind::Some => "some",
                MirPresenceTestKind::None => "none",
            };
            let _ = write!(output, "optional-presence {kind} ");
            dump_place(output, source);
        }
        MirRvalueKind::OptionalBoxPresence {
            owner,
            target,
            layer,
            kind,
        } => {
            let kind = match kind {
                MirPresenceTestKind::Some => "some",
                MirPresenceTestKind::None => "none",
            };
            let _ = write!(
                output,
                "optional-box-presence {kind} owner={owner} target={target} layer={layer}"
            );
        }
        MirRvalueKind::ArrayLength { source, array } => {
            output.push_str("array-len ");
            dump_place(output, source);
            let _ = write!(output, " as {array}");
        }
    }
    let _ = write!(output, " : {}", rvalue.ty);
}

fn dump_optional_source(output: &mut String, source: &MirOptionalSource) {
    match source {
        MirOptionalSource::Absent => output.push_str("absent"),
        MirOptionalSource::Present(value) => {
            let _ = write!(output, "present {value}");
        }
        MirOptionalSource::Copy(place) => {
            output.push_str("copy ");
            dump_place(output, place);
        }
    }
}

fn dump_aggregate_optional_source(output: &mut String, source: &MirAggregateOptionalSource) {
    match source {
        MirAggregateOptionalSource::Absent => output.push_str("absent"),
        MirAggregateOptionalSource::Unpublished => output.push_str("unpublished"),
        MirAggregateOptionalSource::Copy(place) => {
            output.push_str("copy ");
            dump_place(output, place);
        }
    }
}

fn dump_optional_shared_source(output: &mut String, source: &MirOptionalSharedSource) {
    match source {
        MirOptionalSharedSource::Absent => output.push_str("absent"),
        MirOptionalSharedSource::Present(owner) => {
            let _ = write!(output, "present {owner}");
        }
        MirOptionalSharedSource::Copy(place) => {
            output.push_str("copy ");
            dump_place(output, place);
        }
        MirOptionalSharedSource::Move(owner) => {
            let _ = write!(output, "move {owner}");
        }
    }
}

fn dump_place(output: &mut String, place: &MirPlace) {
    match place.base {
        MirPlaceBase::StaticField(field) => {
            let _ = write!(output, "static({field})");
        }
        MirPlaceBase::StaticLifecycleDestination(field) => {
            let _ = write!(output, "static_destination({field})");
        }
        MirPlaceBase::Storage(storage) => {
            let _ = write!(output, "{storage}");
        }
        MirPlaceBase::AliasParameter(storage) => {
            let _ = write!(output, "indirect({storage})");
        }
        MirPlaceBase::CheckedView(storage) => {
            let _ = write!(output, "checked({storage})");
        }
        MirPlaceBase::ArrayAlias(storage) => {
            let _ = write!(output, "array-alias({storage})");
        }
        MirPlaceBase::SharedPointee(storage) => {
            let _ = write!(output, "shared-pointee({storage})");
        }
        MirPlaceBase::SharedAllocationPayload(storage) => {
            let _ = write!(output, "shared-allocation-payload({storage})");
        }
        MirPlaceBase::OptionalBoxPayload { owner, target } => {
            let _ = write!(output, "optional-box-payload({owner}, {target})");
        }
    }
    for projection in &place.projections {
        match projection {
            MirPlaceProjection::Base(base) => {
                let _ = write!(output, ".base({base})");
            }
            MirPlaceProjection::Field(field) => {
                let _ = write!(output, ".field({field})");
            }
            MirPlaceProjection::OptionalPayload(class) => {
                let _ = write!(output, ".optional-payload({class})");
            }
            MirPlaceProjection::AggregateOptionalPayload(optional) => {
                let _ = write!(output, ".optional-payload({optional})");
            }
            MirPlaceProjection::CheckedOptionalPayload(optional) => {
                let _ = write!(output, ".checked-optional-payload({optional})");
            }
            MirPlaceProjection::ArrayElement {
                array,
                normalized_index,
            } => {
                let _ = write!(output, "[{normalized_index}] as {array}");
            }
        }
    }
}

fn dump_array_instruction(output: &mut String, instruction: &MirArrayInstruction) {
    match instruction {
        MirArrayInstruction::Allocate {
            backing,
            array,
            length,
            ownership,
            failure,
            span,
        } => {
            let _ = write!(
                output,
                "array-allocate {backing} {array} length {length} {ownership:?} failure {failure:?}"
            );
            write_span(output, *span);
        }
        MirArrayInstruction::AllocateElements {
            backing,
            prefix,
            array,
            length,
            ownership,
            failure,
            span,
        } => {
            let _ = write!(
                output,
                "array-allocate-elements {backing} {array} length {length} prefix {prefix} {ownership:?} failure {failure:?}"
            );
            write_span(output, *span);
        }
        MirArrayInstruction::InitializeElement {
            backing,
            prefix,
            position,
            value,
            span,
        } => {
            let _ = write!(
                output,
                "array-initialize-element {backing}[{prefix}] position {position} = {value}"
            );
            write_span(output, *span);
        }
        MirArrayInstruction::CompleteElement {
            backing,
            prefix,
            position,
            span,
        } => {
            let _ = write!(
                output,
                "array-complete-element {backing}[{prefix}] position {position}"
            );
            write_span(output, *span);
        }
        MirArrayInstruction::InitializeNext {
            backing,
            index,
            operation,
            span,
        } => {
            let _ = write!(
                output,
                "array-initialize-next {backing}[{index}] via {operation:?}"
            );
            write_span(output, *span);
        }
        MirArrayInstruction::CopyNext {
            backing,
            source,
            index,
            operation,
            span,
        } => {
            let _ = write!(output, "array-copy-next {backing}[{index}] from ");
            dump_place(output, source);
            let _ = write!(output, " via {operation:?}");
            write_span(output, *span);
        }
        MirArrayInstruction::Publish {
            backing,
            destination,
            span,
        } => {
            let _ = write!(output, "array-publish {backing} into {destination}");
            write_span(output, *span);
        }
        MirArrayInstruction::Adopt {
            destination,
            source,
            array,
            span,
        }
        | MirArrayInstruction::Replace {
            destination,
            source,
            array,
            span,
        } => {
            let verb = if matches!(instruction, MirArrayInstruction::Adopt { .. }) {
                "array-adopt"
            } else {
                "array-replace"
            };
            output.push_str(verb);
            output.push(' ');
            dump_place(output, destination);
            let _ = write!(output, " from {source} as {array}");
            write_span(output, *span);
        }
        MirArrayInstruction::Offset {
            destination,
            owner,
            offset,
            array,
            span,
        } => {
            let _ = write!(output, "array-range-offset {destination} = {offset} in ");
            dump_place(output, owner);
            let _ = write!(output, " : {array}");
            write_span(output, *span);
        }
        other => {
            let _ = write!(output, "array-op {other:?}");
        }
    }
}

fn dump_io_instruction(output: &mut String, instruction: &MirIoInstruction) {
    let _ = write!(output, "{} = io ", instruction.result);
    match &instruction.operation {
        MirIoOperation::StandardHandle { stream } => {
            let _ = write!(output, "standard-handle stream {stream}");
        }
        MirIoOperation::Open { path, mode } => {
            output.push_str("open path ");
            dump_io_buffer(output, path);
            let _ = write!(output, " mode {mode}");
        }
        MirIoOperation::Read {
            handle,
            destination,
            offset,
        } => {
            let _ = write!(output, "read handle {handle} destination ");
            dump_io_buffer(output, destination);
            let _ = write!(output, " offset {offset}");
        }
        MirIoOperation::Write {
            handle,
            source,
            offset,
        } => {
            let _ = write!(output, "write handle {handle} source ");
            dump_io_buffer(output, source);
            let _ = write!(output, " offset {offset}");
        }
        MirIoOperation::Close { handle } => {
            let _ = write!(output, "close handle {handle}");
        }
    }
    write_span(output, instruction.span);
}

fn dump_io_buffer(output: &mut String, buffer: &MirIoBuffer) {
    dump_place(output, &buffer.place);
    let _ = write!(
        output,
        " : {} {} anchor {}",
        buffer.array, buffer.access, buffer.anchor
    );
}

fn dump_argument(output: &mut String, argument: &MirArgument) {
    match argument {
        MirArgument::Value(value) => {
            let _ = write!(output, "value({value})");
        }
        MirArgument::Place(place) => {
            output.push_str("place(");
            dump_place(output, place);
            output.push(')');
        }
        MirArgument::View(view) => {
            dump_object_view(output, view);
        }
        MirArgument::OwnedPlace(place) => {
            output.push_str("owned(");
            dump_place(output, place);
            output.push(')');
        }
        MirArgument::SharedOwner(owner) => {
            let _ = write!(output, "shared-owner({owner})");
        }
    }
}

fn dump_object_view(output: &mut String, view: &MirObjectView) {
    output.push_str("view(");
    dump_place(output, &view.source);
    output.push_str(" -> ");
    dump_view_target(output, view.target);
    let _ = write!(output, " {}", view.access);
    if view.provenance == MirViewProvenance::Produced {
        output.push_str(" produced");
    }
    output.push_str(" origin ");
    dump_object_origin(output, &view.origin);
    output.push(')');
}
