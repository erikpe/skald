//! Definite object-liveness analysis for cleanup verification.

use std::collections::{HashMap, HashSet};

use crate::identity::{CallableId, ClassId};

use super::{
    super::model::{
        BlockId, MirAliasAccess, MirArgument, MirBasicBlock, MirCallReceiver, MirCleanup,
        MirDefinitionRef, MirInstruction, MirObjectOrigin, MirPlace, MirPlaceBase,
        MirPlaceProjection, MirProgram, MirStorageKind, MirTerminator, MirType,
    },
    context::Verifier,
    dataflow::ForwardDataflow,
    path_state::{condition_reads, PathStates},
    place::{is_ancestor, places_overlap},
    sink::ErrorSink,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ObjectState {
    live: HashSet<MirPlace>,
    cleaned: HashSet<MirPlace>,
    outstanding_local_cleanup: HashSet<MirPlace>,
    outstanding_parameter_cleanup: HashSet<MirPlace>,
    live_arguments: HashSet<MirPlace>,
    live_temporaries: Vec<MirPlace>,
}

impl<'mir> Verifier<'mir> {
    pub(super) fn verify_cleanup_instruction(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        cleanup: &MirCleanup,
    ) {
        let destination = self.verify_place(function, block, &cleanup.destination);
        if matches!(
            cleanup.destination.base,
            MirPlaceBase::AliasParameter(_)
                | MirPlaceBase::CheckedView(_)
                | MirPlaceBase::ArrayAlias(_)
        ) {
            self.block_error(
                function.callable(),
                block.id,
                "cleanup destination must be owning storage",
            );
        }
        if function
            .storage(cleanup.destination.base.storage())
            .is_some_and(|storage| {
                matches!(
                    storage.kind,
                    MirStorageKind::Return | MirStorageKind::Argument | MirStorageKind::Temporary
                )
            })
        {
            self.block_error(
                function.callable(),
                block.id,
                "return, caller argument, and temporary storage require their dedicated lifetime boundary",
            );
        }
        if self.program.class(cleanup.target).is_none() {
            self.block_error(
                function.callable(),
                block.id,
                format!("cleanup target {} is not declared", cleanup.target),
            );
        }
        match destination.map(|place| place.ty) {
            Some(MirType::Class(class)) if class != cleanup.target => self.block_error(
                function.callable(),
                block.id,
                "cleanup destination has the wrong class type",
            ),
            Some(MirType::Class(_)) => {}
            Some(_) => self.block_error(
                function.callable(),
                block.id,
                "cleanup destination must have class type",
            ),
            None => {}
        }
        if destination.is_some_and(|place| place.access != MirAliasAccess::Mutable) {
            self.block_error(
                function.callable(),
                block.id,
                "cleanup destination requires mutable access",
            );
        }
    }

    pub(super) fn verify_cleanup_liveness(&mut self, function: MirDefinitionRef<'mir>) {
        let mut analysis = CleanupLivenessAnalysis {
            program: self.program,
            function,
            errors: &mut self.errors,
        };
        analysis.analyze();
    }
}

#[cfg(test)]
mod tests;

struct CleanupLivenessAnalysis<'mir, 'errors> {
    program: &'mir MirProgram,
    function: MirDefinitionRef<'mir>,
    errors: &'errors mut ErrorSink,
}

impl CleanupLivenessAnalysis<'_, '_> {
    fn analyze(&mut self) {
        let mut initial = ObjectState::default();
        if !matches!(
            self.function.callable(),
            CallableId::Initializer(_) | CallableId::CopyConstructor(_)
        ) {
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
            if !matches!(
                storage.ty,
                MirType::Class(_) | MirType::Interface(_) | MirType::Obj
            ) {
                continue;
            }
            let place = match storage.kind {
                MirStorageKind::Parameter => {
                    let place = MirPlace::base(storage.id);
                    initial.outstanding_parameter_cleanup.insert(place.clone());
                    place
                }
                MirStorageKind::AliasParameter(_) => MirPlace::alias_parameter(storage.id),
                MirStorageKind::CheckedView(_) => continue,
                MirStorageKind::Receiver
                | MirStorageKind::Return
                | MirStorageKind::Local
                | MirStorageKind::Argument
                | MirStorageKind::Temporary
                | MirStorageKind::SharedAnchor
                | MirStorageKind::ScalarSpill
                | MirStorageKind::PathCondition
                | MirStorageKind::OptionalUnwrap
                | MirStorageKind::SharedAllocation
                | MirStorageKind::ArrayBacking
                | MirStorageKind::ArrayProduced
                | MirStorageKind::ArraySlice
                | MirStorageKind::ArrayPosition
                | MirStorageKind::ArrayAnchor(_)
                | MirStorageKind::ArrayAlias(_) => continue,
            };
            initial.live.insert(place);
        }

        let condition_reads = condition_reads(self.function);
        let activation_conditions: HashMap<_, _> = self
            .function
            .path_conditions()
            .iter()
            .map(|condition| (condition.activation, condition.id))
            .collect();
        let mut flow =
            ForwardDataflow::new(self.function.callable(), self.function.body().blocks.len());
        flow.seed(
            self.function.body().entry,
            PathStates::initial(initial.clone()),
        );

        loop {
            while let Some((block_id, mut states)) = flow.pop() {
                let Some(block) = self.function.block(block_id) else {
                    continue;
                };
                for instruction in &block.instructions {
                    states.update_states(|state| {
                        self.apply_instruction(block, instruction, state);
                    });
                    let MirInstruction::StorageDead(operation) = instruction else {
                        continue;
                    };
                    let Some(condition) = activation_conditions.get(&operation.storage).copied()
                    else {
                        continue;
                    };
                    let mut disagrees = false;
                    let missing = states.end_condition(condition, |existing, incoming| {
                        disagrees |= !existing.has_compatible_liveness(incoming);
                        Self::merge_object_state(existing, incoming);
                    });
                    if disagrees {
                        self.block_error(
                            block.id,
                            format!(
                                "conditional object state remains when path condition {condition} ends"
                            ),
                        );
                    }
                    if missing {
                        self.block_error(
                            block.id,
                            format!(
                                "path condition {condition} ends outside its selected object-lifetime region"
                            ),
                        );
                    }
                }

                match &block.terminator {
                    Some(MirTerminator::Goto { target, .. }) => {
                        self.merge_state(block.id, *target, &states, &mut flow);
                    }
                    Some(MirTerminator::Branch {
                        condition,
                        true_target,
                        false_target,
                        ..
                    }) => {
                        if let Some(path_condition) = condition_reads.get(condition).copied() {
                            for (target, active) in [(*true_target, true), (*false_target, false)] {
                                let (selected, _) = states.select(path_condition, active);
                                self.merge_state(block.id, target, &selected, &mut flow);
                            }
                        } else {
                            for target in [*true_target, *false_target] {
                                self.merge_state(block.id, target, &states, &mut flow);
                            }
                        }
                    }
                    Some(MirTerminator::CheckedCast {
                        binding,
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        let mut success_states = states.clone();
                        success_states.update_states(|state| {
                            self.require_live_place(
                                block,
                                state,
                                &binding.view.source,
                                "checked-cast source",
                            );
                            self.require_live_origin(
                                block,
                                state,
                                &binding.view.origin,
                                "checked-cast origin",
                            );
                            state
                                .live
                                .insert(MirPlace::checked_view(binding.destination));
                        });
                        self.merge_state(block.id, *success_target, &success_states, &mut flow);
                        self.merge_state(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::SharedCast {
                        cast,
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        states.update_states(|state| {
                            if let super::super::model::MirSharedCastSource::Field {
                                place, ..
                            } = &cast.source
                            {
                                self.require_live_place(
                                    block,
                                    state,
                                    place,
                                    "shared-cast field source",
                                );
                            }
                        });
                        self.merge_state(block.id, *success_target, &states, &mut flow);
                        self.merge_state(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::OptionalUnwrap {
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        self.merge_state(block.id, *success_target, &states, &mut flow);
                        self.merge_state(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::OptionalSharedUnwrap {
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        self.merge_state(block.id, *success_target, &states, &mut flow);
                        self.merge_state(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::BeginOptionalView {
                        success_target,
                        absent_target,
                        overflow_target,
                        ..
                    }) => {
                        for target in [*success_target, *absent_target, *overflow_target] {
                            self.merge_state(block.id, target, &states, &mut flow);
                        }
                    }
                    Some(MirTerminator::CheckOptionalMutation {
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        self.merge_state(block.id, *success_target, &states, &mut flow);
                        self.merge_state(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::ArrayPositionCheck {
                        success_target,
                        failure_target,
                        ..
                    })
                    | Some(MirTerminator::ArrayOperationCheck {
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        self.merge_state(block.id, *success_target, &states, &mut flow);
                        self.merge_state(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::ArrayLoop {
                        body_target,
                        complete_target,
                        ..
                    }) => {
                        self.merge_state(block.id, *body_target, &states, &mut flow);
                        self.merge_state(block.id, *complete_target, &states, &mut flow);
                    }
                    Some(MirTerminator::Return { .. })
                    | Some(MirTerminator::ReturnShared { .. })
                    | Some(MirTerminator::ReturnOptionalShared { .. }) => {
                        states.update_states(|state| {
                            self.check_normal_return(block, state);
                        });
                    }
                    Some(MirTerminator::Panic { message, .. }) => {
                        states.update_states(|state| {
                            self.require_live_place(block, state, message, "panic message");
                        });
                    }
                    Some(MirTerminator::Terminate { .. }) => {}
                    None => {}
                }
            }
            if !flow.seed_next_component(
                &self.function.body().blocks,
                PathStates::initial(initial.clone()),
            ) {
                break;
            }
        }
    }

    fn check_normal_return(&mut self, block: &MirBasicBlock, state: &ObjectState) {
        if let Some(return_storage) = self.function.return_storage() {
            if self
                .function
                .storage(return_storage)
                .is_some_and(|storage| matches!(storage.ty, MirType::Class(_)))
                && !self.place_is_live(state, &MirPlace::base(return_storage))
            {
                self.block_error(
                    block.id,
                    "object return storage is not initialized on normal return",
                );
            }
        }
        if !state.outstanding_local_cleanup.is_empty() {
            self.block_error(block.id, "owning local remains live on normal return");
        }
        if !state.outstanding_parameter_cleanup.is_empty() {
            self.block_error(
                block.id,
                "owning value parameter remains live on normal return",
            );
        }
        if !state.live_arguments.is_empty() {
            self.block_error(
                block.id,
                "caller argument storage remains live without ownership transfer",
            );
        }
        if !state.live_temporaries.is_empty() {
            self.block_error(block.id, "owning temporary remains live on normal return");
        }
    }

    fn merge_state(
        &mut self,
        predecessor: BlockId,
        target: BlockId,
        states: &PathStates<ObjectState>,
        flow: &mut ForwardDataflow<PathStates<ObjectState>>,
    ) {
        if states.is_empty() {
            return;
        }
        let selected = states
            .on_edge(self.function, predecessor, target)
            .unwrap_or_else(|_| states.clone());
        flow.merge(target, &selected, |existing, incoming| {
            existing.merge(incoming, |existing, incoming| {
                if existing.live_temporaries != incoming.live_temporaries {
                    self.block_error(
                        target,
                        "owning temporary liveness differs across control-flow paths",
                    );
                }
                Self::merge_object_state(existing, incoming);
            })
        });
    }

    fn merge_object_state(existing: &mut ObjectState, incoming: &ObjectState) {
        existing.live = existing
            .live
            .intersection(&incoming.live)
            .cloned()
            .collect();
        existing.cleaned = existing.cleaned.union(&incoming.cleaned).cloned().collect();
        existing.outstanding_local_cleanup = existing
            .outstanding_local_cleanup
            .union(&incoming.outstanding_local_cleanup)
            .cloned()
            .collect();
        existing.outstanding_parameter_cleanup = existing
            .outstanding_parameter_cleanup
            .union(&incoming.outstanding_parameter_cleanup)
            .cloned()
            .collect();
        existing.live_arguments = existing
            .live_arguments
            .union(&incoming.live_arguments)
            .cloned()
            .collect();
    }

    fn apply_instruction(
        &mut self,
        block: &MirBasicBlock,
        instruction: &MirInstruction,
        state: &mut ObjectState,
    ) {
        match instruction {
            MirInstruction::StorageLive(operation) => {
                state.reset_storage(operation.storage);
            }
            MirInstruction::StorageDead(operation) => {
                if self
                    .function
                    .storage(operation.storage)
                    .is_some_and(|storage| matches!(storage.kind, MirStorageKind::CheckedView(_)))
                    && state.has_live_place(operation.storage)
                {
                    self.block_error(
                        block.id,
                        "checked-view carrier remains active at storage-dead",
                    );
                }
                if state.has_outstanding_local(operation.storage) {
                    self.block_error(block.id, "owning local remains live on normal return");
                }
                if state.has_outstanding_parameter(operation.storage) {
                    self.block_error(
                        block.id,
                        "owning value parameter remains live on normal return",
                    );
                }
                if state.has_live_argument(operation.storage) {
                    self.block_error(
                        block.id,
                        "caller argument storage remains live without ownership transfer",
                    );
                }
                if state.has_live_temporary(operation.storage) {
                    self.block_error(block.id, "owning temporary remains live on normal return");
                }
                state.finish_storage_epoch(operation.storage);
            }
            MirInstruction::Initialize(initialize)
                if self
                    .is_owning_class_place(&initialize.destination, initialize.target.class()) =>
            {
                self.check_borrowed_arguments(block, state, &initialize.arguments);
                self.consume_owned_arguments(block, state, &initialize.arguments);
                self.initialize_place(block, state, &initialize.destination);
            }
            MirInstruction::SharedInitialize(initialize) => {
                self.check_borrowed_arguments(block, state, &initialize.arguments);
                self.consume_owned_arguments(block, state, &initialize.arguments);
            }
            MirInstruction::SharedAllocate(allocation) => {
                if let super::super::model::MirSharedAllocationMode::Copy { source } =
                    &allocation.mode
                {
                    self.require_live_place(block, state, source, "shared copy-allocation source");
                }
            }
            MirInstruction::SharedFieldCopy(copy) => {
                self.require_live_place(block, state, &copy.source, "shared field copy source");
            }
            MirInstruction::SharedCast(cast) => {
                if let super::super::model::MirSharedCastSource::Field { place, .. } = &cast.source
                {
                    self.require_live_place(block, state, place, "shared-cast field source");
                }
            }
            MirInstruction::SharedFieldInitialize(initialize) => {
                self.initialize_place(block, state, &initialize.destination);
            }
            MirInstruction::SharedFieldReplace(replace) => {
                self.require_live_place(
                    block,
                    state,
                    &replace.destination,
                    "shared field replacement destination",
                );
            }
            MirInstruction::StringInitialize(initialize) => {
                self.initialize_place(block, state, &initialize.destination);
            }
            MirInstruction::CopyConstruct(copy)
                if matches!(
                    copy.destination.base,
                    super::super::model::MirPlaceBase::SharedAllocationPayload(_)
                ) =>
            {
                self.require_live_place(
                    block,
                    state,
                    &copy.source,
                    "shared copy-construction source",
                );
            }
            MirInstruction::CopyConstruct(copy)
                if self.is_owning_class_place(&copy.destination, copy.class) =>
            {
                if !self.place_is_live(state, &copy.source) {
                    self.block_error(block.id, "copy-construction source is not live");
                }
                self.initialize_place(block, state, &copy.destination);
            }
            MirInstruction::CopyAssign(copy)
                if self.is_owning_class_place(&copy.destination, copy.class) =>
            {
                if !self.place_is_live(state, &copy.destination) {
                    self.block_error(block.id, "copy-assignment destination is not live");
                }
                if !self.place_is_live(state, &copy.source) {
                    self.block_error(block.id, "copy-assignment source is not live");
                }
            }
            MirInstruction::Call(call) => {
                if let Some(receiver) = &call.receiver {
                    match receiver {
                        MirCallReceiver::Method(receiver) => {
                            self.require_live_place(
                                block,
                                state,
                                &receiver.place,
                                "method receiver",
                            );
                            self.require_live_origin(
                                block,
                                state,
                                &receiver.origin,
                                "method receiver origin",
                            );
                        }
                        MirCallReceiver::Interface(receiver) => {
                            self.require_live_place(
                                block,
                                state,
                                &receiver.source,
                                "interface receiver",
                            );
                            self.require_live_origin(
                                block,
                                state,
                                &receiver.origin,
                                "interface receiver origin",
                            );
                        }
                    }
                }
                self.check_borrowed_arguments(block, state, &call.arguments);
                self.consume_owned_arguments(block, state, &call.arguments);
                if let Some(destination) = &call.destination {
                    if matches!(self.place_type(destination), Some(MirType::Class(_))) {
                        self.initialize_place(block, state, destination);
                    }
                }
            }
            MirInstruction::Cleanup(cleanup)
                if self.is_owning_class_place(&cleanup.destination, cleanup.target) =>
            {
                if state
                    .cleaned
                    .iter()
                    .any(|place| places_overlap(place, &cleanup.destination))
                {
                    self.block_error(block.id, "cleanup destination is destroyed more than once");
                } else if !state
                    .live
                    .iter()
                    .any(|place| is_ancestor(place, &cleanup.destination))
                {
                    self.block_error(block.id, "cleanup destination is not live");
                } else {
                    state.cleaned.insert(cleanup.destination.clone());
                    state
                        .live
                        .retain(|place| !is_ancestor(&cleanup.destination, place));
                    state
                        .outstanding_local_cleanup
                        .retain(|place| !is_ancestor(&cleanup.destination, place));
                    state
                        .outstanding_parameter_cleanup
                        .retain(|place| !is_ancestor(&cleanup.destination, place));
                }
            }
            MirInstruction::EndFullExpression(end) => {
                let expected: Vec<_> = state.live_temporaries.iter().rev().cloned().collect();
                let actual: Vec<_> = end
                    .temporaries
                    .iter()
                    .map(|cleanup| cleanup.destination.clone())
                    .collect();
                if (actual.is_empty() && !expected.is_empty()) || !expected.starts_with(&actual) {
                    self.block_error(
                        block.id,
                        "full-expression temporaries must be cleaned in reverse completion order",
                    );
                }
                for place in &actual {
                    if self.place_is_live(state, place) {
                        state.live.retain(|live| !is_ancestor(place, live));
                        state.cleaned.insert(place.clone());
                    } else {
                        self.block_error(
                            block.id,
                            "full-expression cleanup destination is not live",
                        );
                    }
                }
                state
                    .live_temporaries
                    .retain(|temporary| !actual.contains(temporary));
            }
            MirInstruction::Assign(assignment) => match &assignment.rvalue.kind {
                super::super::model::MirRvalueKind::Load(place) => {
                    self.require_indirect_carrier_live(block, state, place, "load source");
                }
                super::super::model::MirRvalueKind::TypeTest { source, .. } => {
                    self.require_live_place(block, state, &source.source, "type-test source");
                    self.require_live_origin(block, state, &source.origin, "type-test origin");
                }
                _ => {}
            },
            MirInstruction::Store(store) => self.require_indirect_carrier_live(
                block,
                state,
                &store.destination,
                "store destination",
            ),
            MirInstruction::BindCheckedView(binding) => {
                self.require_live_place(block, state, &binding.view.source, "checked-cast source");
                self.require_live_origin(block, state, &binding.view.origin, "checked-cast origin");
                let carrier = MirPlace::checked_view(binding.destination);
                if self.place_is_live(state, &carrier) {
                    self.block_error(block.id, "checked-view carrier is already live");
                } else {
                    state.live.insert(carrier);
                }
            }
            MirInstruction::EndCheckedView(end) => {
                let carrier = MirPlace::checked_view(end.carrier);
                if !self.place_is_live(state, &carrier) {
                    self.block_error(
                        block.id,
                        "checked-view carrier is not live at full-expression end",
                    );
                }
                state.live.remove(&carrier);
            }
            _ => {}
        }
    }

    fn initialize_place(
        &mut self,
        block: &MirBasicBlock,
        state: &mut ObjectState,
        destination: &MirPlace,
    ) {
        if matches!(destination.base, MirPlaceBase::SharedPointee(_)) {
            return;
        }
        if self.place_is_live(state, destination) {
            self.block_error(block.id, "initialization destination is already live");
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
            if !self
                .function
                .storage(place.base.storage())
                .is_some_and(|storage| matches!(storage.ty, MirType::Class(_)))
            {
                continue;
            }
            if !state.live_arguments.remove(place) || !self.place_is_live(state, place) {
                self.block_error(
                    block.id,
                    "owned call argument is not a live caller argument",
                );
            } else {
                state.live.retain(|live| !is_ancestor(place, live));
            }
        }
    }

    fn check_borrowed_arguments(
        &mut self,
        block: &MirBasicBlock,
        state: &ObjectState,
        arguments: &[MirArgument],
    ) {
        for argument in arguments {
            let view = match argument {
                MirArgument::View(view) => view,
                MirArgument::Value(_)
                | MirArgument::Place(_)
                | MirArgument::OwnedPlace(_)
                | MirArgument::SharedOwner(_) => continue,
            };
            self.require_live_place(block, state, &view.source, "object view source");
            self.require_live_origin(block, state, &view.origin, "object view origin");
        }
    }

    fn require_live_origin(
        &mut self,
        block: &MirBasicBlock,
        state: &ObjectState,
        origin: &MirObjectOrigin,
        kind: &str,
    ) {
        let place = match origin {
            MirObjectOrigin::Exact { complete, .. } => complete.clone(),
            MirObjectOrigin::Forwarded { carrier, .. } => {
                let Some(storage) = self.function.storage(*carrier) else {
                    return;
                };
                match storage.kind {
                    MirStorageKind::Receiver => MirPlace::base(*carrier),
                    MirStorageKind::AliasParameter(_) => MirPlace::alias_parameter(*carrier),
                    MirStorageKind::CheckedView(_) => MirPlace::checked_view(*carrier),
                    _ => return,
                }
            }
            MirObjectOrigin::Shared { .. } => return,
        };
        self.require_live_place(block, state, &place, kind);
    }

    fn require_live_place(
        &mut self,
        block: &MirBasicBlock,
        state: &ObjectState,
        place: &MirPlace,
        kind: &str,
    ) {
        if place
            .projections
            .iter()
            .any(|projection| matches!(projection, MirPlaceProjection::ArrayElement { .. }))
        {
            return;
        }
        if matches!(place.base, MirPlaceBase::SharedPointee(_)) {
            // Shared-owner liveness is path-sensitive in the ownership
            // verifier; inline-object cleanup state deliberately does not
            // duplicate it.
            return;
        }
        if place
            .projections
            .iter()
            .any(|projection| matches!(projection, MirPlaceProjection::OptionalPayload(_)))
        {
            // Optional-payload liveness is established by BeginOptionalView
            // and checked, including balanced nested guards, by the dedicated
            // optional verifier. It is dynamic presence rather than ordinary
            // statically initialized object storage.
            return;
        }
        if !self.place_is_live(state, place) {
            self.block_error(block.id, format!("{kind} is not live"));
        }
    }

    fn require_indirect_carrier_live(
        &mut self,
        block: &MirBasicBlock,
        state: &ObjectState,
        place: &MirPlace,
        kind: &str,
    ) {
        if matches!(
            place.base,
            MirPlaceBase::CheckedView(_) | MirPlaceBase::ArrayAlias(_)
        ) {
            self.require_live_place(block, state, place, kind);
        }
    }

    fn place_is_live(&self, state: &ObjectState, place: &MirPlace) -> bool {
        if matches!(place.base, MirPlaceBase::ArrayAlias(_)) {
            // The array ownership verifier proves that a captured alias has
            // one compatible live backing or owner anchor at every consumer.
            return true;
        }
        if place
            .projections
            .iter()
            .any(|projection| matches!(projection, MirPlaceProjection::ArrayElement { .. }))
        {
            // Array construction and initialized-prefix state are verified by
            // the dedicated array ownership analysis. Every element reached
            // through a checked position belongs to an already initialized
            // array; dynamic element positions cannot be represented in this
            // static set of object places.
            return true;
        }
        state.live.iter().any(|live| is_ancestor(live, place))
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
        self.place_type(place) == Some(MirType::Class(expected_class))
    }

    fn place_type(&self, place: &MirPlace) -> Option<MirType> {
        let mut ty = self.function.storage(place.base.storage())?.ty;
        for projection in &place.projections {
            match *projection {
                MirPlaceProjection::Base(base) => {
                    let MirType::Class(owner) = ty else {
                        return None;
                    };
                    if self.program.direct_base(owner) != Some(base) {
                        return None;
                    }
                    ty = MirType::Class(base);
                }
                MirPlaceProjection::Field(field_id) => {
                    let MirType::Class(owner) = ty else {
                        return None;
                    };
                    if field_id.class() != owner {
                        return None;
                    }
                    let field = self.program.field(field_id)?;
                    ty = field.ty;
                }
                MirPlaceProjection::OptionalPayload(class) => {
                    if ty != MirType::OptionalClass(class) {
                        return None;
                    }
                    ty = MirType::Class(class);
                }
                MirPlaceProjection::ArrayElement { array, .. } => {
                    if ty != MirType::Array(array) {
                        return None;
                    }
                    ty = self.program.array_type(array)?.element;
                }
            }
        }
        Some(ty)
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

    fn block_error(&mut self, block: BlockId, message: impl Into<String>) {
        self.errors.block(self.function.callable(), block, message);
    }
}

impl ObjectState {
    fn has_compatible_liveness(&self, other: &Self) -> bool {
        self.live == other.live
            && self.outstanding_local_cleanup == other.outstanding_local_cleanup
            && self.outstanding_parameter_cleanup == other.outstanding_parameter_cleanup
            && self.live_arguments == other.live_arguments
            && self.live_temporaries == other.live_temporaries
    }

    fn has_live_place(&self, storage: crate::mir::StorageId) -> bool {
        contains_storage(&self.live, storage)
    }

    fn has_outstanding_local(&self, storage: crate::mir::StorageId) -> bool {
        contains_storage(&self.outstanding_local_cleanup, storage)
    }

    fn has_outstanding_parameter(&self, storage: crate::mir::StorageId) -> bool {
        contains_storage(&self.outstanding_parameter_cleanup, storage)
    }

    fn has_live_argument(&self, storage: crate::mir::StorageId) -> bool {
        contains_storage(&self.live_arguments, storage)
    }

    fn has_live_temporary(&self, storage: crate::mir::StorageId) -> bool {
        self.live_temporaries
            .iter()
            .any(|place| place.base.storage() == storage)
    }

    fn reset_storage(&mut self, storage: crate::mir::StorageId) {
        self.finish_storage_epoch(storage);
        self.cleaned.retain(|place| place.base.storage() != storage);
    }

    fn finish_storage_epoch(&mut self, storage: crate::mir::StorageId) {
        self.live.retain(|place| place.base.storage() != storage);
        self.outstanding_local_cleanup
            .retain(|place| place.base.storage() != storage);
        self.outstanding_parameter_cleanup
            .retain(|place| place.base.storage() != storage);
        self.live_arguments
            .retain(|place| place.base.storage() != storage);
        self.live_temporaries
            .retain(|place| place.base.storage() != storage);
    }
}

fn contains_storage(places: &HashSet<MirPlace>, storage: crate::mir::StorageId) -> bool {
    places.iter().any(|place| place.base.storage() == storage)
}
