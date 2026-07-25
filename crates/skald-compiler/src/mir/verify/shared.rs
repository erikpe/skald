//! Structural and path-sensitive verification of shared-owner lifetimes.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::identity::CallableId;

use super::{
    super::model::{
        BlockId, MirArgument, MirBasicBlock, MirDefinitionRef, MirInstruction, MirSharedAdopt,
        MirSharedAllocate, MirSharedAllocationOrigin, MirSharedCast, MirSharedCastSource,
        MirSharedCastTransfer, MirSharedCopy, MirSharedFieldCopy, MirSharedFieldInitialize,
        MirSharedFieldReplace, MirSharedInitialize, MirSharedMove, MirSharedPublish,
        MirSharedRelease, MirSharedTarget, MirStorageKind, MirTerminator, MirType, MirViewTarget,
        StorageId, ValueId,
    },
    context::Verifier,
};

impl<'mir> Verifier<'mir> {
    pub(super) fn shared_target_accepts(
        &self,
        expected: MirSharedTarget,
        actual: MirSharedTarget,
    ) -> bool {
        match expected {
            MirSharedTarget::Obj => true,
            MirSharedTarget::Class(expected) => matches!(
                actual,
                MirSharedTarget::Class(actual)
                    if actual == expected || self.program.is_ancestor(expected, actual)
            ),
            MirSharedTarget::Interface(expected) => match actual {
                MirSharedTarget::Class(actual) => {
                    self.program.conformance(actual, expected).is_some()
                }
                MirSharedTarget::Interface(actual) => actual == expected,
                MirSharedTarget::Obj => false,
            },
        }
    }

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
            matches!(
                storage.kind,
                MirStorageKind::Local
                    | MirStorageKind::Temporary
                    | MirStorageKind::SharedAnchor
                    | MirStorageKind::Argument
                    | MirStorageKind::Return
            ) && allocation_class.is_some_and(|class| {
                matches!(
                    storage.ty,
                    MirType::Shared(target)
                        if self.shared_target_accepts(target, MirSharedTarget::Class(class))
                )
            })
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "shared adoption requires a compatible destination owner target",
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
                if matches!(
                    destination.kind,
                    MirStorageKind::Local
                        | MirStorageKind::Temporary
                        | MirStorageKind::Argument
                        | MirStorageKind::Return
                )
                    && matches!(
                        source.kind,
                        MirStorageKind::Local
                            | MirStorageKind::Parameter
                            | MirStorageKind::Temporary
                            | MirStorageKind::Argument
                            | MirStorageKind::Return
                    )
                    && matches!(
                        (destination.ty, source.ty),
                        (MirType::Shared(expected), MirType::Shared(actual))
                            if self.shared_target_accepts(expected, actual)
                    )
        ) {
            self.block_error(
                function.callable(),
                block.id,
                "shared copy requires compatible source and destination owner storage",
            );
        }
    }

    pub(super) fn verify_shared_move(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        transfer: &MirSharedMove,
    ) {
        let destination = function.storage(transfer.destination);
        let source = function.storage(transfer.source);
        if !matches!(
            (destination, source),
            (Some(destination), Some(source))
                if matches!(
                    destination.kind,
                    MirStorageKind::Local | MirStorageKind::Parameter
                )
                    && source.kind == MirStorageKind::Temporary
                    && matches!(
                        (destination.ty, source.ty),
                        (MirType::Shared(expected), MirType::Shared(actual))
                            if self.shared_target_accepts(expected, actual)
                    )
        ) {
            self.block_error(
                function.callable(),
                block.id,
                "shared move requires a matching temporary and replaceable owner destination",
            );
        }
    }

    pub(super) fn verify_shared_field_copy(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        copy: &MirSharedFieldCopy,
    ) {
        let destination = function.storage(copy.destination);
        let source = self.verify_place(function, block, &copy.source);
        if !matches!(
            (destination, source),
            (Some(destination), Some(source))
                if matches!(
                    destination.kind,
                    MirStorageKind::Local
                        | MirStorageKind::Temporary
                        | MirStorageKind::SharedAnchor
                        | MirStorageKind::Argument
                        | MirStorageKind::Return
                )
                    && matches!(
                        (destination.ty, source.ty),
                        (MirType::Shared(expected), MirType::Shared(actual))
                            if self.shared_target_accepts(expected, actual)
                    )
                    && matches!(
                        copy.source.projections.last(),
                        Some(super::super::model::MirPlaceProjection::Field(_))
                    )
        ) {
            self.block_error(
                function.callable(),
                block.id,
                "shared field copy requires a compatible shared field and fresh owner storage",
            );
        }
    }

    pub(super) fn verify_shared_cast(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        cast: &MirSharedCast,
        runtime: bool,
    ) {
        let destination = function.storage(cast.destination);
        if !destination.is_some_and(|storage| {
            matches!(
                storage.kind,
                MirStorageKind::Local
                    | MirStorageKind::Temporary
                    | MirStorageKind::SharedAnchor
                    | MirStorageKind::Argument
                    | MirStorageKind::Return
            ) && storage.ty == MirType::Shared(cast.target)
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "shared cast requires matching fresh owner destination storage",
            );
        }
        self.verify_shared_target_declared(function.callable(), cast.target);
        let source_target = cast.source.target();
        let valid_source = match &cast.source {
            MirSharedCastSource::Owner { storage, target } => {
                function.storage(*storage).is_some_and(|source| {
                    source.ty == MirType::Shared(*target)
                        && match cast.transfer {
                            MirSharedCastTransfer::Copy => {
                                matches!(
                                    source.kind,
                                    MirStorageKind::Local | MirStorageKind::Parameter
                                ) && cast.exact_dynamic_class.is_none()
                            }
                            MirSharedCastTransfer::Adopt => {
                                source.kind == MirStorageKind::Temporary
                            }
                        }
                })
            }
            MirSharedCastSource::Field { place, target } => {
                cast.transfer == MirSharedCastTransfer::Copy
                    && cast.exact_dynamic_class.is_none()
                    && self
                        .verify_place(function, block, place)
                        .is_some_and(|source| {
                            source.ty == MirType::Shared(*target)
                                && matches!(
                                    place.projections.last(),
                                    Some(super::super::model::MirPlaceProjection::Field(_))
                                )
                        })
            }
        };
        if !valid_source {
            self.block_error(
                function.callable(),
                block.id,
                "shared cast source provenance or copy/adopt operation is invalid",
            );
        }
        self.verify_shared_target_declared(function.callable(), source_target);
        let target = shared_view_target(cast.target);
        let relation = cast.exact_dynamic_class.map_or_else(
            || self.classify_type_relation(source_target.ty(), target),
            |class| {
                if self.class_provides_view(class, target) {
                    super::type_operations::TypeRelation::StaticSuccess
                } else {
                    super::type_operations::TypeRelation::StaticFailure
                }
            },
        );
        let expected = if runtime {
            super::type_operations::TypeRelation::Runtime
        } else {
            super::type_operations::TypeRelation::StaticSuccess
        };
        if relation != expected {
            self.block_error(
                function.callable(),
                block.id,
                if runtime {
                    "shared cast does not require a runtime metadata check"
                } else {
                    "static shared cast is not guaranteed to succeed"
                },
            );
        }
        if let Some(class) = cast.exact_dynamic_class {
            if self.program.class(class).is_none()
                || !self.class_can_inhabit_type(class, source_target.ty())
            {
                self.block_error(
                    function.callable(),
                    block.id,
                    "shared cast exact dynamic provenance cannot inhabit its source target",
                );
            }
        }
    }

    pub(super) fn verify_shared_cast_terminator(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        cast: &MirSharedCast,
        success_target: BlockId,
        failure_target: BlockId,
    ) {
        self.verify_shared_cast(function, block, cast, true);
        self.verify_block_target(function, block, success_target);
        self.verify_block_target(function, block, failure_target);
        if success_target == failure_target {
            self.block_error(
                function.callable(),
                block.id,
                "shared cast success and failure edges must differ",
            );
        }
        if !function.block(failure_target).is_some_and(|failure| {
            matches!(
                failure.terminator,
                Some(MirTerminator::Terminate {
                    reason: super::super::model::MirTerminationReason::ObjectCastFailure,
                    ..
                })
            )
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "shared cast failure edge must terminate with object-cast failure",
            );
        }
    }

    pub(super) fn verify_shared_field_initialize(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        initialize: &MirSharedFieldInitialize,
    ) {
        self.verify_shared_field_destination(
            function,
            block,
            &initialize.destination,
            initialize.source,
            true,
        );
    }

    pub(super) fn verify_shared_field_replace(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        replace: &MirSharedFieldReplace,
    ) {
        self.verify_shared_field_destination(
            function,
            block,
            &replace.destination,
            replace.source,
            false,
        );
    }

    fn verify_shared_field_destination(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination: &super::super::model::MirPlace,
        source: StorageId,
        initialization: bool,
    ) {
        let field = self.verify_place(function, block, destination);
        let source = function.storage(source);
        let is_direct_field = matches!(
            destination.projections.last(),
            Some(super::super::model::MirPlaceProjection::Field(_))
        );
        let receiver_initialization = function.receiver() == Some(destination.base.storage())
            && matches!(
                function.callable(),
                CallableId::Initializer(_) | CallableId::CopyConstructor(_)
            );
        let valid = matches!(
            (field, source),
            (Some(field), Some(source))
                if is_direct_field
                    && field.access == super::super::model::MirAliasAccess::Mutable
                    && matches!(field.ty, MirType::Shared(_))
                    && field.ty == source.ty
                    && source.kind == MirStorageKind::Temporary
        );
        if !valid
            || (initialization && !receiver_initialization)
            || (!initialization && receiver_initialization)
        {
            self.block_error(
                function.callable(),
                block.id,
                if initialization {
                    "shared field initialization requires a mutable receiver field and matching temporary owner"
                } else {
                    "shared field replacement requires a mutable shared field and matching temporary owner"
                },
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
            matches!(
                storage.kind,
                MirStorageKind::Local
                    | MirStorageKind::Parameter
                    | MirStorageKind::Temporary
                    | MirStorageKind::SharedAnchor
            ) && matches!(storage.ty, MirType::Shared(_))
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "shared release requires local, parameter, or temporary owner storage",
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
    initialized_fields: HashSet<super::super::model::MirPlace>,
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
                })
                | Some(MirTerminator::CheckedCast {
                    success_target: true_target,
                    failure_target: false_target,
                    ..
                }) => {
                    self.merge(*true_target, &state, &mut incoming, &mut pending);
                    self.merge(*false_target, &state, &mut incoming, &mut pending);
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
                    self.transfer_call_arguments(block.id, state, &initialize.arguments);
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
                | MirInstruction::CopyConstruct(_)
                | MirInstruction::CopyAssign(_)
                | MirInstruction::BindCheckedView(_)
                | MirInstruction::EndCheckedView(_) => {}
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
                super::super::model::MirRvalueKind::Load(place) => {
                    self.require_live_pointee(block, state, place)
                }
                super::super::model::MirRvalueKind::TypeTest { source, .. } => {
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
                        super::super::model::MirCallReceiver::Method(receiver) => {
                            self.require_live_pointee(block, state, &receiver.place);
                            self.require_live_shared_origin(block, state, &receiver.origin);
                        }
                        super::super::model::MirCallReceiver::Interface(view) => {
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

    fn require_live_pointee(
        &mut self,
        block: BlockId,
        state: &SharedState,
        place: &super::super::model::MirPlace,
    ) {
        let super::super::model::MirPlaceBase::SharedPointee(owner) = place.base else {
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
        origin: &super::super::model::MirObjectOrigin,
    ) {
        let super::super::model::MirObjectOrigin::Shared { owner, .. } = origin else {
            return;
        };
        if !state.live_owners.contains(owner) {
            self.error(block, "shared object origin is used without a live owner");
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
                                .map(|field| {
                                    super::super::model::MirPlace::base(receiver)
                                        .project_field(field.id)
                                }),
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
            && self.initialized_fields == other.initialized_fields
            && self.pending_full_expression_boundary == other.pending_full_expression_boundary
    }
}

const fn shared_view_target(target: MirSharedTarget) -> MirViewTarget {
    match target {
        MirSharedTarget::Obj => MirViewTarget::Obj,
        MirSharedTarget::Class(class) => MirViewTarget::Class(class),
        MirSharedTarget::Interface(interface) => MirViewTarget::Interface(interface),
    }
}
