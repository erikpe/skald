//! Stable textual rendering for static-effect analysis.

use std::fmt::Write;

use crate::dump_format::write_span;

use super::model::{StaticEffectAnalysis, StaticEffectNode};

pub fn dump_static_effects(analysis: &StaticEffectAnalysis) -> String {
    let mut output = String::from("StaticEffectAnalysis\n");
    let _ = writeln!(
        output,
        "  RecursiveComponents {}",
        analysis.recursive_components()
    );
    for summary in analysis.summaries() {
        let _ = write!(output, "  Node ");
        write_node(&mut output, summary.node);
        output.push('\n');
        for direct in &summary.direct_effects {
            let _ = write!(
                output,
                "    Direct {} {:?} {:?}",
                direct.field, direct.access, direct.phase
            );
            if direct.lifecycle_owned {
                output.push_str(" lifecycle-destination");
            }
            write_span(&mut output, direct.span);
            output.push('\n');
        }
        for edge in &summary.possible_targets {
            output.push_str("    Target ");
            write_node(&mut output, edge.source);
            output.push_str(" -> ");
            write_node(&mut output, edge.target);
            let _ = write!(output, " {:?} {:?}", edge.kind, edge.phase);
            write_span(&mut output, edge.span);
            output.push('\n');
        }
        for effect in &summary.effects {
            let _ = write!(
                output,
                "    Effect {} {:?} {:?}",
                effect.field, effect.access, effect.phase
            );
            if effect.lifecycle_owned {
                output.push_str(" lifecycle-destination");
            }
            write_span(&mut output, effect.span);
            output.push('\n');
            for edge in &effect.witness {
                output.push_str("      via ");
                write_node(&mut output, edge.source);
                output.push_str(" -> ");
                write_node(&mut output, edge.target);
                let _ = write!(output, " {:?} {:?}", edge.kind, edge.phase);
                write_span(&mut output, edge.span);
                output.push('\n');
            }
        }
    }
    output
}

pub(crate) fn write_node(output: &mut String, node: StaticEffectNode) {
    match node {
        StaticEffectNode::Callable(callable) => {
            let _ = write!(output, "callable {callable}");
        }
        StaticEffectNode::ClassLifecycle { class, operation } => {
            let _ = write!(output, "class {class} {operation:?}");
        }
        StaticEffectNode::ArrayLifecycle { array, operation } => {
            let _ = write!(output, "array {array} {operation:?}");
        }
    }
}
