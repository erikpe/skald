//! Stable focused rendering of the semantic activation decision.

use std::fmt::Write;

use crate::{
    dump_format::write_span,
    mir::{MirExecutionNode, PreliminaryMirProgram},
};

use super::{
    StaticActivationAnalysis, StaticActivationEdge, StaticActivationNode, StaticActivationWitness,
};

pub(crate) fn dump_static_activation(
    program: &PreliminaryMirProgram,
    analysis: &StaticActivationAnalysis,
) -> String {
    let counts = analysis.counts();
    let mut output = String::from("StaticActivationAnalysis\n");
    let _ = writeln!(
        output,
        "  Summary declared={} active={} inactive={} execution={} edges={} accesses={} dependencies={} initializers={} destructions={}",
        counts.declared_fields,
        counts.active_fields,
        counts.inactive_fields,
        counts.reachable_execution_nodes,
        counts.edges,
        counts.static_accesses,
        counts.execution_dependencies,
        counts.initializer_roots,
        counts.destruction_roots,
    );
    if !analysis.target_counts().is_empty() {
        output.push_str("  ConservativeTargets\n");
        for count in analysis.target_counts() {
            let _ = writeln!(output, "    {:?} {}", count.kind(), count.targets());
        }
    }
    output.push_str("  ActiveFields\n");
    for active in analysis.active_fields() {
        output.push_str("    Field ");
        write_field(&mut output, program, active.field());
        output.push('\n');
        write_witness(&mut output, program, active.witness());
        write_outgoing(
            &mut output,
            program,
            analysis,
            StaticActivationNode::field(active.field()),
        );
    }
    output.push_str("  InactiveFields\n");
    for field in analysis.inactive_fields() {
        output.push_str("    Field ");
        write_field(&mut output, program, *field);
        output.push('\n');
    }
    output.push_str("  ReachableExecution\n");
    for execution in analysis.reachable_execution() {
        output.push_str("    Node ");
        write_execution(&mut output, execution.node());
        output.push('\n');
        write_witness(&mut output, program, execution.witness());
        write_outgoing(
            &mut output,
            program,
            analysis,
            StaticActivationNode::execution(execution.node()),
        );
    }
    output
}

fn write_witness(
    output: &mut String,
    program: &PreliminaryMirProgram,
    witness: &StaticActivationWitness,
) {
    output.push_str("      Root ");
    write_execution(output, witness.root().entry());
    write_span(output, witness.root().span());
    output.push('\n');
    for edge in witness.edges() {
        output.push_str("      Via ");
        write_edge(output, program, edge);
        output.push('\n');
    }
}

fn write_outgoing(
    output: &mut String,
    program: &PreliminaryMirProgram,
    analysis: &StaticActivationAnalysis,
    source: StaticActivationNode,
) {
    for edge in analysis.outgoing_dependencies(source) {
        output.push_str("      Target ");
        write_edge(output, program, edge);
        output.push('\n');
    }
}

fn write_edge(output: &mut String, program: &PreliminaryMirProgram, edge: &StaticActivationEdge) {
    write_node(output, program, edge.source());
    output.push_str(" -> ");
    write_node(output, program, edge.target());
    let _ = write!(output, " {:?}", edge.trigger());
    write_span(output, edge.span());
}

fn write_node(output: &mut String, program: &PreliminaryMirProgram, node: StaticActivationNode) {
    match node {
        StaticActivationNode::Execution(node) => write_execution(output, node),
        StaticActivationNode::Field(field) => write_field(output, program, field),
    }
}

fn write_field(
    output: &mut String,
    program: &PreliminaryMirProgram,
    field: crate::identity::StaticFieldId,
) {
    let _ = write!(output, "{field}");
    if let Some(name) = program.static_field_qualified_name(field) {
        let _ = write!(output, " ({name})");
    }
}

fn write_execution(output: &mut String, node: MirExecutionNode) {
    match node {
        MirExecutionNode::Callable(callable) => {
            let _ = write!(output, "callable {callable}");
        }
        MirExecutionNode::ClassLifecycle { class, operation } => {
            let _ = write!(output, "class {class} {operation:?}");
        }
        MirExecutionNode::ArrayLifecycle { array, operation } => {
            let _ = write!(output, "array {array} {operation:?}");
        }
    }
}
