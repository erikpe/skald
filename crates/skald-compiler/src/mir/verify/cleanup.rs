//! Definite object-liveness analysis for cleanup verification.

use std::collections::{HashSet, VecDeque};

use crate::identity::{CallableId, ClassId};

use super::super::model::*;

pub(super) struct CleanupLivenessError {
    pub(super) block: BlockId,
    pub(super) message: &'static str,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ObjectState {
    live: HashSet<MirPlace>,
    cleaned: HashSet<MirPlace>,
    outstanding_local_cleanup: HashSet<MirPlace>,
}

pub(super) fn analyze(
    program: &MirProgram,
    function: MirDefinitionRef<'_>,
) -> Vec<CleanupLivenessError> {
    let mut analyzer = Analyzer {
        program,
        function,
        errors: Vec::new(),
    };
    analyzer.analyze();
    analyzer.errors
}

struct Analyzer<'mir> {
    program: &'mir MirProgram,
    function: MirDefinitionRef<'mir>,
    errors: Vec<CleanupLivenessError>,
}

impl Analyzer<'_> {
    fn analyze(&mut self) {
        let mut initial = ObjectState::default();
        if !matches!(self.function.callable(), CallableId::Initializer(_)) {
            if let Some(receiver) = self.function.receiver() {
                if self
                    .function
                    .storage(receiver)
                    .is_some_and(|storage| matches!(storage.ty, MirType::Class(_)))
                {
                    initial.live.insert(MirPlace::base(receiver));
                }
            }
        }

        let mut incoming = vec![None; self.function.body().blocks.len()];
        if self.function.body().entry.index() >= incoming.len() {
            return;
        }
        incoming[self.function.body().entry.index()] = Some(initial);
        let mut pending = VecDeque::from([self.function.body().entry]);

        while let Some(block_id) = pending.pop_front() {
            let Some(block) = self.function.block(block_id) else {
                continue;
            };
            let Some(mut state) = incoming[block_id.index()].clone() else {
                continue;
            };
            self.apply_block(block, &mut state);

            match &block.terminator {
                Some(MirTerminator::Goto { target, .. }) => {
                    self.merge_state(*target, &state, &mut incoming, &mut pending);
                }
                Some(MirTerminator::Branch {
                    true_target,
                    false_target,
                    ..
                }) => {
                    for target in [*true_target, *false_target] {
                        self.merge_state(target, &state, &mut incoming, &mut pending);
                    }
                }
                Some(MirTerminator::Return { .. }) => self.check_normal_return(block, &state),
                None => {}
            }
        }

        // Unreachable blocks remain structurally checked and must not be able
        // to hide an invalid cleanup operation.
        for block in &self.function.body().blocks {
            if incoming
                .get(block.id.index())
                .is_none_or(|state| state.is_none())
            {
                let mut state = ObjectState::default();
                self.apply_block(block, &mut state);
                if matches!(block.terminator, Some(MirTerminator::Return { .. })) {
                    self.check_normal_return(block, &state);
                }
            }
        }
    }

    fn check_normal_return(&mut self, block: &MirBasicBlock, state: &ObjectState) {
        if !state.outstanding_local_cleanup.is_empty() {
            self.errors.push(CleanupLivenessError {
                block: block.id,
                message: "owning local remains live on normal return",
            });
        }
    }

    fn merge_state(
        &self,
        target: BlockId,
        state: &ObjectState,
        incoming: &mut [Option<ObjectState>],
        pending: &mut VecDeque<BlockId>,
    ) {
        if target.callable() != self.function.callable() || target.index() >= incoming.len() {
            return;
        }
        match &mut incoming[target.index()] {
            None => {
                incoming[target.index()] = Some(state.clone());
                pending.push_back(target);
            }
            Some(existing) => {
                let merged = ObjectState {
                    live: existing.live.intersection(&state.live).cloned().collect(),
                    cleaned: existing.cleaned.union(&state.cleaned).cloned().collect(),
                    outstanding_local_cleanup: existing
                        .outstanding_local_cleanup
                        .union(&state.outstanding_local_cleanup)
                        .cloned()
                        .collect(),
                };
                if *existing != merged {
                    *existing = merged;
                    pending.push_back(target);
                }
            }
        }
    }

    fn apply_block(&mut self, block: &MirBasicBlock, state: &mut ObjectState) {
        for instruction in &block.instructions {
            match instruction {
                MirInstruction::Initialize(initialize)
                    if self.is_owning_class_place(
                        &initialize.destination,
                        initialize.target.class(),
                    ) =>
                {
                    state.live.insert(initialize.destination.clone());
                    if self.is_owning_local_root(&initialize.destination) {
                        state
                            .outstanding_local_cleanup
                            .insert(initialize.destination.clone());
                    }
                    state
                        .cleaned
                        .retain(|place| !places_overlap(place, &initialize.destination));
                }
                MirInstruction::Cleanup(cleanup)
                    if self.is_owning_class_place(&cleanup.destination, cleanup.target) =>
                {
                    if state
                        .cleaned
                        .iter()
                        .any(|place| places_overlap(place, &cleanup.destination))
                    {
                        self.errors.push(CleanupLivenessError {
                            block: block.id,
                            message: "cleanup destination is destroyed more than once",
                        });
                    } else if !state
                        .live
                        .iter()
                        .any(|place| place_is_ancestor(place, &cleanup.destination))
                    {
                        self.errors.push(CleanupLivenessError {
                            block: block.id,
                            message: "cleanup destination is not live",
                        });
                    } else {
                        state.cleaned.insert(cleanup.destination.clone());
                        state
                            .outstanding_local_cleanup
                            .retain(|place| !place_is_ancestor(&cleanup.destination, place));
                    }
                }
                _ => {}
            }
        }
    }

    fn is_owning_class_place(&self, place: &MirPlace, expected_class: ClassId) -> bool {
        if !matches!(place.base, MirPlaceBase::Storage(_)) {
            return false;
        }
        let Some(storage) = self.function.storage(place.base.storage()) else {
            return false;
        };
        if matches!(storage.kind, MirStorageKind::AliasParameter(_)) {
            return false;
        }
        let mut ty = storage.ty;
        for projection in &place.projections {
            let MirPlaceProjection::Field(field_id) = *projection;
            let MirType::Class(owner) = ty else {
                return false;
            };
            if field_id.class() != owner {
                return false;
            }
            let Some(field) = self.program.field(field_id) else {
                return false;
            };
            ty = field.ty;
        }
        ty == MirType::Class(expected_class)
    }

    fn is_owning_local_root(&self, place: &MirPlace) -> bool {
        place.projections.is_empty()
            && self
                .function
                .storage(place.base.storage())
                .is_some_and(|storage| storage.kind == MirStorageKind::Local)
    }
}

fn place_is_ancestor(ancestor: &MirPlace, place: &MirPlace) -> bool {
    ancestor.base == place.base && place.projections.starts_with(&ancestor.projections)
}

fn places_overlap(left: &MirPlace, right: &MirPlace) -> bool {
    place_is_ancestor(left, right) || place_is_ancestor(right, left)
}
