//! Deterministic textual rendering of MIR.

mod body;
mod declarations;
mod lifecycle;
mod value;

use std::fmt::Write;

use crate::dump_format::{write_quoted, write_span};

use super::model::*;
use body::dump_executable_body;
use declarations::{
    dump_class, dump_declaration, dump_definition, dump_member_definition, dump_parameters,
};
use lifecycle::{dump_static_lifecycle_coordinator, write_static_field_reference};
use value::dump_view_target;

pub fn dump_mir(program: &MirProgram) -> String {
    dump_program(program, "MirProgram")
}

pub fn dump_preliminary_mir(program: &PreliminaryMirProgram) -> String {
    let mut output = dump_program(program.program(), "PreliminaryMirProgram");
    output.push_str("  StaticInitializationModes\n");
    for field in program.static_fields() {
        output.push_str("    StaticField ");
        write_static_field_reference(&mut output, program.program(), field.field);
        if field.final_span.is_some() {
            output.push_str(" final");
        }
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
    if !program.function_types.is_empty() {
        output.push_str("  FunctionTypes\n");
        for function in program.function_types.iter() {
            let _ = write!(output, "    FunctionType {} (", function.id);
            dump_parameters(&mut output, &function.parameters);
            let _ = write!(output, ") -> {}", function.result);
            write_span(&mut output, function.span);
            output.push('\n');
        }
    }
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
            "  StringLanguageItem class {} storage {} start {} length {} hash-code {} storage-array {}",
            item.class,
            item.storage_field,
            item.start_field,
            item.length_field,
            item.hash_code_field,
            item.storage_array
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
