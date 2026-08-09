//! Stable textual rendering for planned static lifetimes.

use std::fmt::Write;

use crate::{dump_format::write_span, mir::dump_preliminary_mir};

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
        write_dependency(&mut output, dependency);
    }
    output.push_str("  Activation");
    for field in program.lifecycle().activation() {
        let _ = write!(output, " {field}");
    }
    output.push('\n');
    output.push_str("  Shutdown");
    for field in program.lifecycle().shutdown() {
        let _ = write!(output, " {field}");
    }
    output.push('\n');
    output
}

fn write_dependency(output: &mut String, dependency: &StaticLifetimeDependency) {
    let evidence = &dependency.evidence;
    let _ = write!(
        output,
        "  Dependency {} -> {} {:?} root ",
        dependency.prerequisite, dependency.dependent, evidence.phase
    );
    write_node(output, evidence.root_effect);
    write_span(output, evidence.root_span);
    output.push('\n');
    let _ = write!(
        output,
        "    Access {} {:?} {:?}",
        evidence.target, evidence.access, evidence.effect_phase
    );
    write_span(output, evidence.access_span);
    output.push('\n');
    let _ = write!(output, "    TargetDeclaration {}", evidence.target);
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
