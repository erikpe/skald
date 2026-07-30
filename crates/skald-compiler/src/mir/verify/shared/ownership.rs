//! Path-sensitive allocation, owner, checked-view, and anchor verification.

use std::collections::{HashMap, HashSet};

use crate::identity::{CallableId, LiteralDataId};

use super::super::{
    super::model::{
        BlockId, MirArgument, MirArrayInstruction, MirBasicBlock, MirCallReceiver,
        MirCheckedViewBinding, MirDefinitionRef, MirInstruction, MirObjectOrigin, MirPlace,
        MirPlaceBase, MirRvalueKind, MirSharedAllocationMode, MirSharedCast, MirSharedCastSource,
        MirSharedCastTransfer, MirStorageKind, MirTerminator, MirType, StorageId,
    },
    context::Verifier,
    dataflow::ForwardDataflow,
    path_state::{condition_reads, PathStates},
};

impl<'mir> Verifier<'mir> {
    pub(in crate::mir::verify) fn verify_shared_ownership(
        &mut self,
        function: MirDefinitionRef<'mir>,
    ) {
        SharedOwnershipAnalysis::new(function, self).analyze();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum AllocationState {
    Allocated(MirSharedAllocationMode),
    Initialized,
    Published,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SharedState {
    allocations: HashMap<StorageId, AllocationState>,
    live_owners: HashSet<StorageId>,
    owner_origins: HashMap<StorageId, StorageId>,
    /// Static literal owners may only be consumed by exact string publication.
    static_owners: HashMap<StorageId, LiteralDataId>,
    released_owners: HashSet<StorageId>,
    /// Checked-view carrier to the shared owner that keeps its payload live.
    active_checked_views: HashMap<StorageId, StorageId>,
    initialized_fields: HashSet<MirPlace>,
    pending_full_expression_boundary: bool,
}

struct SharedOwnershipAnalysis<'mir, 'verifier> {
    function: MirDefinitionRef<'mir>,
    verifier: &'verifier mut Verifier<'mir>,
    reported_joins: HashSet<BlockId>,
}

impl<'mir, 'verifier> SharedOwnershipAnalysis<'mir, 'verifier> {
    fn new(function: MirDefinitionRef<'mir>, verifier: &'verifier mut Verifier<'mir>) -> Self {
        Self {
            function,
            verifier,
            reported_joins: HashSet::new(),
        }
    }

    fn analyze(&mut self) {
        let mut initial = SharedState::default();
        for parameter in self.function.parameters() {
            if self
                .function
                .storage(*parameter)
                .is_some_and(|storage| matches!(storage.ty, MirType::Shared(_)))
            {
                initial.live_owners.insert(*parameter);
                initial.owner_origins.insert(*parameter, *parameter);
            }
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
                for state in states.states_mut() {
                    self.apply_block(block, state);
                }
                self.end_conditions_at_storage_death(block, &activation_conditions, &mut states);
                match &block.terminator {
                    Some(MirTerminator::Goto { target, .. }) => {
                        self.merge(block.id, *target, &states, &mut flow)
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
                                self.merge(block.id, target, &selected, &mut flow);
                            }
                        } else {
                            self.merge(block.id, *true_target, &states, &mut flow);
                            self.merge(block.id, *false_target, &states, &mut flow);
                        }
                    }
                    Some(MirTerminator::CheckedCast {
                        binding,
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        let mut success = states.clone();
                        for state in success.states_mut() {
                            self.require_live_pointee(block.id, state, &binding.view.source);
                            self.require_live_shared_origin(block.id, state, &binding.view.origin);
                            self.begin_checked_view(block.id, state, binding);
                        }
                        self.merge(block.id, *success_target, &success, &mut flow);
                        self.merge(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::SharedCast {
                        cast,
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        let mut success = states.clone();
                        for state in success.states_mut() {
                            self.require_shared_cast_source(block.id, state, cast);
                            self.apply_shared_cast(block.id, state, cast);
                        }
                        self.merge(block.id, *success_target, &success, &mut flow);
                        self.merge(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::OptionalUnwrap {
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        self.merge(block.id, *success_target, &states, &mut flow);
                        self.merge(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::OptionalSharedUnwrap {
                        unwrap,
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        let mut success = states.clone();
                        for state in success.states_mut() {
                            if state.live_owners.contains(&unwrap.destination)
                                || state.released_owners.contains(&unwrap.destination)
                            {
                                self.error(
                                    block.id,
                                    "optional shared unwrap destination is already initialized",
                                );
                            } else {
                                state.live_owners.insert(unwrap.destination);
                                state
                                    .owner_origins
                                    .insert(unwrap.destination, unwrap.destination);
                                state.pending_full_expression_boundary = true;
                            }
                        }
                        self.merge(block.id, *success_target, &success, &mut flow);
                        self.merge(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::BeginOptionalView {
                        success_target,
                        absent_target,
                        overflow_target,
                        ..
                    }) => {
                        self.merge(block.id, *success_target, &states, &mut flow);
                        self.merge(block.id, *absent_target, &states, &mut flow);
                        self.merge(block.id, *overflow_target, &states, &mut flow);
                    }
                    Some(MirTerminator::CheckOptionalMutation {
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        self.merge(block.id, *success_target, &states, &mut flow);
                        self.merge(block.id, *failure_target, &states, &mut flow);
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
                        self.merge(block.id, *success_target, &states, &mut flow);
                        self.merge(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::ArrayLoop {
                        body_target,
                        complete_target,
                        ..
                    }) => {
                        self.merge(block.id, *body_target, &states, &mut flow);
                        self.merge(block.id, *complete_target, &states, &mut flow);
                    }
                    Some(MirTerminator::Return { .. }) => {
                        for state in states.states_mut() {
                            self.check_return(block, state, None);
                        }
                    }
                    Some(MirTerminator::ReturnShared { owner, .. }) => {
                        for state in states.states_mut() {
                            self.check_return(block, state, Some(*owner));
                        }
                    }
                    Some(MirTerminator::ReturnOptionalShared { .. }) => {
                        for state in states.states_mut() {
                            self.check_return(block, state, None);
                        }
                    }
                    Some(MirTerminator::Panic { .. })
                    | Some(MirTerminator::Terminate { .. })
                    | None => {}
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

    fn apply_block(&mut self, block: &MirBasicBlock, state: &mut SharedState) {
        for instruction in &block.instructions {
            self.check_pointee_uses(block.id, state, instruction);
            match instruction {
                MirInstruction::StorageLive(operation) => {
                    state.reset_storage(operation.storage);
                }
                MirInstruction::StorageDead(operation) => {
                    if state.allocations.contains_key(&operation.storage) {
                        self.error(
                            block.id,
                            "shared allocation is not published and adopted on normal return",
                        );
                    }
                    if state.live_owners.contains(&operation.storage) {
                        self.error(block.id, "shared owner remains live on normal return");
                    }
                    if state.static_owners.contains_key(&operation.storage) {
                        self.error(
                            block.id,
                            "static literal owner is not consumed by string initialization",
                        );
                    }
                    if state.active_checked_views.contains_key(&operation.storage)
                        || state
                            .active_checked_views
                            .values()
                            .any(|owner| *owner == operation.storage)
                    {
                        self.error(
                            block.id,
                            "shared-backed checked view remains live on normal return",
                        );
                    }
                    state.reset_storage(operation.storage);
                }
                MirInstruction::SharedAllocate(allocation) => {
                    if let MirSharedAllocationMode::Copy { source } = &allocation.mode {
                        self.require_live_pointee(block.id, state, source);
                    }
                    if state
                        .allocations
                        .insert(
                            allocation.allocation,
                            AllocationState::Allocated(allocation.mode.clone()),
                        )
                        .is_some()
                    {
                        self.error(
                            block.id,
                            "shared allocation storage is allocated more than once",
                        );
                    }
                }
                MirInstruction::SharedInitialize(initialize) => {
                    self.transfer_call_arguments(block.id, state, &initialize.arguments);
                    self.transition(
                        block.id,
                        state,
                        initialize.allocation,
                        AllocationState::Allocated(MirSharedAllocationMode::Initialize),
                        AllocationState::Initialized,
                        "shared initialization requires unpublished allocated storage",
                    );
                }
                MirInstruction::CopyConstruct(copy) => {
                    if let MirPlaceBase::SharedAllocationPayload(allocation) = copy.destination.base
                    {
                        self.require_live_pointee(block.id, state, &copy.source);
                        let expected = AllocationState::Allocated(MirSharedAllocationMode::Copy {
                            source: copy.source.clone(),
                        });
                        self.transition(
                            block.id,
                            state,
                            allocation,
                            expected,
                            AllocationState::Initialized,
                            "shared copy allocation requires its established source and one selected copy construction",
                        );
                    }
                }
                MirInstruction::SharedPublish(publish) => {
                    self.transition(
                        block.id,
                        state,
                        publish.allocation,
                        AllocationState::Initialized,
                        AllocationState::Published,
                        "shared publication requires completed initialization",
                    );
                }
                MirInstruction::SharedStatic(static_owner) => {
                    if state.live_owners.contains(&static_owner.destination)
                        || state.released_owners.contains(&static_owner.destination)
                    {
                        self.error(
                            block.id,
                            "static shared owner destination is already initialized",
                        );
                    } else {
                        state.live_owners.insert(static_owner.destination);
                        state
                            .owner_origins
                            .insert(static_owner.destination, static_owner.destination);
                        state
                            .static_owners
                            .insert(static_owner.destination, static_owner.data);
                    }
                    state.pending_full_expression_boundary = true;
                }
                MirInstruction::SharedAdopt(adopt) => {
                    let produced = if state.allocations.get(&adopt.allocation)
                        == Some(&AllocationState::Published)
                    {
                        state.allocations.remove(&adopt.allocation);
                        true
                    } else {
                        self.error(
                            block.id,
                            "shared adoption requires one published produced owner",
                        );
                        false
                    };
                    if state.live_owners.contains(&adopt.destination)
                        || state.released_owners.contains(&adopt.destination)
                    {
                        self.error(
                            block.id,
                            "shared owner storage is initialized more than once",
                        );
                    } else if produced {
                        state.live_owners.insert(adopt.destination);
                        state
                            .owner_origins
                            .insert(adopt.destination, adopt.allocation);
                    }
                    state.pending_full_expression_boundary = produced;
                }
                MirInstruction::SharedCopy(copy) => {
                    self.reject_static_owner(block.id, state, copy.source, "shared copy");
                    let source_origin = state.owner_origins.get(&copy.source).copied();
                    if source_origin.is_none() || !state.live_owners.contains(&copy.source) {
                        self.error(block.id, "shared copy source is not a live owner");
                    }
                    if state.live_owners.contains(&copy.destination)
                        || state.released_owners.contains(&copy.destination)
                    {
                        self.error(block.id, "shared copy destination is already initialized");
                    } else if let Some(origin) = source_origin {
                        state.live_owners.insert(copy.destination);
                        state.owner_origins.insert(copy.destination, origin);
                    }
                    state.pending_full_expression_boundary = source_origin.is_some();
                }
                MirInstruction::SharedFieldCopy(copy) => {
                    if state.live_owners.contains(&copy.destination)
                        || state.released_owners.contains(&copy.destination)
                    {
                        self.error(block.id, "shared copy destination is already initialized");
                    } else {
                        state.live_owners.insert(copy.destination);
                        state
                            .owner_origins
                            .insert(copy.destination, copy.destination);
                    }
                    state.pending_full_expression_boundary = true;
                }
                MirInstruction::SharedCast(cast) => {
                    self.require_shared_cast_source(block.id, state, cast);
                    self.apply_shared_cast(block.id, state, cast);
                }
                MirInstruction::SharedMove(transfer) => {
                    self.reject_static_owner(block.id, state, transfer.source, "shared move");
                    let source_origin = state.owner_origins.get(&transfer.source).copied();
                    if source_origin.is_none() || !state.live_owners.remove(&transfer.source) {
                        self.error(block.id, "shared move source is not a live owner");
                    } else {
                        state.owner_origins.remove(&transfer.source);
                        state.released_owners.insert(transfer.source);
                    }
                    if state.live_owners.contains(&transfer.destination) {
                        self.error(block.id, "shared move destination is still live");
                    } else if !state.released_owners.remove(&transfer.destination) {
                        self.error(
                            block.id,
                            "shared move destination was not released before replacement",
                        );
                    } else if let Some(origin) = source_origin {
                        state.live_owners.insert(transfer.destination);
                        state.owner_origins.insert(transfer.destination, origin);
                    }
                }
                MirInstruction::SharedRelease(release) => {
                    self.reject_static_owner(block.id, state, release.owner, "shared release");
                    if state
                        .active_checked_views
                        .values()
                        .any(|owner| *owner == release.owner)
                    {
                        self.error(
                            block.id,
                            "shared owner is released before its checked view ends",
                        );
                    }
                    if !state.live_owners.remove(&release.owner) {
                        self.error(block.id, "shared owner is released without being live");
                    } else {
                        state.owner_origins.remove(&release.owner);
                        state.released_owners.insert(release.owner);
                    }
                }
                MirInstruction::SharedFieldInitialize(initialize) => {
                    self.consume_field_transfer_source(block.id, state, initialize.source);
                    if !state
                        .initialized_fields
                        .insert(initialize.destination.clone())
                    {
                        self.error(block.id, "shared field is initialized more than once");
                    }
                }
                MirInstruction::SharedFieldReplace(replace) => {
                    self.consume_field_transfer_source(block.id, state, replace.source);
                }
                MirInstruction::StringInitialize(initialize) => {
                    if state.static_owners.get(&initialize.backing) != Some(&initialize.data) {
                        self.error(
                            block.id,
                            "string initialization requires its exact live static literal owner",
                        );
                    } else {
                        state.static_owners.remove(&initialize.backing);
                        state.live_owners.remove(&initialize.backing);
                        state.owner_origins.remove(&initialize.backing);
                        state.released_owners.insert(initialize.backing);
                    }
                    state.pending_full_expression_boundary = true;
                }
                MirInstruction::OptionalSharedInitialize(initialize) => {
                    if let crate::mir::MirOptionalSharedSource::Present(owner) = initialize.source {
                        self.consume_optional_shared_source(block.id, state, owner);
                    }
                    if !initialize.destination.projections.is_empty()
                        && !state
                            .initialized_fields
                            .insert(initialize.destination.clone())
                    {
                        self.error(
                            block.id,
                            "optional shared field is initialized more than once",
                        );
                    }
                }
                MirInstruction::OptionalSharedAssign(assignment) => {
                    if let crate::mir::MirOptionalSharedSource::Present(owner) = assignment.source {
                        self.consume_optional_shared_source(block.id, state, owner);
                    }
                }
                MirInstruction::OptionalSharedCleanup(_) => {}
                MirInstruction::EndFullExpression(_) => {
                    if state.live_owners.iter().any(|owner| {
                        self.function
                            .storage(*owner)
                            .is_some_and(|storage| storage.kind == MirStorageKind::Temporary)
                    }) {
                        self.error(
                            block.id,
                            "shared temporary remains live at full-expression boundary",
                        );
                    }
                    let owners_remain = state.live_owners.iter().any(|owner| {
                        self.function.storage(*owner).is_some_and(|storage| {
                            matches!(
                                storage.kind,
                                MirStorageKind::Temporary | MirStorageKind::SharedAnchor
                            )
                        })
                    });
                    state.pending_full_expression_boundary = owners_remain;
                }
                MirInstruction::Call(call) => {
                    let transferred =
                        self.transfer_call_arguments(block.id, state, &call.arguments);
                    if let Some(result) = call.shared_result {
                        if self
                            .function
                            .storage(result)
                            .is_some_and(|storage| matches!(storage.ty, MirType::OptionalShared(_)))
                        {
                            // Optional-owner initialization is verified by the
                            // optional definite-initialization analysis.
                        } else if state.live_owners.contains(&result)
                            || state.released_owners.contains(&result)
                        {
                            self.error(
                                block.id,
                                "shared call result storage is already initialized",
                            );
                        } else {
                            state.live_owners.insert(result);
                            state.owner_origins.insert(result, result);
                        }
                    }
                    state.pending_full_expression_boundary |=
                        transferred || call.shared_result.is_some();
                }
                MirInstruction::Assign(_)
                | MirInstruction::Cleanup(_)
                | MirInstruction::Store(_)
                | MirInstruction::CopyAssign(_)
                | MirInstruction::OptionalInitialize(_)
                | MirInstruction::OptionalAssign(_)
                | MirInstruction::ClassOptionalInitialize(_)
                | MirInstruction::ClassOptionalAssign(_)
                | MirInstruction::ClassOptionalPublish(_)
                | MirInstruction::ClassOptionalCleanup(_)
                | MirInstruction::EndOptionalView(_) => {}
                MirInstruction::Array(MirArrayInstruction::PublishShared {
                    destination, ..
                }) => {
                    if !state.live_owners.insert(*destination) {
                        self.error(block.id, "shared array owner is published more than once");
                    }
                    state.owner_origins.insert(*destination, *destination);
                }
                MirInstruction::Array(_) => {}
                MirInstruction::BindCheckedView(binding) => {
                    self.require_live_pointee(block.id, state, &binding.view.source);
                    self.require_live_shared_origin(block.id, state, &binding.view.origin);
                    self.begin_checked_view(block.id, state, binding);
                }
                MirInstruction::EndCheckedView(end) => {
                    state.active_checked_views.remove(&end.carrier);
                }
                MirInstruction::Initialize(initialize) => {
                    let transferred =
                        self.transfer_call_arguments(block.id, state, &initialize.arguments);
                    state.pending_full_expression_boundary |= transferred;
                }
            }
        }
    }

    fn require_shared_cast_source(
        &mut self,
        block: BlockId,
        state: &SharedState,
        cast: &MirSharedCast,
    ) {
        match &cast.source {
            MirSharedCastSource::Owner { storage, .. } => {
                self.reject_static_owner(block, state, *storage, "shared cast");
                if !state.live_owners.contains(storage)
                    || !state.owner_origins.contains_key(storage)
                {
                    self.error(block, "shared cast source is not a live owner");
                }
                if let Some(class) = cast.exact_dynamic_class {
                    let exact_origin = state
                        .owner_origins
                        .get(storage)
                        .and_then(|origin| self.function.storage(*origin))
                        .is_some_and(|origin| origin.ty == MirType::Class(class));
                    if !exact_origin {
                        self.error(
                            block,
                            "shared cast exact dynamic provenance does not match its allocation",
                        );
                    }
                }
            }
            MirSharedCastSource::Field { place, .. } => {
                self.require_live_pointee(block, state, place);
            }
        }
    }

    fn apply_shared_cast(&mut self, block: BlockId, state: &mut SharedState, cast: &MirSharedCast) {
        let origin = match (&cast.source, cast.transfer) {
            (MirSharedCastSource::Owner { storage, .. }, MirSharedCastTransfer::Copy) => {
                state.owner_origins.get(storage).copied()
            }
            (MirSharedCastSource::Owner { storage, .. }, MirSharedCastTransfer::Adopt) => {
                let origin = state.owner_origins.remove(storage);
                if !state.live_owners.remove(storage) {
                    self.error(block, "shared cast transfer source is not live");
                } else {
                    state.released_owners.insert(*storage);
                }
                origin
            }
            (MirSharedCastSource::Field { .. }, MirSharedCastTransfer::Copy) => {
                Some(cast.destination)
            }
            (MirSharedCastSource::Field { .. }, MirSharedCastTransfer::Adopt) => None,
        };
        if state.live_owners.contains(&cast.destination)
            || state.released_owners.contains(&cast.destination)
        {
            self.error(block, "shared cast destination is already initialized");
        } else if let Some(origin) = origin {
            state.live_owners.insert(cast.destination);
            state.owner_origins.insert(cast.destination, origin);
        }
        state.pending_full_expression_boundary = origin.is_some();
    }

    fn check_pointee_uses(
        &mut self,
        block: BlockId,
        state: &SharedState,
        instruction: &MirInstruction,
    ) {
        match instruction {
            MirInstruction::Assign(assignment) => match &assignment.rvalue.kind {
                MirRvalueKind::Load(place) => self.require_live_pointee(block, state, place),
                MirRvalueKind::TypeTest { source, .. } => {
                    self.require_live_pointee(block, state, &source.source);
                    self.require_live_shared_origin(block, state, &source.origin);
                }
                _ => {}
            },
            MirInstruction::Store(store) => {
                self.require_live_pointee(block, state, &store.destination)
            }
            MirInstruction::Call(call) => {
                if let Some(receiver) = &call.receiver {
                    match receiver {
                        MirCallReceiver::Method(receiver) => {
                            self.require_live_pointee(block, state, &receiver.place);
                            self.require_live_shared_origin(block, state, &receiver.origin);
                        }
                        MirCallReceiver::Interface(view) => {
                            self.require_live_pointee(block, state, &view.source);
                            self.require_live_shared_origin(block, state, &view.origin);
                        }
                    }
                }
                for argument in &call.arguments {
                    if let MirArgument::Place(place) = argument {
                        self.require_live_pointee(block, state, place);
                    } else if let MirArgument::View(view) = argument {
                        self.require_live_pointee(block, state, &view.source);
                        self.require_live_shared_origin(block, state, &view.origin);
                    }
                }
            }
            MirInstruction::SharedFieldCopy(copy) => {
                self.require_live_pointee(block, state, &copy.source)
            }
            MirInstruction::SharedFieldInitialize(initialize) => {
                self.require_live_pointee(block, state, &initialize.destination)
            }
            MirInstruction::SharedFieldReplace(replace) => {
                self.require_live_pointee(block, state, &replace.destination)
            }
            _ => {}
        }
    }

    fn begin_checked_view(
        &mut self,
        block: BlockId,
        state: &mut SharedState,
        binding: &MirCheckedViewBinding,
    ) {
        let MirPlaceBase::SharedPointee(owner) = binding.view.source.base else {
            return;
        };
        if state
            .active_checked_views
            .insert(binding.destination, owner)
            .is_some()
        {
            self.error(
                block,
                "shared-backed checked-view carrier is activated more than once",
            );
        }
    }

    fn require_live_pointee(&mut self, block: BlockId, state: &SharedState, place: &MirPlace) {
        let MirPlaceBase::SharedPointee(owner) = place.base else {
            return;
        };
        if !state.live_owners.contains(&owner) {
            self.error(block, "shared pointee is used without a live owner");
        }
    }

    fn require_live_shared_origin(
        &mut self,
        block: BlockId,
        state: &SharedState,
        origin: &MirObjectOrigin,
    ) {
        let MirObjectOrigin::Shared {
            owner,
            exact_dynamic_class,
            ..
        } = origin
        else {
            return;
        };
        if !state.live_owners.contains(owner) {
            self.error(block, "shared object origin is used without a live owner");
        }
        if let Some(class) = exact_dynamic_class {
            let exact_origin = state
                .owner_origins
                .get(owner)
                .and_then(|origin| self.function.storage(*origin))
                .is_some_and(|origin| origin.ty == MirType::Class(*class));
            if !exact_origin {
                self.error(
                    block,
                    "shared object origin exact dynamic provenance does not match its allocation",
                );
            }
        }
    }

    fn consume_field_transfer_source(
        &mut self,
        block: BlockId,
        state: &mut SharedState,
        source: StorageId,
    ) {
        self.reject_static_owner(block, state, source, "shared field transfer");
        if !state.live_owners.remove(&source) {
            self.error(block, "shared field transfer source is not a live owner");
        } else {
            state.owner_origins.remove(&source);
            state.released_owners.insert(source);
        }
    }

    fn transfer_call_arguments(
        &mut self,
        block: BlockId,
        state: &mut SharedState,
        arguments: &[MirArgument],
    ) -> bool {
        let mut transferred = false;
        for argument in arguments {
            let MirArgument::SharedOwner(owner) = argument else {
                continue;
            };
            if self
                .function
                .storage(*owner)
                .is_some_and(|storage| matches!(storage.ty, MirType::OptionalShared(_)))
            {
                continue;
            }
            transferred = true;
            self.reject_static_owner(block, state, *owner, "shared call argument");
            if !state.live_owners.remove(owner) {
                self.error(block, "shared call argument is not a live owner");
            } else {
                state.owner_origins.remove(owner);
                state.released_owners.insert(*owner);
            }
        }
        transferred
    }

    fn consume_optional_shared_source(
        &mut self,
        block: BlockId,
        state: &mut SharedState,
        source: StorageId,
    ) {
        self.reject_static_owner(block, state, source, "optional shared injection");
        if !state.live_owners.remove(&source) {
            self.error(
                block,
                "optional shared injection source is not a live ordinary owner",
            );
        } else {
            state.owner_origins.remove(&source);
            state.released_owners.insert(source);
        }
    }

    fn reject_static_owner(
        &mut self,
        block: BlockId,
        state: &SharedState,
        owner: StorageId,
        operation: &'static str,
    ) {
        if state.static_owners.contains_key(&owner) {
            self.error(
                block,
                format!(
                    "{operation} cannot consume static literal backing before string initialization"
                ),
            );
        }
    }

    fn transition(
        &mut self,
        block: BlockId,
        state: &mut SharedState,
        allocation: StorageId,
        expected: AllocationState,
        next: AllocationState,
        message: &'static str,
    ) {
        if state.allocations.get(&allocation) != Some(&expected) {
            self.error(block, message);
        } else {
            state.allocations.insert(allocation, next);
        }
    }

    fn check_return(
        &mut self,
        block: &MirBasicBlock,
        state: &SharedState,
        returned_owner: Option<StorageId>,
    ) {
        if state.pending_full_expression_boundary {
            self.error(
                block.id,
                "shared owner adoption has no full-expression boundary",
            );
        }
        let live_is_exact_result = returned_owner.is_some_and(|owner| {
            state.live_owners.len() == 1 && state.live_owners.contains(&owner)
        });
        if (!state.live_owners.is_empty() && !live_is_exact_result)
            || (returned_owner.is_some() && !live_is_exact_result)
        {
            self.error(block.id, "shared owner remains live on normal return");
        }
        if !state.allocations.is_empty() {
            self.error(
                block.id,
                "shared allocation is not published and adopted on normal return",
            );
        }
        if !state.static_owners.is_empty() {
            self.error(
                block.id,
                "static literal owner is not consumed by string initialization",
            );
        }
        if !state.active_checked_views.is_empty() {
            self.error(
                block.id,
                "shared-backed checked view remains live on normal return",
            );
        }
        if matches!(
            self.function.callable(),
            CallableId::Initializer(_) | CallableId::CopyConstructor(_)
        ) {
            let mut expected = HashSet::new();
            if let Some(receiver) = self.function.receiver() {
                if let Some(MirType::Class(class)) =
                    self.function.storage(receiver).map(|storage| storage.ty)
                {
                    if let Some(class) = self.verifier.program.class(class) {
                        expected.extend(
                            class
                                .fields
                                .iter()
                                .filter(|field| {
                                    matches!(
                                        field.ty,
                                        MirType::Shared(_) | MirType::OptionalShared(_)
                                    )
                                })
                                .map(|field| MirPlace::base(receiver).project_field(field.id)),
                        );
                    }
                }
            }
            if state.initialized_fields != expected {
                self.error(
                    block.id,
                    "shared receiver fields are not initialized exactly once on normal return",
                );
            }
        }
    }

    fn end_conditions_at_storage_death(
        &mut self,
        block: &MirBasicBlock,
        activation_conditions: &HashMap<StorageId, crate::mir::PathConditionId>,
        states: &mut PathStates<SharedState>,
    ) {
        for instruction in &block.instructions {
            let MirInstruction::StorageDead(operation) = instruction else {
                continue;
            };
            let Some(condition) = activation_conditions.get(&operation.storage).copied() else {
                continue;
            };
            let mut incompatible = false;
            let missing = states.end_condition(condition, |existing, incoming| {
                if !existing.same_live_state(incoming) {
                    incompatible = true;
                    return;
                }
                existing
                    .released_owners
                    .extend(incoming.released_owners.iter().copied());
            });
            if incompatible {
                self.error(
                    block.id,
                    format!(
                        "conditional shared ownership state remains when path condition {condition} ends"
                    ),
                );
            }
            if missing {
                self.error(
                    block.id,
                    format!(
                        "path condition {condition} ends outside its selected shared-ownership region"
                    ),
                );
            }
        }
    }

    fn merge(
        &mut self,
        predecessor: BlockId,
        target: BlockId,
        states: &PathStates<SharedState>,
        flow: &mut ForwardDataflow<PathStates<SharedState>>,
    ) {
        if states.is_empty() {
            return;
        }
        let selected = states
            .on_edge(self.function, predecessor, target)
            .unwrap_or_else(|_| states.clone());
        flow.merge(target, &selected, |existing, incoming| {
            existing.merge(incoming, |existing, incoming| {
                if !existing.same_live_state(incoming) {
                    if self.reported_joins.insert(target) {
                        self.error(
                            target,
                            "shared ownership state differs across control-flow paths",
                        );
                    }
                    return;
                }
                existing
                    .released_owners
                    .extend(incoming.released_owners.iter().copied());
            })
        });
    }

    fn error(&mut self, block: BlockId, message: impl Into<String>) {
        self.verifier
            .block_error(self.function.callable(), block, message);
    }
}

impl SharedState {
    fn reset_storage(&mut self, storage: StorageId) {
        self.allocations.remove(&storage);
        self.live_owners.remove(&storage);
        self.owner_origins.remove(&storage);
        self.static_owners.remove(&storage);
        self.released_owners.remove(&storage);
        self.active_checked_views
            .retain(|carrier, owner| *carrier != storage && *owner != storage);
        self.initialized_fields
            .retain(|place| place.base.storage() != storage);
    }

    fn same_live_state(&self, other: &Self) -> bool {
        self.allocations == other.allocations
            && self.live_owners == other.live_owners
            && self.owner_origins == other.owner_origins
            && self.static_owners == other.static_owners
            && self.active_checked_views == other.active_checked_views
            && self.initialized_fields == other.initialized_fields
            && self.pending_full_expression_boundary == other.pending_full_expression_boundary
    }
}
