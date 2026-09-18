//! Local constraint search; no liveness analysis or persistent register policy.
use super::{
    super::transfers::aliases,
    super::{model::*, requirements::*, target::PlacementTarget},
};
use crate::backend::{
    graph::SelectedValueId,
    plan::SignatureId,
    selected::{Bundle, Constraint, Description, OperandRole, Payload, Representation, Timing},
};
use std::collections::BTreeMap;

struct Group {
    slots: Vec<usize>,
    candidates: Vec<Location>,
}
fn conflict(d: &Description<'_>, a: usize, b: usize) -> bool {
    let (a, b) = (d.operands[a], d.operands[b]);
    match (a.role, b.role) {
        (OperandRole::Use, OperandRole::Use) => {
            a.value != b.value || a.representation != b.representation
        }
        (OperandRole::Definition, OperandRole::Definition) => true,
        _ => {
            let (u, v) = if a.role == OperandRole::Use {
                (a, b)
            } else {
                (b, a)
            };
            u.timing == Timing::Late && v.timing == Timing::Early
        }
    }
}
fn search<P>(
    draft: &PlacementDraft<'_, '_, P>,
    target: &impl PlacementTarget,
    d: &Description<'_>,
    groups: &[Group],
    index: usize,
    chosen: &mut [Option<Location>],
) -> bool {
    if index == groups.len() {
        return true;
    }
    let resources = &draft.selected().draft().context().resources;
    for &location in &groups[index].candidates {
        let legal = groups[index].slots.iter().all(|&slot| {
            let op = d.operands[slot];
            if let Location::Resource(view) = location {
                let units = resources.view_units(view).expect("verified view");
                if d.clobbers.iter().any(|&(timing, unit)| {
                    units.contains(&unit)
                        && ((op.role == OperandRole::Use
                            && op.timing == Timing::Late
                            && timing == Timing::Early)
                            || (op.role == OperandRole::Definition
                                && op.timing == Timing::Early
                                && timing == Timing::Late))
                }) {
                    return false;
                }
            }
            chosen.iter().enumerate().all(|(other, &other_location)| {
                other_location.is_none_or(|other_location| {
                    !conflict(d, slot, other)
                        || !aliases(
                            draft,
                            target,
                            location,
                            op.representation,
                            other_location,
                            d.operands[other].representation,
                        )
                })
            })
        });
        if !legal {
            continue;
        }
        for &slot in &groups[index].slots {
            chosen[slot] = Some(location);
        }
        if search(draft, target, d, groups, index + 1, chosen) {
            return true;
        }
        for &slot in &groups[index].slots {
            chosen[slot] = None;
        }
    }
    false
}
pub(super) fn assign<P: Payload>(
    draft: &mut PlacementDraft<'_, '_, P>,
    target: &impl PlacementTarget,
    d: &Description<'_>,
    site: Site,
    signature: SignatureId,
    homes: &BTreeMap<SelectedValueId, (Location, Representation)>,
) -> Result<Vec<Location>, CheckFailure> {
    let mut labels: Vec<_> = (0..d.operands.len()).collect();
    for tie in d.ties {
        let (a, b) = (labels[tie.input], labels[tie.output]);
        for label in &mut labels {
            if *label == b {
                *label = a;
            }
        }
    }
    let mut groups = BTreeMap::<usize, Group>::new();
    for (slot, op) in d.operands.iter().enumerate() {
        let mut candidates = match op.constraint {
            Constraint::Fixed(view) => vec![Location::Resource(view)],
            Constraint::AbiSlot { area, index } => vec![Location::Abi {
                signature,
                area,
                index,
            }],
            Constraint::Resources { views, memory } => {
                let mut locations: Vec<_> = views.iter().copied().map(Location::Resource).collect();
                if memory {
                    locations.push(homes[&op.value].0);
                }
                locations
            }
        };
        candidates.sort_unstable();
        candidates.dedup();
        if let Some(group) = groups.get_mut(&labels[slot]) {
            group.slots.push(slot);
            group.candidates.retain(|l| candidates.contains(l));
        } else {
            groups.insert(
                labels[slot],
                Group {
                    slots: vec![slot],
                    candidates,
                },
            );
        }
    }
    let mut groups: Vec<_> = groups.into_values().collect();
    groups.sort_by_key(|g| (g.candidates.len(), g.slots[0]));
    let mut chosen = vec![None; d.operands.len()];
    let scratch = match &d.bundle {
        Some(Bundle::Bounded { scratch, .. }) => scratch.as_ref(),
        _ => &[],
    };
    // Scratch is reserved first: it must be disjoint from every operand, even
    // when operand events could otherwise share a resource.
    let requests: Vec<_> = scratch
        .iter()
        .enumerate()
        .flat_map(|(group, s)| (0..usize::from(s.count.get())).map(move |slot| (group, slot, s)))
        .collect();
    fn reserve<P>(
        draft: &PlacementDraft<'_, '_, P>,
        target: &impl PlacementTarget,
        d: &Description<'_>,
        groups: &[Group],
        requests: &[(usize, usize, &crate::backend::selected::Scratch<'_>)],
        selected: &mut Vec<Location>,
        chosen: &mut [Option<Location>],
    ) -> bool {
        if selected.len() == requests.len() {
            let filtered: Vec<_> = groups
                .iter()
                .map(|g| Group {
                    slots: g.slots.clone(),
                    candidates: g
                        .candidates
                        .iter()
                        .copied()
                        .filter(|&l| {
                            g.slots.iter().all(|&slot| {
                                selected.iter().enumerate().all(|(i, &s)| {
                                    !aliases(
                                        draft,
                                        target,
                                        l,
                                        d.operands[slot].representation,
                                        s,
                                        requests[i].2.representation,
                                    )
                                })
                            })
                        })
                        .collect(),
                })
                .collect();
            return search(draft, target, d, &filtered, 0, chosen);
        }
        let request = requests[selected.len()].2;
        let mut views = request.views.to_vec();
        views.sort_unstable();
        views.dedup();
        for view in views {
            if target.preserved_views().iter().any(|&original| {
                draft
                    .selected()
                    .draft()
                    .context()
                    .resources
                    .overlaps(view, original)
                    .expect("target views")
            }) {
                continue;
            }
            let location = Location::Resource(view);
            if selected.iter().enumerate().any(|(i, &s)| {
                aliases(
                    draft,
                    target,
                    location,
                    request.representation,
                    s,
                    requests[i].2.representation,
                )
            }) {
                continue;
            }
            selected.push(location);
            if reserve(draft, target, d, groups, requests, selected, chosen) {
                return true;
            }
            selected.pop();
        }
        false
    }
    let mut reserved = vec![];
    if !reserve(
        draft,
        target,
        d,
        &groups,
        &requests,
        &mut reserved,
        &mut chosen,
    ) {
        return Err(CheckFailure::new(
            CheckLocation::Site(site),
            CheckReason::Constraint,
        ));
    }
    let result: Vec<_> = chosen
        .into_iter()
        .map(|l| l.expect("complete local solution"))
        .collect();
    for (slot, &location) in result.iter().enumerate() {
        draft
            .assign(Assignment::Operand { site, slot }, location)
            .expect("one coordinate");
    }
    for ((group, slot, _), location) in requests.into_iter().zip(reserved) {
        draft
            .assign(Assignment::Scratch { site, group, slot }, location)
            .expect("one scratch coordinate");
    }
    Ok(result)
}
