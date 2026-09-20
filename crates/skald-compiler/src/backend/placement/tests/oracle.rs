//! Small executable specification. It knows neither selected opcodes nor producer maps.
//! Keep it independent when the production checker arrives; it creates no authority.
use crate::backend::selected::{BankKind, ResourceCatalog, UnitId, ViewId};
use std::collections::{BTreeMap, BTreeSet};

use super::super::{
    check::SolverObservation,
    model::PlacementDraft,
    requirements::{CheckLocation, CheckReason, Requirements},
    state::State as ProductionState,
};
use crate::backend::{graph::SelectedBlockId, selected::Payload};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Loc {
    Register(ViewId),
    Home(usize),
}
type State = BTreeMap<Loc, BTreeSet<usize>>;
#[derive(Clone)]
enum Action {
    Define(usize, Loc),
    Copy(usize, Loc, Loc),
    Use(usize, Loc),
    Clobber(UnitId),
    // (successor parameter, argument token, captured source, assigned destination).
    Rebind(Vec<(usize, usize, Loc, Loc)>),
}
#[derive(Clone)]
struct Edge {
    target: usize,
    actions: Vec<Action>,
}
#[derive(Clone)]
struct Block {
    actions: Vec<Action>,
    edges: Vec<Edge>,
}
struct Machine {
    catalog: ResourceCatalog,
    argument: Loc,
    target: Loc,
    narrow: Loc,
    wide: Loc,
    float: Loc,
    volatile: UnitId,
    upper: UnitId,
}
fn machine(partial: bool) -> Machine {
    let mut catalog = ResourceCatalog::default();
    let integers = catalog.bank(BankKind::Integer);
    let floats = catalog.bank(BankKind::Float);
    let volatile = catalog.unit().unwrap();
    let low = catalog.unit().unwrap();
    let upper = if partial {
        catalog.unit().unwrap()
    } else {
        low
    };
    let vector = catalog.unit().unwrap();
    let argument = Loc::Register(catalog.view(integers, 64, &[volatile], false).unwrap());
    let target_unit = catalog.unit().unwrap();
    let target = Loc::Register(catalog.view(integers, 64, &[target_unit], false).unwrap());
    let narrow = Loc::Register(
        catalog
            .view(if partial { floats } else { integers }, 64, &[low], false)
            .unwrap(),
    );
    let wide_units = if partial { vec![low, upper] } else { vec![low] };
    let wide = Loc::Register(
        catalog
            .view(
                if partial { floats } else { integers },
                128,
                &wide_units,
                false,
            )
            .unwrap(),
    );
    let float = Loc::Register(catalog.view(floats, 64, &[vector], false).unwrap());
    Machine {
        catalog,
        argument,
        target,
        narrow,
        wide,
        float,
        volatile,
        upper,
    }
}
fn overwrite(catalog: &ResourceCatalog, state: &mut State, location: Loc, tokens: BTreeSet<usize>) {
    for (other, contents) in state.iter_mut() {
        let aliases = match (*other, location) {
            (Loc::Register(a), Loc::Register(b)) => catalog.overlaps(a, b).unwrap(),
            _ => *other == location,
        };
        if aliases {
            contents.clear();
        }
    }
    state.insert(location, tokens);
}
fn run(
    catalog: &ResourceCatalog,
    state: &mut State,
    actions: &[Action],
    strict: bool,
) -> Result<(), usize> {
    for (ordinal, action) in actions.iter().enumerate() {
        let has = |token: usize, location: Loc| {
            state.get(&location).is_some_and(|set| set.contains(&token))
        };
        match action {
            Action::Define(token, location) => {
                for contents in state.values_mut() {
                    contents.remove(token);
                }
                overwrite(catalog, state, *location, BTreeSet::from([*token]));
            }
            Action::Copy(token, source, destination) => {
                let available = has(*token, *source);
                if strict && !available {
                    return Err(ordinal);
                }
                let captured = state.get(source).cloned().unwrap_or_default();
                overwrite(
                    catalog,
                    state,
                    *destination,
                    if available { captured } else { BTreeSet::new() },
                );
            }
            Action::Use(token, location) => {
                if strict && !has(*token, *location) {
                    return Err(ordinal);
                }
            }
            Action::Clobber(unit) => {
                for (location, contents) in state.iter_mut() {
                    if matches!(location, Loc::Register(view) if catalog.view_units(*view).unwrap().contains(unit))
                    {
                        contents.clear();
                    }
                }
            }
            Action::Rebind(bindings) => {
                let available: Vec<_> = bindings
                    .iter()
                    .map(|(_, argument, source, _)| has(*argument, *source))
                    .collect();
                if strict && available.iter().any(|present| !present) {
                    return Err(ordinal);
                }
                let mut destinations: BTreeMap<Loc, (BTreeSet<usize>, BTreeSet<usize>)> =
                    BTreeMap::new();
                for ((parameter, _, source, destination), present) in
                    bindings.iter().zip(&available)
                {
                    let captured = if *present {
                        state.get(source).cloned().unwrap_or_default()
                    } else {
                        BTreeSet::new()
                    };
                    let (parameters, common) = destinations
                        .entry(*destination)
                        .or_insert_with(|| (BTreeSet::new(), captured.clone()));
                    parameters.insert(*parameter);
                    common.retain(|identity| captured.contains(identity));
                }
                if strict && destinations.values().any(|(_, common)| common.is_empty()) {
                    return Err(ordinal);
                }
                let locations: Vec<_> = destinations.keys().copied().collect();
                for (index, &left) in locations.iter().enumerate() {
                    for &right in &locations[index + 1..] {
                        if matches!((left, right), (Loc::Register(a), Loc::Register(b)) if catalog.overlaps(a, b).unwrap())
                        {
                            return Err(ordinal);
                        }
                    }
                }
                for (parameter, _, _, _) in bindings {
                    for contents in state.values_mut() {
                        contents.remove(parameter);
                    }
                }
                // Equal captured arguments may share one destination, written once.
                for (destination, (parameters, common)) in destinations {
                    overwrite(
                        catalog,
                        state,
                        destination,
                        if !common.is_empty() {
                            parameters
                        } else {
                            BTreeSet::new()
                        },
                    );
                }
            }
        }
    }
    Ok(())
}
fn straight(machine: &Machine, actions: &[Action]) -> Result<(), usize> {
    run(&machine.catalog, &mut State::new(), actions, true)
}

pub(in crate::backend) fn check_native_resource_events(
    catalog: &ResourceCatalog,
    argument: ViewId,
    target: ViewId,
    byte: ViewId,
    full: ViewId,
    float: ViewId,
    clobbers: &[UnitId],
) {
    let argument = Loc::Register(argument);
    let target = Loc::Register(target);
    let mut state = State::new();
    let actions = [
        Action::Define(0, target),
        Action::Define(1, argument),
        Action::Copy(0, target, Loc::Home(0)),
        Action::Copy(1, argument, Loc::Register(float)),
        Action::Use(0, target),
        Action::Use(1, Loc::Register(float)),
    ];
    run(catalog, &mut state, &actions, true).unwrap();
    for &unit in clobbers {
        run(catalog, &mut state, &[Action::Clobber(unit)], true).unwrap();
    }
    assert_eq!(
        run(catalog, &mut state, &[Action::Use(0, target)], true),
        Err(0)
    );
    assert_eq!(
        run(catalog, &mut state, &[Action::Use(0, Loc::Home(0))], true),
        Ok(())
    );
    assert_eq!(
        run(
            catalog,
            &mut state,
            &[
                Action::Define(2, Loc::Register(full)),
                Action::Define(3, Loc::Register(byte)),
                Action::Use(2, Loc::Register(full))
            ],
            true
        ),
        Err(2)
    );
}
fn intersect(left: &mut State, right: &State) {
    for (location, contents) in left {
        contents.retain(|token| right.get(location).is_some_and(|set| set.contains(token)));
    }
}
/// Greatest must fixed point from finite top, then inspect uses in stable states.
/// Diagnostics: stable block order, instruction order, edge occurrence order.
fn graph(
    machine: &Machine,
    blocks: &[Block],
    locations: &[Loc],
    tokens: &[usize],
) -> Result<usize, (usize, usize)> {
    let mut reachable = BTreeSet::from([0]);
    loop {
        let old = reachable.clone();
        for &block in &old {
            for edge in &blocks[block].edges {
                reachable.insert(edge.target);
            }
        }
        if reachable == old {
            break;
        }
    }
    let top: State = locations
        .iter()
        .map(|location| (*location, tokens.iter().copied().collect()))
        .collect();
    let entry: State = locations
        .iter()
        .map(|location| (*location, BTreeSet::new()))
        .collect();
    let mut states = vec![top.clone(); blocks.len()];
    states[0] = entry.clone();
    let bound = blocks.len() * locations.len() * tokens.len() + 2;
    for iteration in 1..=bound {
        let mut next = vec![top.clone(); blocks.len()];
        next[0] = entry.clone();
        for &block in &reachable {
            let mut output = states[block].clone();
            run(&machine.catalog, &mut output, &blocks[block].actions, false).unwrap();
            for edge in &blocks[block].edges {
                let mut outgoing = output.clone();
                run(&machine.catalog, &mut outgoing, &edge.actions, false).unwrap();
                intersect(&mut next[edge.target], &outgoing);
            }
        }
        if next == states {
            for &block in &reachable {
                let mut output = states[block].clone();
                run(&machine.catalog, &mut output, &blocks[block].actions, true)
                    .map_err(|ordinal| (block, ordinal))?;
                for (slot, edge) in blocks[block].edges.iter().enumerate() {
                    let mut outgoing = output.clone();
                    run(&machine.catalog, &mut outgoing, &edge.actions, true)
                        .map_err(|_| (block, blocks[block].actions.len() + slot))?;
                }
            }
            return Ok(iteration);
        }
        states = next;
    }
    panic!("finite descending-state bound exceeded");
}

/// Independent Jacobi schedule over the production transition semantics.
///
/// The action oracle above owns the semantic model. This adapter deliberately
/// duplicates only reachability, round joining, termination and stable replay,
/// so future production scheduling can be compared without sharing its control
/// flow.
pub(super) fn observe_round_solver<P: Payload>(
    requirements: &Requirements<'_, P>,
    draft: &PlacementDraft<'_, '_, P>,
) -> SolverObservation {
    let mut reachable = BTreeSet::from([requirements.entry]);
    let mut pending = vec![requirements.entry];
    while let Some(block) = pending.pop() {
        for (target, _) in &requirements.blocks[&block].edges {
            if reachable.insert(*target) {
                pending.push(*target);
            }
        }
    }

    let Some(bound) = oracle_iteration_bound(
        reachable.len(),
        requirements.locations.len(),
        requirements.tokens.len(),
    ) else {
        return SolverObservation {
            fixed_point: None,
            outcome: Err(requirements.failure(CheckLocation::Entry, CheckReason::Capacity)),
        };
    };
    let top = ProductionState::top(requirements.locations.len(), &requirements.tokens);
    let mut states: BTreeMap<SelectedBlockId, ProductionState> = reachable
        .iter()
        .map(|&block| (block, top.clone()))
        .collect();
    states.insert(requirements.entry, requirements.seed.clone());

    for _ in 0..bound {
        let mut joined: BTreeMap<SelectedBlockId, ProductionState> = reachable
            .iter()
            .map(|&block| (block, top.clone()))
            .collect();
        joined.insert(requirements.entry, requirements.seed.clone());
        for &block in &reachable {
            let mut output = states[&block].clone();
            if let Err(failure) = requirements.block(draft, &mut output, block, false) {
                return SolverObservation {
                    fixed_point: None,
                    outcome: Err(failure),
                };
            }
            for (slot, (target, _)) in requirements.blocks[&block].edges.iter().enumerate() {
                let mut outgoing = output.clone();
                if let Err(failure) = requirements.edge(draft, &mut outgoing, block, slot, false) {
                    return SolverObservation {
                        fixed_point: None,
                        outcome: Err(failure),
                    };
                }
                joined
                    .get_mut(target)
                    .expect("reachable successor")
                    .intersect(&outgoing);
            }
        }
        if joined == states {
            return SolverObservation {
                fixed_point: Some(requirements.digest(&states)),
                outcome: replay_stable_states(requirements, draft, &states),
            };
        }
        states = joined;
    }

    SolverObservation {
        fixed_point: None,
        outcome: Err(requirements.failure(CheckLocation::Entry, CheckReason::Convergence)),
    }
}

fn replay_stable_states<P: Payload>(
    requirements: &Requirements<'_, P>,
    draft: &PlacementDraft<'_, '_, P>,
    states: &BTreeMap<SelectedBlockId, ProductionState>,
) -> Result<(), super::super::requirements::CheckFailure> {
    for (&block, state) in states {
        let mut output = state.clone();
        requirements.block(draft, &mut output, block, true)?;
        for slot in 0..requirements.blocks[&block].edges.len() {
            let mut outgoing = output.clone();
            requirements.edge(draft, &mut outgoing, block, slot, true)?;
        }
    }
    Ok(())
}

pub(super) fn oracle_iteration_bound(
    blocks: usize,
    locations: usize,
    tokens: usize,
) -> Option<usize> {
    blocks
        .checked_mul(locations)?
        .checked_mul(tokens)?
        .checked_add(1)
}
#[test]
fn destructive_tie_requires_a_surviving_copy_of_a_live_input() {
    let m = machine(false);
    let mut actions = vec![
        Action::Define(0, m.argument),
        Action::Use(0, m.argument),
        Action::Define(1, m.argument),
        Action::Use(0, m.argument),
    ];
    assert_eq!(straight(&m, &actions), Err(3));
    actions.insert(1, Action::Copy(0, m.argument, Loc::Home(0)));
    *actions.last_mut().unwrap() = Action::Use(0, Loc::Home(0));
    assert_eq!(straight(&m, &actions), Ok(()));
}
#[test]
fn calls_kill_volatile_contents_after_uses_before_results() {
    let m = machine(false);
    assert_eq!(
        straight(
            &m,
            &[
                Action::Define(0, m.argument),
                Action::Use(0, m.argument),
                Action::Clobber(m.volatile),
                Action::Use(0, m.argument)
            ]
        ),
        Err(3)
    );
    assert_eq!(
        straight(
            &m,
            &[
                Action::Define(0, m.argument),
                Action::Copy(0, m.argument, Loc::Home(0)),
                Action::Clobber(m.volatile),
                Action::Define(1, m.argument),
                Action::Use(1, m.argument),
                Action::Use(0, Loc::Home(0))
            ]
        ),
        Ok(())
    );
}
#[test]
fn secured_indirect_target_must_survive_argument_marshalling() {
    let m = machine(false);
    let actions = [
        Action::Define(0, m.target),
        Action::Define(1, m.argument),
        Action::Copy(1, m.argument, m.target),
        Action::Use(0, m.target),
    ];
    assert_eq!(straight(&m, &actions), Err(3));
    assert_eq!(
        straight(
            &m,
            &[
                Action::Define(0, Loc::Home(0)),
                Action::Define(1, m.argument),
                Action::Copy(1, m.argument, Loc::Home(1)),
                Action::Copy(0, Loc::Home(0), m.target),
                Action::Copy(1, Loc::Home(1), m.argument),
                Action::Use(1, m.argument),
                Action::Use(0, m.target)
            ]
        ),
        Ok(())
    );
}
#[test]
fn mixed_bank_cycle_needs_explicit_scratch_and_preserves_bits() {
    let m = machine(false);
    let prefix = [Action::Define(0, m.argument), Action::Define(1, m.float)];
    let mut broken = prefix.to_vec();
    broken.extend([
        Action::Copy(0, m.argument, m.float),
        Action::Copy(1, m.float, m.argument),
    ]);
    assert_eq!(straight(&m, &broken), Err(3));
    let mut valid = prefix.to_vec();
    valid.extend([
        Action::Copy(0, m.argument, Loc::Home(0)),
        Action::Copy(1, m.float, m.argument),
        Action::Copy(0, Loc::Home(0), m.float),
        Action::Use(0, m.float),
        Action::Use(1, m.argument),
    ]);
    assert_eq!(straight(&m, &valid), Ok(()));
}
#[test]
fn partial_preservation_and_overlapping_widths_use_units_not_bank_roles() {
    // Synthetic link/argument roles differ from x86; a vector's low lane is preserved.
    let m = machine(true);
    assert_eq!(
        straight(
            &m,
            &[
                Action::Define(0, m.narrow),
                Action::Clobber(m.upper),
                Action::Use(0, m.narrow)
            ]
        ),
        Ok(())
    );
    assert_eq!(
        straight(
            &m,
            &[
                Action::Define(0, m.wide),
                Action::Clobber(m.upper),
                Action::Use(0, m.wide)
            ]
        ),
        Err(2)
    );
    assert_eq!(
        straight(
            &m,
            &[
                Action::Define(0, m.wide),
                Action::Define(1, m.narrow),
                Action::Use(0, m.wide)
            ]
        ),
        Err(2)
    );
}
#[test]
fn duplicate_successors_intersect_each_edge_occurrence() {
    let m = machine(false);
    let home = Loc::Home(0);
    let edge = Edge {
        target: 1,
        actions: vec![Action::Copy(0, m.argument, home)],
    };
    let mut blocks = vec![
        Block {
            actions: vec![Action::Define(0, m.argument)],
            edges: vec![
                edge.clone(),
                Edge {
                    target: 1,
                    actions: vec![],
                },
            ],
        },
        Block {
            actions: vec![Action::Use(0, home)],
            edges: vec![],
        },
    ];
    assert_eq!(graph(&m, &blocks, &[m.argument, home], &[0]), Err((1, 0)));
    blocks[0].edges[1] = edge;
    assert!(graph(&m, &blocks, &[m.argument, home], &[0]).is_ok());
}
#[test]
fn loop_parameter_rebinding_invalidates_saved_previous_epochs() {
    let m = machine(true);
    let parameter = Loc::Home(0);
    let saved = Loc::Home(1);
    let bind = |argument, source| Action::Rebind(vec![(1, argument, source, parameter)]);
    let mut blocks = vec![
        Block {
            actions: vec![Action::Define(0, m.argument)],
            edges: vec![Edge {
                target: 1,
                actions: vec![bind(0, m.argument)],
            }],
        },
        Block {
            actions: vec![
                Action::Use(1, parameter),
                Action::Copy(1, parameter, saved),
                Action::Define(2, m.argument),
            ],
            edges: vec![
                Edge {
                    target: 1,
                    actions: vec![bind(2, m.argument)],
                },
                Edge {
                    target: 2,
                    actions: vec![bind(2, m.argument)],
                },
            ],
        },
        Block {
            actions: vec![Action::Use(1, parameter), Action::Use(1, saved)],
            edges: vec![],
        },
    ];
    assert_eq!(
        graph(&m, &blocks, &[m.argument, parameter, saved], &[0, 1, 2]),
        Err((2, 1))
    );
    blocks[2].actions.pop();
    assert!(graph(&m, &blocks, &[m.argument, parameter, saved], &[0, 1, 2]).unwrap() <= 29);
}
#[test]
fn loop_instruction_redefinition_invalidates_copies_from_previous_iteration() {
    let m = machine(false);
    let saved = Loc::Home(0);
    let definition = [Action::Define(0, m.argument)];
    let mut state = State::new();
    // Replay the same static definition on successive dynamic loop iterations.
    run(&m.catalog, &mut state, &definition, true).unwrap();
    run(
        &m.catalog,
        &mut state,
        &[Action::Copy(0, m.argument, saved)],
        true,
    )
    .unwrap();
    run(&m.catalog, &mut state, &definition, true).unwrap();
    assert_eq!(
        run(&m.catalog, &mut state, &[Action::Use(0, saved)], true),
        Err(0)
    );
    assert_eq!(
        run(
            &m.catalog,
            &mut state,
            &[Action::Copy(0, m.argument, saved), Action::Use(0, saved)],
            true
        ),
        Ok(())
    );
}

#[test]
fn equal_parameter_identities_survive_copies_but_unequal_arguments_cannot_share_storage() {
    let m = machine(true);
    let a = Loc::Home(0);
    let b = Loc::Home(1);
    assert_eq!(
        straight(
            &m,
            &[
                Action::Define(0, m.argument),
                Action::Rebind(vec![(1, 0, m.argument, a), (2, 0, m.argument, a)]),
                Action::Copy(1, a, b),
                Action::Use(2, b)
            ]
        ),
        Ok(())
    );
    assert_eq!(
        straight(
            &m,
            &[
                Action::Define(0, m.argument),
                Action::Define(3, m.target),
                Action::Rebind(vec![(1, 0, m.argument, a), (2, 3, m.target, a)])
            ]
        ),
        Err(2)
    );
}

#[test]
fn original_preserved_contents_need_explicit_save_and_restore() {
    let m = machine(true);
    let save = Loc::Home(0);
    // The disjoint identity 100 denotes original incoming contents, not a body value.
    assert_eq!(
        straight(
            &m,
            &[
                Action::Define(100, m.target),
                Action::Define(0, m.target),
                Action::Use(100, m.target)
            ]
        ),
        Err(2)
    );
    assert_eq!(
        straight(
            &m,
            &[
                Action::Define(100, m.target),
                Action::Copy(100, m.target, save),
                Action::Define(0, m.target),
                Action::Copy(100, save, m.target),
                Action::Use(100, m.target)
            ]
        ),
        Ok(())
    );
}

#[test]
fn cyclic_parameter_swaps_reach_a_finite_must_fixed_point() {
    let m = machine(true);
    let a = Loc::Home(0);
    let b = Loc::Home(1);
    let blocks = vec![
        Block {
            actions: vec![Action::Define(0, m.argument), Action::Define(3, m.target)],
            edges: vec![Edge {
                target: 1,
                actions: vec![Action::Rebind(vec![
                    (1, 0, m.argument, a),
                    (2, 3, m.target, b),
                ])],
            }],
        },
        Block {
            actions: vec![Action::Use(1, a), Action::Use(2, b)],
            edges: vec![Edge {
                target: 1,
                actions: vec![Action::Rebind(vec![(1, 2, b, a), (2, 1, a, b)])],
            }],
        },
    ];
    assert!(graph(&m, &blocks, &[m.argument, m.target, a, b], &[0, 1, 2, 3]).unwrap() <= 34);
}
#[test]
fn simultaneous_parameter_swap_reads_before_rebinding() {
    let m = machine(true);
    let a = Loc::Home(0);
    let b = Loc::Home(1);
    assert_eq!(
        straight(
            &m,
            &[
                Action::Define(0, a),
                Action::Define(1, b),
                Action::Rebind(vec![(0, 1, b, a), (1, 0, a, b)]),
                Action::Use(0, a),
                Action::Use(1, b)
            ]
        ),
        Ok(())
    );
}
