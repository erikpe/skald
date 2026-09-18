//! Sole acceptance constructor: legality, finite convergence, then strict replay.
use super::{model::*, requirements::*, state::State, target::PlacementTarget};
use crate::backend::{
    graph::SelectedBlockId,
    selected::{Bundle, Event, Flow, Payload, Phase, SelectedFact, VerifiedSelectedCallable},
};
use std::collections::{BTreeMap, BTreeSet};

pub(in crate::backend) struct CheckedPlacement<'s, 'p, P> {
    draft: PlacementDraft<'s, 'p, P>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'s, 'p, P> CheckedPlacement<'s, 'p, P> {
    pub(in crate::backend) fn selected(&self) -> &'s VerifiedSelectedCallable<'p, P> {
        self.draft.selected
    }
    pub(in crate::backend) fn assignment(&self, assignment: Assignment) -> Option<Location> {
        self.draft.assignments.get(&assignment).copied()
    }
    pub(in crate::backend) fn storage(&self) -> &[Storage] {
        &self.draft.storage
    }
    pub(in crate::backend) fn transfers(&self, point: TransferPoint) -> &[Transfer] {
        self.draft.transfers.get(&point).map_or(&[], Vec::as_slice)
    }
    pub(in crate::backend) fn require_selected(
        &self,
        selected: &VerifiedSelectedCallable<'p, P>,
    ) -> Result<(), PlacementError> {
        self.draft.require_selected(selected)
    }
}
pub(in crate::backend) fn check_placement<'s, 'p, P: Payload>(
    draft: PlacementDraft<'s, 'p, P>,
    target: &impl PlacementTarget,
) -> Result<CheckedPlacement<'s, 'p, P>, CheckFailure> {
    draft.validate_structure().map_err(|error| {
        let location = match error {
            PlacementError::DuplicateAssignment(a)
            | PlacementError::MissingAssignment(a)
            | PlacementError::UnknownAssignment(a)
            | PlacementError::InvalidLocation(a) => CheckLocation::Assignment(a),
            PlacementError::InvalidStorage(i) => CheckLocation::Storage(i),
            PlacementError::InvalidTransfer(point, index) => {
                CheckLocation::Transfer { point, index }
            }
            PlacementError::UnknownTransferPoint(point) => {
                CheckLocation::Transfer { point, index: 0 }
            }
            _ => CheckLocation::Entry,
        };
        let mut failure = CheckFailure::new(location, CheckReason::Constraint);
        failure.structure = Some(Box::new(error));
        if let Some(site) = location.site() {
            let block = match site {
                Site::Instruction { block, .. } | Site::Terminal(block) => block,
            };
            let mut block_origin = None;
            draft
                .selected
                .visit::<()>(|fact| {
                    match fact {
                        SelectedFact::Block { id, origin, .. } if id == block => {
                            block_origin = origin
                        }
                        SelectedFact::Instruction {
                            block,
                            ordinal,
                            payload,
                        } if site == (Site::Instruction { block, ordinal }) => {
                            failure.origin = payload.span().or(block_origin)
                        }
                        SelectedFact::Terminal { block, payload, .. }
                            if site == Site::Terminal(block) =>
                        {
                            failure.origin = payload.and_then(Payload::span).or(block_origin)
                        }
                        _ => {}
                    }
                    Ok(())
                })
                .expect("infallible provenance visitor");
        }
        failure
    })?;
    let mut requirements = Requirements::collect(&draft, target)?;
    requirements.validate(&draft, target)?;
    requirements.seed(&draft)?;
    requirements.solve(&draft)?;
    // No sibling has access to this field or another checked constructor.
    Ok(CheckedPlacement { draft })
}
impl<P: Payload> Requirements<'_, P> {
    fn node(
        &self,
        draft: &PlacementDraft<'_, '_, P>,
        state: &mut State,
        site: Site,
        payload: &P,
        strict: bool,
    ) -> Result<(), CheckFailure> {
        self.transfers(draft, state, TransferPoint::Before(site), strict)?;
        let description = payload.describe();
        if let Some(Bundle::Bounded { scratch, .. }) = &description.bundle {
            for (group, scratch) in scratch.iter().enumerate() {
                for slot in 0..usize::from(scratch.count.get()) {
                    let location = draft.assignments[&Assignment::Scratch { site, group, slot }];
                    self.write(draft, state, location, BTreeSet::new());
                }
            }
        }
        // All uses in a phase precede its kills/definitions; descriptor order is frozen.
        let mut killed_slots = false;
        for event in description.events() {
            match event {
                Event::Operand { phase, slot } => {
                    let operand = description.operands[slot];
                    let assignment = Assignment::Operand { site, slot };
                    let location = draft.assignments[&assignment];
                    let token = TransferValue::Selected(operand.value);
                    if matches!(phase, Phase::EarlyUses | Phase::LateUses) {
                        if strict && !self.has(state, location, token) {
                            return Err(self.failure(
                                CheckLocation::Assignment(assignment),
                                CheckReason::MissingValue,
                            ));
                        }
                    } else {
                        // A call with no explicit register kills still invalidates ABI temporaries.
                        if phase == Phase::LateDefinitions
                            && description.call_signature.is_some()
                            && !killed_slots
                        {
                            self.kill_call_slots(state);
                            killed_slots = true;
                        }
                        state.forget(token);
                        self.write(draft, state, location, BTreeSet::from([token]));
                    }
                }
                Event::Clobber { phase, unit } => {
                    if phase == Phase::LateClobbers
                        && description.call_signature.is_some()
                        && !killed_slots
                    {
                        self.kill_call_slots(state);
                        killed_slots = true;
                    }
                    self.kill_unit(draft, state, unit);
                }
            }
        }
        if description.call_signature.is_some() && !killed_slots {
            self.kill_call_slots(state);
        }
        if matches!(site, Site::Instruction { .. }) {
            self.transfers(draft, state, TransferPoint::After(site), strict)?;
        }
        if strict && description.flow == Flow::Return {
            for &view in &self.preserved {
                if !self.has(
                    state,
                    Location::Resource(view),
                    TransferValue::Preserved(view),
                ) {
                    return Err(self.failure(CheckLocation::Site(site), CheckReason::Preservation));
                }
            }
        }
        Ok(())
    }
    fn block(
        &self,
        draft: &PlacementDraft<'_, '_, P>,
        state: &mut State,
        block: SelectedBlockId,
        strict: bool,
    ) -> Result<(), CheckFailure> {
        let facts = &self.blocks[&block];
        for (ordinal, &payload) in facts.instructions.iter().enumerate() {
            self.node(
                draft,
                state,
                Site::Instruction { block, ordinal },
                payload,
                strict,
            )?;
        }
        self.node(
            draft,
            state,
            Site::Terminal(block),
            facts.terminal.expect("verified terminal"),
            strict,
        )
    }
    fn edge(
        &self,
        draft: &PlacementDraft<'_, '_, P>,
        state: &mut State,
        block: SelectedBlockId,
        slot: usize,
        strict: bool,
    ) -> Result<(), CheckFailure> {
        let (target, arguments) = &self.blocks[&block].edges[slot];
        let parameters = &self.blocks[target].parameters;
        let location = CheckLocation::Edge { block, slot };
        let mut captures = vec![];
        for (index, &argument) in arguments.iter().enumerate() {
            let source = draft.assignments[&Assignment::EdgeArgument {
                block,
                edge: slot,
                slot: index,
            }];
            let available = self.has(state, source, TransferValue::Selected(argument));
            if strict && !available {
                return Err(self.failure(location, CheckReason::MissingValue));
            }
            captures.push(if available {
                self.contents(state, source).clone()
            } else {
                BTreeSet::new()
            });
        }
        self.transfers(draft, state, TransferPoint::Edge { block, slot }, strict)?;
        let mut destinations: BTreeMap<
            Location,
            (BTreeSet<TransferValue>, BTreeSet<TransferValue>),
        > = BTreeMap::new();
        for (index, (&parameter, &argument)) in parameters.iter().zip(arguments).enumerate() {
            let destination = draft.assignments[&Assignment::Parameter {
                block: *target,
                slot: index,
            }];
            let available = self.has(state, destination, TransferValue::Selected(argument));
            if strict && !available {
                return Err(self.failure(location, CheckReason::MissingValue));
            }
            let captured = if available {
                captures[index].clone()
            } else {
                BTreeSet::new()
            };
            let (rebound, common) = destinations
                .entry(destination)
                .or_insert_with(|| (BTreeSet::new(), captured.clone()));
            common.retain(|token| captured.contains(token));
            rebound.insert(TransferValue::Selected(parameter));
        }
        if strict && destinations.values().any(|(_, common)| common.is_empty()) {
            return Err(self.failure(location, CheckReason::Overlap));
        }
        for &parameter in parameters {
            state.forget(TransferValue::Selected(parameter));
        }
        for (destination, (rebound, common)) in destinations {
            self.write(
                draft,
                state,
                destination,
                if common.is_empty() {
                    BTreeSet::new()
                } else {
                    rebound
                },
            );
        }
        Ok(())
    }
    fn solve(&self, draft: &PlacementDraft<'_, '_, P>) -> Result<(), CheckFailure> {
        let mut reachable = BTreeSet::from([self.entry]);
        let mut pending = vec![self.entry];
        while let Some(block) = pending.pop() {
            for (target, _) in &self.blocks[&block].edges {
                if reachable.insert(*target) {
                    pending.push(*target);
                }
            }
        }
        let bound = iteration_bound(reachable.len(), self.locations.len(), self.tokens.len())
            .ok_or_else(|| self.failure(CheckLocation::Entry, CheckReason::Capacity))?;
        let top = State::top(self.locations.len(), &self.tokens);
        let mut states: BTreeMap<_, _> = reachable
            .iter()
            .map(|&block| (block, top.clone()))
            .collect();
        states.insert(self.entry, self.seed.clone());
        for _ in 0..bound {
            let mut next: BTreeMap<_, _> = reachable
                .iter()
                .map(|&block| (block, top.clone()))
                .collect();
            next.insert(self.entry, self.seed.clone());
            for &block in &reachable {
                let mut output = states[&block].clone();
                self.block(draft, &mut output, block, false)?;
                for (slot, (target, _)) in self.blocks[&block].edges.iter().enumerate() {
                    let mut outgoing = output.clone();
                    self.edge(draft, &mut outgoing, block, slot, false)?;
                    next.get_mut(target)
                        .expect("reachable successor")
                        .intersect(&outgoing);
                }
            }
            if next == states {
                for &block in &reachable {
                    let mut output = states[&block].clone();
                    self.block(draft, &mut output, block, true)?;
                    for slot in 0..self.blocks[&block].edges.len() {
                        let mut outgoing = output.clone();
                        self.edge(draft, &mut outgoing, block, slot, true)?;
                    }
                }
                return Ok(());
            }
            states = next;
        }
        Err(self.failure(CheckLocation::Entry, CheckReason::Convergence))
    }
}

fn iteration_bound(blocks: usize, locations: usize, tokens: usize) -> Option<usize> {
    blocks
        .checked_mul(locations)?
        .checked_mul(tokens)?
        .checked_add(1)
}

#[cfg(test)]
mod tests {
    use super::iteration_bound;
    #[test]
    fn finite_lattice_bound_checks_products_and_final_increment() {
        assert_eq!(iteration_bound(3, 4, 5), Some(61));
        assert_eq!(iteration_bound(1, 0, 0), Some(1));
        assert_eq!(iteration_bound(usize::MAX, 2, 1), None);
        assert_eq!(iteration_bound(1, usize::MAX, 2), None);
        assert_eq!(iteration_bound(1, 1, usize::MAX), None);
    }
}
