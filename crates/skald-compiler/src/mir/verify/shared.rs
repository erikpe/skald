//! Structural and path-sensitive verification of shared-owner lifetimes.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::identity::CallableId;

use super::{
    super::model::{
        BlockId, MirBasicBlock, MirDefinitionRef, MirInstruction, MirSharedAdopt,
        MirSharedAllocate, MirSharedAllocationOrigin, MirSharedCopy, MirSharedInitialize,
        MirSharedPublish, MirSharedRelease, MirSharedTarget, MirStorageKind, MirTerminator,
        MirType, StorageId, ValueId,
    },
    context::Verifier,
};

impl<'mir> Verifier<'mir> {
    pub(super) fn verify_shared_target_declared(
        &mut self,
        callable: CallableId,
        target: MirSharedTarget,
    ) {
        let declared = match target {
            MirSharedTarget::Obj => true,
            MirSharedTarget::Class(class) => self.program.class(class).is_some(),
            MirSharedTarget::Interface(interface) => self.program.interface(interface).is_some(),
        };
        if !declared {
            self.function_error(callable, format!("shared target {target} is not declared"));
        }
    }

    pub(super) fn verify_shared_allocate(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        allocation: &MirSharedAllocate,
    ) {
        if allocation.origin != MirSharedAllocationOrigin::New {
            self.block_error(
                function.callable(),
                block.id,
                "shared allocation does not originate from `new`",
            );
        }
        if self.program.class(allocation.class).is_none() {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "shared allocation class {} is not declared",
                    allocation.class
                ),
            );
        }
        self.verify_allocation_storage(
            function,
            block,
            allocation.allocation,
            Some(allocation.class),
        );
    }

    pub(super) fn verify_shared_initialize(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        initialize: &MirSharedInitialize,
        defined: &HashSet<ValueId>,
    ) {
        let class = self.verify_allocation_storage(function, block, initialize.allocation, None);
        let Some(target) = self.program.initializer(initialize.target) else {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "shared initializer target {} is not declared",
                    initialize.target
                ),
            );
            return;
        };
        if class.is_some_and(|class| class != initialize.target.class()) {
            self.block_error(
                function.callable(),
                block.id,
                "shared initializer does not match the exact allocation class",
            );
        }
        self.verify_arguments(
            function,
            block,
            "shared initializer",
            &initialize.arguments,
            &target.parameters,
            defined,
        );
    }

    pub(super) fn verify_shared_publish(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        publish: &MirSharedPublish,
    ) {
        self.verify_allocation_storage(function, block, publish.allocation, None);
    }

    pub(super) fn verify_shared_adopt(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        adopt: &MirSharedAdopt,
    ) {
        let allocation_class =
            self.verify_allocation_storage(function, block, adopt.allocation, None);
        let destination = function.storage(adopt.destination);
        if destination.is_none() {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "shared adoption destination {} is not declared",
                    adopt.destination
                ),
            );
        }
        if !destination.is_some_and(|storage| {
            storage.kind == MirStorageKind::Local
                && allocation_class.is_some_and(|class| {
                    storage.ty == MirType::Shared(MirSharedTarget::Class(class))
                })
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "shared adoption requires compatible exact-class owner storage",
            );
        }
    }

    pub(super) fn verify_shared_copy(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        copy: &MirSharedCopy,
    ) {
        let destination = function.storage(copy.destination);
        let source = function.storage(copy.source);
        if destination.is_none() || source.is_none() {
            self.block_error(
                function.callable(),
                block.id,
                "shared copy storage is not declared in this function",
            );
            return;
        }
        if !matches!(
            (destination, source),
            (Some(destination), Some(source))
                if destination.kind == MirStorageKind::Local
                    && source.kind == MirStorageKind::Local
                    && matches!(destination.ty, MirType::Shared(MirSharedTarget::Class(_)))
                    && destination.ty == source.ty
        ) {
            self.block_error(
                function.callable(),
                block.id,
                "shared copy requires matching exact-class owner storage",
            );
        }
    }

    pub(super) fn verify_shared_release(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        release: &MirSharedRelease,
    ) {
        if !function.storage(release.owner).is_some_and(|storage| {
            storage.kind == MirStorageKind::Local && matches!(storage.ty, MirType::Shared(_))
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "shared release requires local owner storage",
            );
        }
    }

    fn verify_allocation_storage(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        allocation: StorageId,
        expected_class: Option<crate::identity::ClassId>,
    ) -> Option<crate::identity::ClassId> {
        let Some(storage) = function.storage(allocation) else {
            self.block_error(
                function.callable(),
                block.id,
                format!("shared allocation storage {allocation} is not declared"),
            );
            return None;
        };
        let MirType::Class(class) = storage.ty else {
            self.block_error(
                function.callable(),
                block.id,
                "shared allocation storage must have exact class type",
            );
            return None;
        };
        if storage.kind != MirStorageKind::SharedAllocation {
            self.block_error(
                function.callable(),
                block.id,
                "shared construction operation requires allocation storage",
            );
        }
        if expected_class.is_some_and(|expected| expected != class) {
            self.block_error(
                function.callable(),
                block.id,
                "shared allocation instruction has the wrong exact class",
            );
        }
        Some(class)
    }

    pub(super) fn verify_shared_ownership(&mut self, function: MirDefinitionRef<'mir>) {
        SharedOwnershipAnalysis::new(function, self).analyze();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AllocationState {
    Allocated,
    Initialized,
    Published,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SharedState {
    allocations: HashMap<StorageId, AllocationState>,
    live_owners: HashSet<StorageId>,
    owner_origins: HashMap<StorageId, StorageId>,
    released_owners: HashSet<StorageId>,
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
        incoming[self.function.body().entry.index()] = Some(SharedState::default());
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
                })
                | Some(MirTerminator::CheckedCast {
                    success_target: true_target,
                    failure_target: false_target,
                    ..
                }) => {
                    self.merge(*true_target, &state, &mut incoming, &mut pending);
                    self.merge(*false_target, &state, &mut incoming, &mut pending);
                }
                Some(MirTerminator::Return { .. }) => self.check_return(block, &state),
                Some(MirTerminator::Terminate { .. }) | None => {}
            }
        }
    }

    fn apply_block(&mut self, block: &MirBasicBlock, state: &mut SharedState) {
        for instruction in &block.instructions {
            if state.pending_full_expression_boundary
                && !matches!(instruction, MirInstruction::EndFullExpression(_))
            {
                self.error(
                    block.id,
                    "shared owner adoption must end at a full-expression boundary",
                );
                state.pending_full_expression_boundary = false;
            }
            match instruction {
                MirInstruction::SharedAllocate(allocation) => {
                    if state
                        .allocations
                        .insert(allocation.allocation, AllocationState::Allocated)
                        .is_some()
                    {
                        self.error(
                            block.id,
                            "shared allocation storage is allocated more than once",
                        );
                    }
                }
                MirInstruction::SharedInitialize(initialize) => {
                    self.transition(
                        block.id,
                        state,
                        initialize.allocation,
                        AllocationState::Allocated,
                        AllocationState::Initialized,
                        "shared initialization requires unpublished allocated storage",
                    );
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
                MirInstruction::SharedRelease(release) => {
                    if !state.live_owners.remove(&release.owner) {
                        self.error(block.id, "shared owner is released without being live");
                    } else {
                        state.owner_origins.remove(&release.owner);
                        state.released_owners.insert(release.owner);
                    }
                }
                MirInstruction::EndFullExpression(_) => {
                    state.pending_full_expression_boundary = false;
                }
                MirInstruction::Assign(_)
                | MirInstruction::Call(_)
                | MirInstruction::Cleanup(_)
                | MirInstruction::Initialize(_)
                | MirInstruction::Store(_)
                | MirInstruction::CopyConstruct(_)
                | MirInstruction::CopyAssign(_)
                | MirInstruction::BindCheckedView(_)
                | MirInstruction::EndCheckedView(_) => {}
            }
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

    fn check_return(&mut self, block: &MirBasicBlock, state: &SharedState) {
        if state.pending_full_expression_boundary {
            self.error(
                block.id,
                "shared owner adoption has no full-expression boundary",
            );
        }
        if !state.live_owners.is_empty() {
            self.error(block.id, "shared owner remains live on normal return");
        }
        if !state.allocations.is_empty() {
            self.error(
                block.id,
                "shared allocation is not published and adopted on normal return",
            );
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
            && self.pending_full_expression_boundary == other.pending_full_expression_boundary
    }
}
