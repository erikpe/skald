//! Stable target-independent reachability rendering for tests and tools.

use std::fmt::Write;

use crate::{dump_format::write_span, mir::MirExecutionNode};

use super::{
    MirDependencyEdge, MirDependencyTarget, MirReachabilityAnalysis, MirReachabilityRootTarget,
};

pub(crate) fn dump_reachability(analysis: &MirReachabilityAnalysis) -> String {
    let counts = analysis.counts();
    let mut output = String::from("MirReachabilityAnalysis\n");
    let _ = writeln!(
        output,
        "  Summary roots={} nodes={} callables={} retained={} dependencies={} runtime={} virtual-families={} interface-requirements={} function-signatures={} function-targets={}",
        counts.roots,
        counts.reachable_nodes,
        counts.reachable_callables,
        counts.retained_definitions,
        counts.dependencies,
        counts.runtime_entities,
        counts.virtual_families,
        counts.interface_requirements,
        counts.function_value_signatures,
        counts.function_value_targets,
    );
    output.push_str("  Roots\n");
    for root in analysis.roots() {
        let _ = write!(output, "    {:?} ", root.reason());
        write_root_target(&mut output, root.target());
        write_span(&mut output, root.span());
        output.push('\n');
    }
    if !analysis.function_value_candidates().is_empty() {
        output.push_str("  FunctionValues\n");
        for candidates in analysis.function_value_candidates() {
            let _ = writeln!(output, "    Type {}", candidates.function_type());
            for target in candidates.targets() {
                let _ = write!(output, "      Target {} formed-in ", target.callable());
                write_node(&mut output, target.source());
                write_span(&mut output, target.first_formation_span());
                output.push('\n');
            }
        }
    }
    if !analysis.runtime_entities().is_empty() {
        output.push_str("  RuntimeEntities\n");
        for entity in analysis.runtime_entities() {
            let _ = writeln!(output, "    {entity:?}");
        }
    }
    output.push_str("  ReachableNodes\n");
    for node in analysis.reachable_nodes() {
        output.push_str("    Node ");
        write_node(&mut output, *node);
        output.push('\n');
        if let Some(explanation) = analysis.explanation(*node) {
            let _ = write!(output, "      Root {:?} ", explanation.root().reason());
            write_root_target(&mut output, explanation.root().target());
            write_span(&mut output, explanation.root().span());
            output.push('\n');
            for dependency in explanation.dependencies() {
                output.push_str("      Via ");
                write_dependency(&mut output, dependency);
                output.push('\n');
            }
        }
        for dependency in analysis.outgoing_dependencies(*node) {
            output.push_str("      Target ");
            write_dependency(&mut output, dependency);
            output.push('\n');
        }
    }
    output
}

fn write_dependency(output: &mut String, dependency: &MirDependencyEdge) {
    write_node(output, dependency.source());
    output.push_str(" -> ");
    write_dependency_target(output, dependency.target());
    let _ = write!(output, " {:?}", dependency.kind());
    write_span(output, dependency.span());
}

fn write_root_target(output: &mut String, target: MirReachabilityRootTarget) {
    match target {
        MirReachabilityRootTarget::Execution(node) => write_node(output, node),
        MirReachabilityRootTarget::RuntimeEntity(entity) => {
            let _ = write!(output, "runtime {entity:?}");
        }
    }
}

fn write_dependency_target(output: &mut String, target: MirDependencyTarget) {
    match target {
        MirDependencyTarget::Execution(node) => write_node(output, node),
        MirDependencyTarget::RuntimeEntity(entity) => {
            let _ = write!(output, "runtime {entity:?}");
        }
        MirDependencyTarget::External(link) => {
            let _ = write!(output, "external {link}");
        }
        MirDependencyTarget::Intrinsic(intrinsic) => {
            let _ = write!(output, "intrinsic {intrinsic:?}");
        }
    }
}

fn write_node(output: &mut String, node: MirExecutionNode) {
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
