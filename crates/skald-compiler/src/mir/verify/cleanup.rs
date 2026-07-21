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
    outstanding_parameter_cleanup: HashSet<MirPlace>,
    live_arguments: HashSet<MirPlace>,
    live_temporaries: Vec<MirPlace>,
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
        for storage in self.function.storage_entries() {
            if !matches!(storage.ty, MirType::Class(_)) {
                continue;
            }
            let place = match storage.kind {
                MirStorageKind::Parameter => {
                    let place = MirPlace::base(storage.id);
                    initial.outstanding_parameter_cleanup.insert(place.clone());
                    place
                }
                MirStorageKind::AliasParameter(_) => MirPlace::alias_parameter(storage.id),
                MirStorageKind::Receiver
                | MirStorageKind::Local
                | MirStorageKind::Argument
                | MirStorageKind::Temporary => continue,
            };
            initial.live.insert(place);
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
        if !state.outstanding_parameter_cleanup.is_empty() {
            self.errors.push(CleanupLivenessError {
                block: block.id,
                message: "owning value parameter remains live on normal return",
            });
        }
        if !state.live_arguments.is_empty() {
            self.errors.push(CleanupLivenessError {
                block: block.id,
                message: "caller argument storage remains live without ownership transfer",
            });
        }
        if !state.live_temporaries.is_empty() {
            self.errors.push(CleanupLivenessError {
                block: block.id,
                message: "owning temporary remains live on normal return",
            });
        }
    }

    fn merge_state(
        &mut self,
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
                    outstanding_parameter_cleanup: existing
                        .outstanding_parameter_cleanup
                        .union(&state.outstanding_parameter_cleanup)
                        .cloned()
                        .collect(),
                    live_arguments: existing
                        .live_arguments
                        .union(&state.live_arguments)
                        .cloned()
                        .collect(),
                    live_temporaries: if existing.live_temporaries == state.live_temporaries {
                        existing.live_temporaries.clone()
                    } else {
                        self.errors.push(CleanupLivenessError {
                            block: target,
                            message: "owning temporary liveness differs across control-flow paths",
                        });
                        // Keep one concrete ordering so later checks remain
                        // conservative instead of silently forgetting live
                        // temporaries at the join.
                        existing.live_temporaries.clone()
                    },
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
                    self.consume_owned_arguments(block, state, &initialize.arguments);
                    self.initialize_place(block, state, &initialize.destination);
                }
                MirInstruction::CopyConstruct(copy)
                    if self.is_owning_class_place(&copy.destination, copy.class) =>
                {
                    if !self.place_is_live(state, &copy.source) {
                        self.errors.push(CleanupLivenessError {
                            block: block.id,
                            message: "copy-construction source is not live",
                        });
                    }
                    self.initialize_place(block, state, &copy.destination);
                }
                MirInstruction::CopyAssign(copy)
                    if self.is_owning_class_place(&copy.destination, copy.class) =>
                {
                    if !self.place_is_live(state, &copy.destination) {
                        self.errors.push(CleanupLivenessError {
                            block: block.id,
                            message: "copy-assignment destination is not live",
                        });
                    }
                    if !self.place_is_live(state, &copy.source) {
                        self.errors.push(CleanupLivenessError {
                            block: block.id,
                            message: "copy-assignment source is not live",
                        });
                    }
                }
                MirInstruction::Call(call) => {
                    self.consume_owned_arguments(block, state, &call.arguments);
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
                            .live
                            .retain(|place| !place_is_ancestor(&cleanup.destination, place));
                        state
                            .outstanding_local_cleanup
                            .retain(|place| !place_is_ancestor(&cleanup.destination, place));
                        state
                            .outstanding_parameter_cleanup
                            .retain(|place| !place_is_ancestor(&cleanup.destination, place));
                    }
                }
                MirInstruction::EndFullExpression(end) => {
                    let expected: Vec<_> = state.live_temporaries.iter().rev().cloned().collect();
                    let actual: Vec<_> = end
                        .temporaries
                        .iter()
                        .map(|cleanup| cleanup.destination.clone())
                        .collect();
                    if actual != expected {
                        self.errors.push(CleanupLivenessError {
                            block: block.id,
                            message: "full-expression temporaries must be cleaned in reverse completion order",
                        });
                    }
                    for place in &actual {
                        if self.place_is_live(state, place) {
                            state.live.retain(|live| !place_is_ancestor(place, live));
                            state.cleaned.insert(place.clone());
                        } else {
                            self.errors.push(CleanupLivenessError {
                                block: block.id,
                                message: "full-expression cleanup destination is not live",
                            });
                        }
                    }
                    state
                        .live_temporaries
                        .retain(|temporary| !actual.contains(temporary));
                }
                _ => {}
            }
        }
    }

    fn initialize_place(
        &mut self,
        block: &MirBasicBlock,
        state: &mut ObjectState,
        destination: &MirPlace,
    ) {
        if self.place_is_live(state, destination) {
            self.errors.push(CleanupLivenessError {
                block: block.id,
                message: "initialization destination is already live",
            });
            return;
        }
        state.live.insert(destination.clone());
        if self.is_owning_local_root(destination) {
            state.outstanding_local_cleanup.insert(destination.clone());
        }
        if self.is_argument_root(destination) {
            state.live_arguments.insert(destination.clone());
        }
        if self.is_temporary_root(destination) {
            state.live_temporaries.push(destination.clone());
        }
        state
            .cleaned
            .retain(|place| !places_overlap(place, destination));
    }

    fn consume_owned_arguments(
        &mut self,
        block: &MirBasicBlock,
        state: &mut ObjectState,
        arguments: &[MirArgument],
    ) {
        for argument in arguments {
            let MirArgument::OwnedPlace(place) = argument else {
                continue;
            };
            if !state.live_arguments.remove(place) || !self.place_is_live(state, place) {
                self.errors.push(CleanupLivenessError {
                    block: block.id,
                    message: "owned call argument is not a live caller argument",
                });
            } else {
                state.live.retain(|live| !place_is_ancestor(place, live));
            }
        }
    }

    fn place_is_live(&self, state: &ObjectState, place: &MirPlace) -> bool {
        state.live.iter().any(|live| place_is_ancestor(live, place))
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

    fn is_temporary_root(&self, place: &MirPlace) -> bool {
        place.projections.is_empty()
            && matches!(place.base, MirPlaceBase::Storage(_))
            && self
                .function
                .storage(place.base.storage())
                .is_some_and(|storage| storage.kind == MirStorageKind::Temporary)
    }

    fn is_argument_root(&self, place: &MirPlace) -> bool {
        place.projections.is_empty()
            && matches!(place.base, MirPlaceBase::Storage(_))
            && self
                .function
                .storage(place.base.storage())
                .is_some_and(|storage| storage.kind == MirStorageKind::Argument)
    }
}

fn place_is_ancestor(ancestor: &MirPlace, place: &MirPlace) -> bool {
    ancestor.base == place.base && place.projections.starts_with(&ancestor.projections)
}

fn places_overlap(left: &MirPlace, right: &MirPlace) -> bool {
    place_is_ancestor(left, right) || place_is_ancestor(right, left)
}
