//! Rendering for static initialization and destruction coordination.

use std::fmt::Write;

use crate::dump_format::{write_quoted, write_span};

use super::super::model::*;
use super::body::dump_executable_body;
use super::value::dump_place;

pub(super) fn dump_static_lifecycle_coordinator(
    output: &mut String,
    program: &MirProgram,
    coordinator: &MirStaticLifecycleCoordinator,
) {
    output.push_str("  StaticLifecycleCoordinator\n");
    let lifecycle = coordinator.lifecycle();
    let _ = writeln!(
        output,
        "    Proof authority-roots={} active-fields={}",
        lifecycle.proof().authority().roots().len(),
        lifecycle.proof().activation().len(),
    );
    output.push_str("    Definitions\n");
    let positions = lifecycle
        .plan()
        .activation()
        .iter()
        .copied()
        .enumerate()
        .map(|(activation, field)| {
            (
                field,
                (
                    activation,
                    lifecycle.plan().activation().len() - activation - 1,
                ),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    for definition in lifecycle.definitions() {
        output.push_str("      Field ");
        write_static_field_reference(output, program, definition.field);
        if definition.final_span.is_some() {
            output.push_str(" final");
        }
        let (activation, shutdown) = positions[&definition.field];
        let _ = writeln!(
            output,
            " : {} {} activation={} shutdown={}",
            definition.ty, definition.initialization, activation, shutdown
        );
    }
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

pub(super) fn write_static_field_reference(
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
