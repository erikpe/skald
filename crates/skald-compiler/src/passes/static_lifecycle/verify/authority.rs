//! Exact issuance verification for planner-owned baseline authority.

use std::collections::BTreeSet;

use crate::mir::{MirVerificationError, StaticLifecycleAuthority, StaticLifecycleRootAuthority};

use super::{
    super::{
        analysis::{extract, root_effects},
        plan::{derived, PlannedMirProgram},
    },
    program_error,
};

pub(super) fn verify(program: &PlannedMirProgram, errors: &mut Vec<MirVerificationError>) {
    let extracted = extract::extract(program.preliminary());
    let expected = match root_effects::analyze_for_fields(
        program.preliminary(),
        &extracted,
        program.activation_authority().fields(),
    ) {
        Ok(authority) => authority,
        Err(error) => {
            program_error(
                errors,
                format!("cannot recompute static-lifecycle baseline authority: {error:?}"),
            );
            return;
        }
    };
    let authority = program.lifecycle_mir().proof().authority();

    verify_canonical_authority(program, authority, &expected, &extracted, errors);
    verify_dependency_order(program, authority, errors);
}

fn verify_canonical_authority(
    program: &PlannedMirProgram,
    authority: &StaticLifecycleAuthority,
    expected: &StaticLifecycleAuthority,
    extracted: &extract::ExtractedGraph,
    errors: &mut Vec<MirVerificationError>,
) {
    let roots = authority.roots().collect::<Vec<_>>();
    for pair in roots.windows(2) {
        match pair[0].root().cmp(&pair[1].root()) {
            std::cmp::Ordering::Equal => program_error(
                errors,
                "baseline authority contains a duplicate lifecycle root",
            ),
            std::cmp::Ordering::Greater => program_error(
                errors,
                "baseline authority lifecycle roots are not in canonical order",
            ),
            std::cmp::Ordering::Less => {}
        }
    }

    let declared_fields = program
        .static_fields()
        .map(|field| field.field)
        .collect::<BTreeSet<_>>();
    for root in &roots {
        verify_root_shape(root, &declared_fields, errors);
        if !extracted.nodes.contains_key(&root.root()) {
            program_error(
                errors,
                format!(
                    "baseline authority names foreign lifecycle root {:?}",
                    root.root()
                ),
            );
        } else if expected.root(root.root()).is_none() {
            program_error(
                errors,
                format!(
                    "baseline authority contains extra lifecycle root {:?}",
                    root.root()
                ),
            );
        }
    }

    for expected_root in expected.roots() {
        let Some(actual_root) = authority.root(expected_root.root()) else {
            program_error(
                errors,
                format!(
                    "baseline authority omits lifecycle root {:?}",
                    expected_root.root()
                ),
            );
            continue;
        };
        let expected_facts = expected_root
            .effects()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let actual_facts = actual_root
            .effects()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if let Some(fact) = expected_facts.difference(&actual_facts).next() {
            program_error(
                errors,
                format!(
                    "baseline authority for {:?} omits preliminary-MIR fact {fact:?}",
                    expected_root.root()
                ),
            );
        }
        if let Some(fact) = actual_facts.difference(&expected_facts).next() {
            program_error(
                errors,
                format!(
                    "baseline authority for {:?} contains extra fact {fact:?}",
                    expected_root.root()
                ),
            );
        }
    }
}

fn verify_root_shape(
    root: &StaticLifecycleRootAuthority,
    declared_fields: &BTreeSet<crate::identity::StaticFieldId>,
    errors: &mut Vec<MirVerificationError>,
) {
    for pair in root.effects().windows(2) {
        match pair[0].cmp(&pair[1]) {
            std::cmp::Ordering::Equal => program_error(
                errors,
                format!(
                    "baseline authority for {:?} contains a duplicate fact",
                    root.root()
                ),
            ),
            std::cmp::Ordering::Greater => program_error(
                errors,
                format!(
                    "baseline authority facts for {:?} are not in canonical order",
                    root.root()
                ),
            ),
            std::cmp::Ordering::Less => {}
        }
    }
    for fact in root.effects() {
        if !declared_fields.contains(&fact.target()) {
            program_error(
                errors,
                format!(
                    "baseline authority for {:?} names foreign static field {}",
                    root.root(),
                    fact.target()
                ),
            );
        }
    }
}

fn verify_dependency_order(
    program: &PlannedMirProgram,
    authority: &StaticLifecycleAuthority,
    errors: &mut Vec<MirVerificationError>,
) {
    let derived = match root_effects::dependency_pairs_for_definitions(
        program.preliminary().program(),
        program.lifecycle_mir().definitions(),
        authority,
    ) {
        Ok(pairs) => pairs,
        Err(error) => {
            program_error(
                errors,
                format!("cannot derive dependencies from baseline authority: {error:?}"),
            );
            return;
        }
    };
    let positions = derived::positions(program.lifecycle());
    for (prerequisite, dependent) in derived {
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
                    "baseline authority dependency {prerequisite} -> {dependent} violates activation order"
                ),
            );
        }
    }
}
