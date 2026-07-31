use std::collections::HashSet;

use crate::{
    identity::CallableId,
    mir::{
        BlockId, MirArgument, MirArrayInstruction, MirBasicBlock, MirCheckedViewBinding,
        MirInstruction, MirPlace, MirPlaceBase, MirSharedAllocationMode, MirSharedCast,
        MirSharedCastSource, MirSharedCastTransfer, MirStorageKind, MirType, StorageId,
    },
};

use super::{
    state::{AllocationState, SharedState},
    SharedOwnershipAnalysis,
};

impl SharedOwnershipAnalysis<'_, '_> {
    pub(super) fn apply_block(&mut self, block: &MirBasicBlock, state: &mut SharedState) {
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

    pub(super) fn apply_shared_cast(
        &mut self,
        block: BlockId,
        state: &mut SharedState,
        cast: &MirSharedCast,
    ) {
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

    pub(super) fn begin_checked_view(
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

    pub(super) fn check_return(
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
}
