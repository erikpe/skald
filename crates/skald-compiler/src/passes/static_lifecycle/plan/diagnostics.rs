//! Source-facing diagnostics for invalid static-lifetime constraints.

use crate::{diagnostics::Diagnostic, mir::PreliminaryMirProgram};

use super::{
    graph::LifetimeGraph,
    model::{StaticLifetimeDependency, StaticLifetimePhase},
};

pub const STATIC_LIFECYCLE_SELF_DEPENDENCY: &str = "STA001";
pub const STATIC_LIFECYCLE_DEPENDENCY_CYCLE: &str = "STA002";

pub(crate) fn cycle_diagnostics(
    program: &PreliminaryMirProgram,
    graph: &LifetimeGraph,
    components: &[Vec<usize>],
) -> Vec<Diagnostic> {
    components
        .iter()
        .map(|component| {
            let cycle = graph.representative_cycle(component);
            let edges = cycle
                .windows(2)
                .map(|pair| graph.dependency(pair[0], pair[1]))
                .collect::<Vec<_>>();
            if cycle.len() == 2 && cycle[0] == cycle[1] {
                self_dependency_diagnostic(program, edges[0])
            } else {
                dependency_cycle_diagnostic(program, &edges)
            }
        })
        .collect()
}

fn self_dependency_diagnostic(
    program: &PreliminaryMirProgram,
    dependency: &StaticLifetimeDependency,
) -> Diagnostic {
    let evidence = &dependency.evidence;
    let field = field_name(program, dependency.dependent);
    let phase = phase_name(evidence.phase);
    let article = match evidence.phase {
        StaticLifetimePhase::Initialization => "an",
        StaticLifetimePhase::Destruction => "a",
    };
    let mut diagnostic = Diagnostic::error(
        STATIC_LIFECYCLE_SELF_DEPENDENCY,
        format!("static field `{field}` has {article} {phase} self-dependency"),
    )
    .with_primary_label(
        evidence.root_span,
        format!("the {phase} lifetime of `{field}` starts here"),
    );
    diagnostic = add_evidence_labels(program, diagnostic, dependency);
    diagnostic.with_note(match evidence.phase {
        StaticLifetimePhase::Initialization => {
            format!("`{field}` is accessed before its value has been published as live")
        }
        StaticLifetimePhase::Destruction => {
            format!("`{field}` is accessed after its state has changed to destroying")
        }
    })
}

fn dependency_cycle_diagnostic(
    program: &PreliminaryMirProgram,
    dependencies: &[&StaticLifetimeDependency],
) -> Diagnostic {
    let first = dependencies[0];
    let mut diagnostic = Diagnostic::error(
        STATIC_LIFECYCLE_DEPENDENCY_CYCLE,
        "static field lifetimes contain a dependency cycle",
    )
    .with_primary_label(
        first.evidence.root_span,
        format!(
            "the lifetime of `{}` participates in this cycle",
            field_name(program, first.dependent)
        ),
    );
    for dependency in dependencies {
        diagnostic = add_evidence_labels(program, diagnostic, dependency);
    }

    let mut cycle = dependencies
        .iter()
        .map(|dependency| field_name(program, dependency.prerequisite))
        .collect::<Vec<_>>();
    cycle.push(field_name(program, dependencies[0].prerequisite));
    diagnostic.with_note(format!(
        "required activation order closes a cycle: {}",
        cycle.join(" -> ")
    ))
}

fn add_evidence_labels(
    program: &PreliminaryMirProgram,
    mut diagnostic: Diagnostic,
    dependency: &StaticLifetimeDependency,
) -> Diagnostic {
    let evidence = &dependency.evidence;
    let root = field_name(program, dependency.dependent);
    let target = field_name(program, dependency.prerequisite);
    diagnostic = diagnostic.with_secondary_label(
        evidence.root_span,
        format!(
            "{} of `{root}` requires `{target}` to remain live",
            phase_name(evidence.phase)
        ),
    );
    diagnostic = diagnostic.with_secondary_label(
        evidence.target_span,
        format!("dependency target `{target}` is declared here"),
    );
    for edge in &evidence.witness {
        diagnostic = diagnostic.with_secondary_label(
            edge.span,
            format!("dependency continues through {:?}", edge.kind),
        );
    }
    diagnostic.with_secondary_label(
        evidence.access_span,
        format!("`{target}` is accessed here as {:?}", evidence.access),
    )
}

fn phase_name(phase: StaticLifetimePhase) -> &'static str {
    match phase {
        StaticLifetimePhase::Initialization => "initialization",
        StaticLifetimePhase::Destruction => "destruction",
    }
}

fn field_name(program: &PreliminaryMirProgram, field: crate::identity::StaticFieldId) -> String {
    program
        .static_field_qualified_name(field)
        .expect("lifetime field declaration must exist")
}
