//! Monotone final-MIR realization verification for baseline authority.

use std::collections::{BTreeMap, BTreeSet};

use crate::mir::{
    MirExecutionNode, MirVerificationError, StaticLifecycleAuthority, StaticLifecycleEffectFact,
};

use super::{
    super::{
        analysis::{extract, root_effects},
        plan::derived,
    },
    program_error, LifecycleMirView,
};

pub(super) fn verify(program: LifecycleMirView<'_>, errors: &mut Vec<MirVerificationError>) {
    let realized = match analyze(program) {
        Ok(realized) => realized,
        Err(error) => {
            program_error(
                errors,
                format!("cannot derive final static-lifecycle realization: {error:?}"),
            );
            return;
        }
    };
    let baseline = program.lifecycle.proof().authority();

    verify_root_coverage_and_subset(baseline, &realized, errors);
    verify_realized_dependencies(program, &realized, errors);
}

pub(super) fn analyze(
    program: LifecycleMirView<'_>,
) -> Result<StaticLifecycleAuthority, root_effects::StaticLifecycleRootEffectError> {
    let extracted = extract::extract_final(program.program, program.initializers);
    root_effects::analyze_final(program.program, program.lifecycle.definitions(), &extracted)
}

fn verify_root_coverage_and_subset(
    baseline: &StaticLifecycleAuthority,
    realized: &StaticLifecycleAuthority,
    errors: &mut Vec<MirVerificationError>,
) {
    let mut baseline_roots = BTreeMap::new();
    for root in baseline.roots() {
        if baseline_roots.insert(root.root(), root).is_some() {
            program_error(
                errors,
                format!(
                    "baseline authority contains duplicate lifecycle root {:?}",
                    root.root()
                ),
            );
        }
    }

    for root in realized.roots() {
        let Some(authorized) = baseline_roots.get(&root.root()).copied() else {
            program_error(
                errors,
                format!(
                    "final MIR lifecycle root {:?} has no baseline authority",
                    root.root()
                ),
            );
            continue;
        };
        let authorized_facts = authorized
            .effects()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        for fact in root.effects() {
            if !authorized_facts.contains(fact) {
                unauthorized_fact(errors, root.root(), *fact);
            }
        }
    }

    for root in baseline_roots.keys() {
        if realized.root(*root).is_none() {
            program_error(
                errors,
                format!("final MIR definitions do not require baseline lifecycle root {root:?}"),
            );
        }
    }
}

fn unauthorized_fact(
    errors: &mut Vec<MirVerificationError>,
    root: MirExecutionNode,
    fact: StaticLifecycleEffectFact,
) {
    program_error(
        errors,
        format!("final MIR lifecycle root {root:?} realizes unauthorized fact {fact:?}"),
    );
}

fn verify_realized_dependencies(
    program: LifecycleMirView<'_>,
    realized: &StaticLifecycleAuthority,
    errors: &mut Vec<MirVerificationError>,
) {
    let dependencies = match root_effects::dependency_pairs_for_definitions(
        program.program,
        program.lifecycle.definitions(),
        realized,
    ) {
        Ok(dependencies) => dependencies,
        Err(error) => {
            program_error(
                errors,
                format!("cannot derive final static-lifecycle dependencies: {error:?}"),
            );
            return;
        }
    };
    let positions = derived::positions(program.lifecycle.plan());

    for (prerequisite, dependent) in dependencies {
        let valid = positions
            .get(&prerequisite)
            .zip(positions.get(&dependent))
            .is_some_and(|(prerequisite, dependent)| {
                prerequisite.activation < dependent.activation
            });
        if !valid {
            program_error(
                errors,
                format!(
                    "final static-lifecycle dependency {prerequisite} -> {dependent} violates activation order"
                ),
            );
        }
    }
}
