//! Stable textual rendering for planned static lifetimes.

use std::fmt::Write;

use crate::{
    dump_format::{write_quoted, write_span},
    identity::StaticFieldId,
    mir::dump_preliminary_mir,
};

use super::{
    super::{dump::write_node, dump_static_effects},
    model::{PlannedMirProgram, StaticLifetimeDependency},
};

pub fn dump_planned_mir(program: &PlannedMirProgram) -> String {
    format!(
        "{}{}{}",
        dump_preliminary_mir(program.preliminary()),
        dump_static_effects(program.effects()),
        dump_static_lifetime_plan(program),
    )
}

pub fn dump_static_lifetime_plan(program: &PlannedMirProgram) -> String {
    let mut output = String::from("StaticLifetimePlan\n");
    for dependency in program.dependencies() {
        write_dependency(&mut output, program, dependency);
    }
    output.push_str("  Activation");
    for field in program.lifecycle().activation() {
        output.push(' ');
        write_field_reference(&mut output, program, *field);
    }
    output.push('\n');
    output.push_str("  Shutdown");
    for field in program.lifecycle().shutdown() {
        output.push(' ');
        write_field_reference(&mut output, program, *field);
    }
    output.push('\n');
    output.push_str("ProgramLifecycle\n");
    for definition in program.lifecycle_mir().definitions() {
        output.push_str("  Field ");
        write_field_reference(&mut output, program, definition.field);
        let _ = write!(
            output,
            " {} {} activation={} shutdown={}",
            definition.ty,
            definition.initialization,
            definition.indices.activation,
            definition.indices.shutdown
        );
        write_span(&mut output, definition.span);
        output.push('\n');
    }
    output.push_str("  ActivationTransitions\n");
    for transition in program.lifecycle_mir().activation() {
        output.push_str("    ");
        write_field_reference(&mut output, program, transition.field);
        let _ = write!(output, " {:?}", transition.kind);
        write_span(&mut output, transition.span);
        output.push('\n');
    }
    output.push_str("  ShutdownTransitions\n");
    for transition in program.lifecycle_mir().shutdown() {
        output.push_str("    ");
        write_field_reference(&mut output, program, transition.field);
        let _ = write!(output, " {:?}", transition.kind);
        write_span(&mut output, transition.span);
        output.push('\n');
    }
    output
}

fn write_dependency(
    output: &mut String,
    program: &PlannedMirProgram,
    dependency: &StaticLifetimeDependency,
) {
    let evidence = &dependency.evidence;
    output.push_str("  Dependency ");
    write_field_reference(output, program, dependency.prerequisite);
    output.push_str(" -> ");
    write_field_reference(output, program, dependency.dependent);
    let _ = write!(output, " {:?} root ", evidence.phase);
    write_node(output, evidence.root_effect);
    write_span(output, evidence.root_span);
    output.push('\n');
    output.push_str("    Access ");
    write_field_reference(output, program, evidence.target);
    let _ = write!(output, " {:?} {:?}", evidence.access, evidence.effect_phase);
    write_span(output, evidence.access_span);
    output.push('\n');
    output.push_str("    TargetDeclaration ");
    write_field_reference(output, program, evidence.target);
    write_span(output, evidence.target_span);
    output.push('\n');
    for edge in &evidence.witness {
        output.push_str("    via ");
        write_node(output, edge.source);
        output.push_str(" -> ");
        write_node(output, edge.target);
        let _ = write!(output, " {:?} {:?}", edge.kind, edge.phase);
        write_span(output, edge.span);
        output.push('\n');
    }
}

fn write_field_reference(output: &mut String, program: &PlannedMirProgram, field: StaticFieldId) {
    let _ = write!(output, "{field}");
    if let Some(name) = program.preliminary().static_field_qualified_name(field) {
        output.push(' ');
        write_quoted(output, &name);
    }
}
