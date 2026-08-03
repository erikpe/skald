//! Class-graph validation and canonical hierarchy construction.

use std::collections::BTreeMap;

use super::*;

pub(super) fn build_class_hierarchy(
    classes: &ResolvedClassDeclarationTable,
    class_symbols: &[ClassSymbols],
    diagnostics: &mut Diagnostics,
) -> ResolvedClassHierarchy {
    let direct_bases: Vec<_> = classes
        .iter()
        .map(|class| class.direct_base.map(|base| base.class))
        .collect();

    let cycles = inheritance_cycles(&direct_bases);
    for cycle in &cycles {
        diagnostics.push(cycle_diagnostic(classes, cycle));
    }
    let ancestry_valid = ancestry_validity(&direct_bases, &cycles);

    let entries = classes
        .iter()
        .zip(class_symbols)
        .map(|(class, symbols)| ResolvedClassHierarchyEntry {
            class: class.id,
            direct_base: direct_bases[class.id.index()],
            ancestry_valid: ancestry_valid[class.id.index()],
            members: symbols
                .ordinary
                .iter()
                .map(|(name, symbol)| (name.clone(), resolved_member(symbol.kind)))
                .collect::<BTreeMap<_, _>>(),
        })
        .collect();
    ResolvedClassHierarchy::new(entries)
}

fn inheritance_cycles(direct_bases: &[Option<ClassId>]) -> Vec<Vec<ClassId>> {
    const UNVISITED: u8 = 0;
    const VISITING: u8 = 1;
    const FINISHED: u8 = 2;

    let mut states = vec![UNVISITED; direct_bases.len()];
    let mut positions = vec![None; direct_bases.len()];
    let mut cycles = Vec::new();

    for start in 0..direct_bases.len() {
        if states[start] != UNVISITED {
            continue;
        }

        let mut path = Vec::new();
        let mut current = Some(ClassId::new(start));
        while let Some(class) = current {
            let index = class.index();
            if index >= direct_bases.len() {
                break;
            }
            match states[index] {
                UNVISITED => {
                    states[index] = VISITING;
                    positions[index] = Some(path.len());
                    path.push(class);
                    current = direct_bases[index];
                }
                VISITING => {
                    let cycle_start =
                        positions[index].expect("visiting class must belong to the active path");
                    cycles.push(normalize_cycle(path[cycle_start..].to_vec()));
                    break;
                }
                FINISHED => break,
                _ => unreachable!("class visitation state must be valid"),
            }
        }

        for class in path {
            states[class.index()] = FINISHED;
            positions[class.index()] = None;
        }
    }

    cycles.sort_by_key(|cycle| cycle[0].index());
    cycles
}

fn normalize_cycle(mut cycle: Vec<ClassId>) -> Vec<ClassId> {
    let earliest = cycle
        .iter()
        .enumerate()
        .min_by_key(|(_, class)| class.index())
        .map(|(index, _)| index)
        .expect("inheritance cycle cannot be empty");
    cycle.rotate_left(earliest);
    cycle
}

fn ancestry_validity(direct_bases: &[Option<ClassId>], cycles: &[Vec<ClassId>]) -> Vec<bool> {
    let mut validity = vec![None; direct_bases.len()];
    for cycle in cycles {
        for class in cycle {
            validity[class.index()] = Some(false);
        }
    }

    for start in 0..direct_bases.len() {
        if validity[start].is_some() {
            continue;
        }
        let mut path = Vec::new();
        let mut current = Some(ClassId::new(start));
        let outcome = loop {
            let Some(class) = current else {
                break true;
            };
            if class.index() >= direct_bases.len() {
                break false;
            }
            if let Some(valid) = validity[class.index()] {
                break valid;
            }
            path.push(class);
            current = direct_bases[class.index()];
        };
        for class in path {
            validity[class.index()] = Some(outcome);
        }
    }

    validity
        .into_iter()
        .map(|valid| valid.expect("every class ancestry must be classified"))
        .collect()
}

fn cycle_diagnostic(classes: &ResolvedClassDeclarationTable, cycle: &[ClassId]) -> Diagnostic {
    let names: Vec<_> = cycle
        .iter()
        .map(|class| {
            classes
                .get(*class)
                .expect("cycle class must exist")
                .name
                .as_str()
        })
        .collect();
    let mut path = names.join(" -> ");
    path.push_str(" -> ");
    path.push_str(names[0]);

    let first = classes.get(cycle[0]).expect("cycle class must exist");
    let first_base = first
        .direct_base
        .expect("cycle class must have a direct base");
    let mut diagnostic =
        Diagnostic::error(INHERITANCE_CYCLE, format!("inheritance cycle: `{path}`"))
            .with_primary_label(
                first_base.span,
                format!("class `{}` participates in this cycle", first.name),
            );

    for &class in &cycle[1..] {
        let declaration = classes.get(class).expect("cycle class must exist");
        let base = declaration
            .direct_base
            .expect("cycle class must have a direct base");
        diagnostic = diagnostic.with_secondary_label(
            base.span,
            format!("class `{}` continues the cycle", declaration.name),
        );
    }

    diagnostic.with_note("class inheritance must form an acyclic chain")
}

const fn resolved_member(kind: OrdinaryMemberSymbolKind) -> ResolvedClassMember {
    match kind {
        OrdinaryMemberSymbolKind::Field(field) => ResolvedClassMember::Field(field),
        OrdinaryMemberSymbolKind::StaticField(field) => ResolvedClassMember::StaticField(field),
        OrdinaryMemberSymbolKind::Method(method) => ResolvedClassMember::Method(method),
    }
}

#[cfg(test)]
mod tests;
