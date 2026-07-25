//! Path-sensitive allocation, owner, checked-view, and anchor verification.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::identity::CallableId;

use super::super::{
    super::model::{
        BlockId, MirArgument, MirBasicBlock, MirCallReceiver, MirCheckedViewBinding,
        MirDefinitionRef, MirInstruction, MirObjectOrigin, MirPlace, MirPlaceBase, MirRvalueKind,
        MirSharedAllocationMode, MirSharedCast, MirSharedCastSource, MirSharedCastTransfer,
        MirStorageKind, MirTerminator, MirType, StorageId,
    },
    context::Verifier,
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
        let mut incoming = vec![None; self.function.body().blocks.len()];
        if self.function.body().entry.index() >= incoming.len() {
            return;
        }
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
                    self.merge(*target, &state, &mut incoming, &mut pending)
                }
                Some(MirTerminator::Branch {
                    true_target,
                    false_target,
                    ..
                }) => {
                    self.merge(*true_target, &state, &mut incoming, &mut pending);
                    self.merge(*false_target, &state, &mut incoming, &mut pending);
                }
                Some(MirTerminator::CheckedCast {
                    binding,
                    success_target,
                    failure_target,
                    ..
                }) => {
                    self.require_live_pointee(block.id, &state, &binding.view.source);
                    self.require_live_shared_origin(block.id, &state, &binding.view.origin);
                    let mut success = state.clone();
                    self.begin_checked_view(block.id, &mut success, binding);
                    self.merge(*success_target, &success, &mut incoming, &mut pending);
                    self.merge(*failure_target, &state, &mut incoming, &mut pending);
                }
                Some(MirTerminator::SharedCast {
                    cast,
                    success_target,
                    failure_target,
                    ..
                }) => {
                    self.require_shared_cast_source(block.id, &state, cast);
                    let mut success = state.clone();
                    self.apply_shared_cast(block.id, &mut success, cast);
                    self.merge(*success_target, &success, &mut incoming, &mut pending);
                    self.merge(*failure_target, &state, &mut incoming, &mut pending);
                }
                Some(MirTerminator::OptionalUnwrap {
                    success_target,
                    failure_target,
                    ..
                }) => {
                    self.merge(*success_target, &state, &mut incoming, &mut pending);
                    self.merge(*failure_target, &state, &mut incoming, &mut pending);
                }
                Some(MirTerminator::BeginOptionalView {
                    success_target,
                    absent_target,
                    overflow_target,
                    ..
                }) => {
                    self.merge(*success_target, &state, &mut incoming, &mut pending);
                    self.merge(*absent_target, &state, &mut incoming, &mut pending);
                    self.merge(*overflow_target, &state, &mut incoming, &mut pending);
                }
                Some(MirTerminator::CheckOptionalMutation {
                    success_target,
                    failure_target,
                    ..
                }) => {
                    self.merge(*success_target, &state, &mut incoming, &mut pending);
                    self.merge(*failure_target, &state, &mut incoming, &mut pending);
                }
                Some(MirTerminator::Return { .. }) => self.check_return(block, &state, None),
                Some(MirTerminator::ReturnShared { owner, .. }) => {
                    self.check_return(block, &state, Some(*owner))
                }
                Some(MirTerminator::Terminate { .. }) | None => {}
            }
        }
    }

    fn apply_block(&mut self, block: &MirBasicBlock, state: &mut SharedState) {
        for instruction in &block.instructions {
            self.check_pointee_uses(block.id, state, instruction);
            match instruction {
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
                        if state.live_owners.contains(&result)
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
            transferred = true;
            if !state.live_owners.remove(owner) {
                self.error(block, "shared call argument is not a live owner");
            } else {
                state.owner_origins.remove(owner);
                state.released_owners.insert(*owner);
            }
        }
        transferred
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
                                .filter(|field| matches!(field.ty, MirType::Shared(_)))
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

    fn merge(
        &mut self,
        target: BlockId,
        state: &SharedState,
        incoming: &mut [Option<SharedState>],
        pending: &mut VecDeque<BlockId>,
    ) {
        if target.callable() != self.function.callable() || target.index() >= incoming.len() {
            return;
        }
        match &incoming[target.index()] {
            None => {
                incoming[target.index()] = Some(state.clone());
                pending.push_back(target);
            }
            Some(existing) if !existing.same_live_state(state) => {
                if self.reported_joins.insert(target) {
                    self.error(
                        target,
                        "shared ownership state differs across control-flow paths",
                    );
                }
            }
            Some(existing) => {
                let mut merged = existing.clone();
                merged
                    .released_owners
                    .extend(state.released_owners.iter().copied());
                if &merged != existing {
                    incoming[target.index()] = Some(merged);
                    pending.push_back(target);
                }
            }
        }
    }

    fn error(&mut self, block: BlockId, message: impl Into<String>) {
        self.verifier
            .block_error(self.function.callable(), block, message);
    }
}

impl SharedState {
    fn same_live_state(&self, other: &Self) -> bool {
        self.allocations == other.allocations
            && self.live_owners == other.live_owners
            && self.owner_origins == other.owner_origins
            && self.active_checked_views == other.active_checked_views
            && self.initialized_fields == other.initialized_fields
            && self.pending_full_expression_boundary == other.pending_full_expression_boundary
    }
}
