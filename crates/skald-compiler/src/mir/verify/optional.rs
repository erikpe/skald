//! Primitive optional structure and definite-initialization verification.

use std::collections::{BTreeMap, HashSet, VecDeque};

use super::{
    super::model::{
        MirAliasAccess, MirBasicBlock, MirDefinitionRef, MirInstruction, MirOptionalSharedSource,
        MirOptionalSource, MirPlace, MirPrimitiveType, MirProgram, MirRvalueKind, MirSharedTarget,
        MirStorageKind, MirTerminationReason, MirTerminator, MirType, OptionalGuardId, StorageId,
        ValueId,
    },
    context::Verifier,
};

impl Verifier<'_> {
    pub(super) fn verify_optional_shared_operation(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination: &MirPlace,
        source: &MirOptionalSharedSource,
        target: MirSharedTarget,
    ) {
        self.verify_shared_target_declared(function.callable(), target);
        if self
            .verify_place(function, block, destination)
            .map(|place| place.ty)
            != Some(MirType::OptionalShared(target))
        {
            self.block_error(
                function.callable(),
                block.id,
                "optional shared destination has the wrong exact target type",
            );
        }
        let actual = match source {
            MirOptionalSharedSource::Absent => return,
            MirOptionalSharedSource::Present(owner) => {
                function
                    .storage(*owner)
                    .and_then(|storage| match storage.ty {
                        MirType::Shared(target) => Some(target),
                        _ => None,
                    })
            }
            MirOptionalSharedSource::Move(owner) => {
                function
                    .storage(*owner)
                    .and_then(|storage| match storage.ty {
                        MirType::OptionalShared(target) => Some(target),
                        _ => None,
                    })
            }
            MirOptionalSharedSource::Copy(place) => self
                .verify_place(function, block, place)
                .and_then(|place| match place.ty {
                    MirType::OptionalShared(target) => Some(target),
                    _ => None,
                }),
        };
        if !actual.is_some_and(|actual| self.shared_target_accepts(target, actual)) {
            self.block_error(
                function.callable(),
                block.id,
                "optional shared source is not a compatible owner",
            );
        }
    }

    pub(super) fn verify_optional_shared_cleanup(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        cleanup: &crate::mir::MirOptionalSharedCleanup,
    ) {
        if self
            .verify_place(function, block, &cleanup.destination)
            .map(|place| place.ty)
            != Some(MirType::OptionalShared(cleanup.target))
        {
            self.block_error(
                function.callable(),
                block.id,
                "optional shared cleanup has the wrong exact target type",
            );
        }
    }

    pub(super) fn verify_optional_shared_unwrap_terminator(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        unwrap: &crate::mir::MirOptionalSharedUnwrap,
        success_target: crate::mir::BlockId,
        failure_target: crate::mir::BlockId,
    ) {
        let source_valid = self
            .verify_place(function, block, &unwrap.source)
            .is_some_and(|place| place.ty == MirType::OptionalShared(unwrap.target));
        let destination_valid = function.storage(unwrap.destination).is_some_and(|storage| {
            matches!(
                storage.kind,
                MirStorageKind::Temporary | MirStorageKind::SharedAnchor
            ) && storage.ty == MirType::Shared(unwrap.target)
                && storage.source.is_none()
        });
        if !source_valid || !destination_valid {
            self.block_error(
                function.callable(),
                block.id,
                "optional shared unwrap requires matching optional source and fresh shared owner",
            );
        }
        self.verify_block_target(function, block, success_target);
        self.verify_block_target(function, block, failure_target);
        if success_target == failure_target
            || !function.block(failure_target).is_some_and(|failure| {
                failure.instructions.is_empty()
                    && matches!(
                        failure.terminator,
                        Some(MirTerminator::Terminate {
                            reason: MirTerminationReason::OptionalAccessFailure,
                            ..
                        })
                    )
            })
        {
            self.block_error(
                function.callable(),
                block.id,
                "optional shared unwrap failure edge must be distinct and terminate with optional-access failure",
            );
        }
    }

    pub(super) fn verify_optional_view_begin(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        begin: &crate::mir::MirOptionalViewBegin,
        success_target: crate::mir::BlockId,
        absent_target: crate::mir::BlockId,
        overflow_target: crate::mir::BlockId,
    ) {
        if begin.guard.callable() != function.callable() {
            self.block_error(
                function.callable(),
                block.id,
                "optional guard belongs to another callable",
            );
        }
        if self
            .verify_place(function, block, &begin.source)
            .map(|place| place.ty)
            != Some(MirType::OptionalClass(begin.class))
        {
            self.block_error(
                function.callable(),
                block.id,
                "optional-view source has the wrong exact class type",
            );
        }
        for target in [success_target, absent_target, overflow_target] {
            self.verify_block_target(function, block, target);
        }
        if success_target == absent_target
            || success_target == overflow_target
            || absent_target == overflow_target
        {
            self.block_error(
                function.callable(),
                block.id,
                "optional-view success, absence, and overflow edges must be distinct",
            );
        }
        self.require_failure_edge(
            function,
            block,
            absent_target,
            MirTerminationReason::OptionalAccessFailure,
            "optional-view absence edge must terminate with optional-access failure",
        );
        self.require_failure_edge(
            function,
            block,
            overflow_target,
            MirTerminationReason::OptionalGuardOverflow,
            "optional-view overflow edge must terminate with optional-guard overflow",
        );
    }

    pub(super) fn verify_optional_mutation_check(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        source: &MirPlace,
        success_target: crate::mir::BlockId,
        failure_target: crate::mir::BlockId,
    ) {
        if !matches!(
            self.verify_place(function, block, source)
                .map(|place| place.ty),
            Some(MirType::OptionalClass(_))
        ) {
            self.block_error(
                function.callable(),
                block.id,
                "optional mutation check requires exact-class optional storage",
            );
        }
        self.verify_block_target(function, block, success_target);
        self.verify_block_target(function, block, failure_target);
        if success_target == failure_target {
            self.block_error(
                function.callable(),
                block.id,
                "optional mutation success and failure edges must be distinct",
            );
        }
        self.require_failure_edge(
            function,
            block,
            failure_target,
            MirTerminationReason::OptionalPinnedMutation,
            "optional mutation failure edge must terminate with optional-pinned-mutation",
        );
    }

    pub(super) fn verify_optional_view_end(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        end: &crate::mir::MirOptionalViewEnd,
    ) {
        if end.guard.callable() != function.callable()
            || self
                .verify_place(function, block, &end.source)
                .map(|place| place.ty)
                != Some(MirType::OptionalClass(end.class))
        {
            self.block_error(
                function.callable(),
                block.id,
                "optional-view end has an incompatible guard root",
            );
        }
    }

    fn require_failure_edge(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        target: crate::mir::BlockId,
        reason: MirTerminationReason,
        message: &'static str,
    ) {
        if function.block(target).is_some_and(|failure| {
            !failure.instructions.is_empty()
                || !matches!(
                    failure.terminator,
                    Some(MirTerminator::Terminate { reason: actual, .. }) if actual == reason
                )
        }) {
            self.block_error(function.callable(), block.id, message);
        }
    }

    pub(super) fn verify_optional_initialize(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination: &MirPlace,
        source: &MirOptionalSource,
        defined: &HashSet<ValueId>,
    ) {
        let payload = self.verify_optional_place(function, block, destination);
        self.verify_optional_source(function, block, source, payload, defined);
    }

    pub(super) fn verify_optional_assign(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination: &MirPlace,
        source: &MirOptionalSource,
        defined: &HashSet<ValueId>,
    ) {
        let checked = self.verify_place(function, block, destination);
        if checked.is_some_and(|place| place.access != MirAliasAccess::Mutable) {
            self.block_error(
                function.callable(),
                block.id,
                "optional assignment destination requires mutable access",
            );
        }
        let payload = self.verify_optional_place(function, block, destination);
        self.verify_optional_source(function, block, source, payload, defined);
    }

    pub(super) fn verify_optional_presence(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        source: &MirPlace,
        result_type: MirType,
    ) {
        if !matches!(
            self.verify_place(function, block, source)
                .map(|place| place.ty),
            Some(
                MirType::OptionalPrimitive(_)
                    | MirType::OptionalClass(_)
                    | MirType::OptionalShared(_)
            )
        ) {
            self.block_error(
                function.callable(),
                block.id,
                "optional presence-test source is not optional storage",
            );
        }
        if result_type != MirType::Bool {
            self.block_error(
                function.callable(),
                block.id,
                "optional presence test result is not `bool`",
            );
        }
    }

    pub(super) fn verify_optional_unwrap_terminator(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        source: &MirPlace,
        destination: StorageId,
        success_target: crate::mir::BlockId,
        failure_target: crate::mir::BlockId,
    ) {
        let payload = self.verify_optional_storage(function, block, source);
        let destination_valid = function.storage(destination).is_some_and(|storage| {
            storage.kind == MirStorageKind::OptionalUnwrap
                && Some(storage.ty) == payload.map(MirPrimitiveType::payload_type)
                && storage.source.is_none()
        });
        if !destination_valid {
            self.block_error(
                function.callable(),
                block.id,
                "optional unwrap destination must be matching compiler-owned scalar storage",
            );
        }
        self.verify_block_target(function, block, success_target);
        self.verify_block_target(function, block, failure_target);
        if success_target == failure_target {
            self.block_error(
                function.callable(),
                block.id,
                "optional unwrap success and failure edges must be distinct",
            );
        }
        if function.block(failure_target).is_some_and(|failure| {
            !failure.instructions.is_empty()
                || !matches!(
                    failure.terminator,
                    Some(MirTerminator::Terminate {
                        reason: crate::mir::MirTerminationReason::OptionalAccessFailure,
                        ..
                    })
                )
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "optional unwrap failure edge must terminate with optional-access failure",
            );
        }
    }

    pub(super) fn verify_optional_initialization(&mut self, function: MirDefinitionRef<'_>) {
        if function.body().entry.index() >= function.body().blocks.len() {
            return;
        }
        let mut incoming: Vec<Option<HashSet<MirPlace>>> = vec![None; function.body().blocks.len()];
        incoming[function.body().entry.index()] = Some(initialized_at_entry(function));
        let mut pending = VecDeque::from([function.body().entry]);

        while let Some(block_id) = pending.pop_front() {
            let Some(block) = function.block(block_id) else {
                continue;
            };
            let Some(mut state) = incoming[block_id.index()].clone() else {
                continue;
            };
            apply_initializations(self.program, function, block, &mut state);
            for successor in block.terminator.iter().flat_map(MirTerminator::successors) {
                let Some(slot) = incoming.get_mut(successor.index()) else {
                    continue;
                };
                let changed = match slot {
                    Some(existing) => {
                        let merged: HashSet<_> = existing.intersection(&state).cloned().collect();
                        if *existing == merged {
                            false
                        } else {
                            *existing = merged;
                            true
                        }
                    }
                    None => {
                        *slot = Some(state.clone());
                        true
                    }
                };
                if changed {
                    pending.push_back(successor);
                }
            }
        }

        for block in &function.body().blocks {
            let Some(Some(mut state)) = incoming.get(block.id.index()).cloned() else {
                continue;
            };
            for instruction in &block.instructions {
                match instruction {
                    MirInstruction::OptionalInitialize(initialize) => {
                        require_initialized_source(
                            self,
                            function,
                            block,
                            &initialize.source,
                            &state,
                        );
                        if !state.insert(initialize.destination.clone()) {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "optional storage is initialized more than once",
                            );
                        }
                    }
                    MirInstruction::OptionalAssign(assignment) => {
                        require_initialized(
                            self,
                            function,
                            block,
                            &assignment.destination,
                            &state,
                            "optional assignment destination",
                        );
                        require_initialized_source(
                            self,
                            function,
                            block,
                            &assignment.source,
                            &state,
                        );
                    }
                    MirInstruction::OptionalSharedInitialize(initialize) => {
                        require_initialized_optional_shared_source(
                            self,
                            function,
                            block,
                            &initialize.source,
                            &state,
                        );
                        if !state.insert(initialize.destination.clone()) {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "optional shared storage is initialized more than once",
                            );
                        }
                    }
                    MirInstruction::OptionalSharedAssign(assignment) => {
                        require_initialized(
                            self,
                            function,
                            block,
                            &assignment.destination,
                            &state,
                            "optional shared assignment destination",
                        );
                        require_initialized_optional_shared_source(
                            self,
                            function,
                            block,
                            &assignment.source,
                            &state,
                        );
                    }
                    MirInstruction::OptionalSharedCleanup(cleanup) => require_initialized(
                        self,
                        function,
                        block,
                        &cleanup.destination,
                        &state,
                        "optional shared cleanup destination",
                    ),
                    MirInstruction::ClassOptionalInitialize(initialize) => {
                        require_initialized_class_source(
                            self,
                            function,
                            block,
                            &initialize.source,
                            &state,
                        );
                        if !state.insert(initialize.destination.clone()) {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "class optional storage is initialized more than once",
                            );
                        }
                    }
                    MirInstruction::ClassOptionalAssign(assignment) => {
                        require_initialized(
                            self,
                            function,
                            block,
                            &assignment.destination,
                            &state,
                            "class optional assignment destination",
                        );
                        require_initialized_class_source(
                            self,
                            function,
                            block,
                            &assignment.source,
                            &state,
                        );
                    }
                    MirInstruction::Assign(assignment) => {
                        if let MirRvalueKind::OptionalPresence { source, .. } =
                            &assignment.rvalue.kind
                        {
                            require_initialized(
                                self,
                                function,
                                block,
                                source,
                                &state,
                                "optional presence-test source",
                            );
                        }
                    }
                    MirInstruction::Call(call) => {
                        if let Some(result) = call.shared_result {
                            if function.storage(result).is_some_and(|storage| {
                                matches!(storage.ty, MirType::OptionalShared(_))
                            }) {
                                state.insert(MirPlace::base(result));
                            }
                        }
                        if let Some(destination) = &call.destination {
                            if function
                                .storage(destination.base.storage())
                                .is_some_and(|storage| {
                                    matches!(
                                        storage.ty,
                                        MirType::OptionalPrimitive(_)
                                            | MirType::OptionalClass(_)
                                            | MirType::OptionalShared(_)
                                    )
                                })
                                && !state.insert(destination.clone())
                            {
                                self.block_error(
                                    function.callable(),
                                    block.id,
                                    "call destination is already initialized",
                                );
                            } else if let Some(class) =
                                complete_class_storage(function, destination)
                            {
                                initialize_optional_fields(
                                    self.program,
                                    class,
                                    destination,
                                    &mut state,
                                );
                            }
                        }
                    }
                    MirInstruction::Initialize(initialize) => initialize_optional_fields(
                        self.program,
                        initialize.target.class(),
                        &initialize.destination,
                        &mut state,
                    ),
                    MirInstruction::CopyConstruct(copy) => initialize_optional_fields(
                        self.program,
                        copy.class,
                        &copy.destination,
                        &mut state,
                    ),
                    _ => {}
                }
            }
            if let Some(MirTerminator::OptionalUnwrap { source, .. }) = &block.terminator {
                require_initialized(
                    self,
                    function,
                    block,
                    source,
                    &state,
                    "optional unwrap source",
                );
            }
            if let Some(MirTerminator::OptionalSharedUnwrap { unwrap, .. }) = &block.terminator {
                require_initialized(
                    self,
                    function,
                    block,
                    &unwrap.source,
                    &state,
                    "optional shared unwrap source",
                );
            }
            match &block.terminator {
                Some(MirTerminator::BeginOptionalView { begin, .. }) => require_initialized(
                    self,
                    function,
                    block,
                    &begin.source,
                    &state,
                    "optional-view source",
                ),
                Some(MirTerminator::CheckOptionalMutation { source, .. }) => require_initialized(
                    self,
                    function,
                    block,
                    source,
                    &state,
                    "optional mutation-check source",
                ),
                _ => {}
            }
            if matches!(
                block.terminator,
                Some(MirTerminator::Return { .. } | MirTerminator::ReturnOptionalShared { .. })
            ) {
                if let Some(return_storage) = function.return_storage() {
                    let place = MirPlace::base(return_storage);
                    if function.storage(return_storage).is_some_and(|storage| {
                        matches!(
                            storage.ty,
                            MirType::OptionalPrimitive(_)
                                | MirType::OptionalClass(_)
                                | MirType::OptionalShared(_)
                        )
                    }) {
                        require_initialized(
                            self,
                            function,
                            block,
                            &place,
                            &state,
                            "optional return destination",
                        );
                    }
                }
            }
        }
    }

    pub(super) fn verify_optional_guards(&mut self, function: MirDefinitionRef<'_>) {
        if function.body().entry.index() >= function.body().blocks.len() {
            return;
        }
        let mut incoming = vec![None; function.body().blocks.len()];
        incoming[function.body().entry.index()] = Some(OptionalGuardState::default());
        let mut pending = VecDeque::from([function.body().entry]);
        let mut reported_joins = HashSet::new();

        while let Some(block_id) = pending.pop_front() {
            let Some(block) = function.block(block_id) else {
                continue;
            };
            let Some(mut state) = incoming[block_id.index()].clone() else {
                continue;
            };
            for instruction in &block.instructions {
                self.verify_guarded_payload_instruction(function, block, instruction, &state);
                if !state.mutation_permits.is_empty()
                    && !matches!(
                        instruction,
                        MirInstruction::ClassOptionalAssign(_)
                            | MirInstruction::ClassOptionalCleanup(_)
                    )
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "optional mutation check is not immediately followed by its transition",
                    );
                    state.mutation_permits.clear();
                }
                match instruction {
                    MirInstruction::EndOptionalView(end) => {
                        let expected = (end.source.clone(), end.class);
                        let ordered = state.order.last() == Some(&end.guard);
                        if !ordered || state.active.remove(&end.guard) != Some(expected) {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "optional view must end its matching active guard in reverse begin order",
                            );
                        }
                        if ordered {
                            state.order.pop();
                        }
                    }
                    MirInstruction::ClassOptionalAssign(assignment) => {
                        let self_copy = matches!(
                            &assignment.source,
                            crate::mir::MirClassOptionalSource::Copy(source)
                                if source == &assignment.destination
                        );
                        if !self_copy && !state.mutation_permits.remove(&assignment.destination) {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "class optional assignment lacks a matching mutation check",
                            );
                        }
                    }
                    MirInstruction::ClassOptionalCleanup(cleanup) => {
                        if !state.mutation_permits.remove(&cleanup.destination) {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "class optional cleanup lacks a matching mutation check",
                            );
                        }
                    }
                    MirInstruction::SharedRelease(release)
                        if state.active.values().any(|(source, _)| {
                            matches!(
                                source.base,
                                crate::mir::MirPlaceBase::SharedPointee(owner)
                                    if owner == release.owner
                            )
                        }) =>
                    {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "optional payload guard outlives its shared container anchor",
                        );
                    }
                    _ => {}
                }
            }
            if !state.mutation_permits.is_empty() {
                self.block_error(
                    function.callable(),
                    block.id,
                    "optional mutation check has no immediate transition",
                );
                state.mutation_permits.clear();
            }

            match &block.terminator {
                Some(MirTerminator::BeginOptionalView {
                    begin,
                    success_target,
                    absent_target,
                    overflow_target,
                    ..
                }) => {
                    self.require_active_payload_guards(function, block, &begin.source, &state);
                    let mut success = state.clone();
                    if success
                        .active
                        .insert(begin.guard, (begin.source.clone(), begin.class))
                        .is_some()
                    {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "optional guard begins more than once",
                        );
                    } else {
                        success.order.push(begin.guard);
                    }
                    merge_optional_guard_state(
                        self,
                        function,
                        *success_target,
                        &success,
                        &mut incoming,
                        &mut pending,
                        &mut reported_joins,
                    );
                    for target in [*absent_target, *overflow_target] {
                        merge_optional_guard_state(
                            self,
                            function,
                            target,
                            &state,
                            &mut incoming,
                            &mut pending,
                            &mut reported_joins,
                        );
                    }
                }
                Some(MirTerminator::CheckOptionalMutation {
                    source,
                    success_target,
                    failure_target,
                    ..
                }) => {
                    self.require_active_payload_guards(function, block, source, &state);
                    let mut success = state.clone();
                    success.mutation_permits.insert(source.clone());
                    merge_optional_guard_state(
                        self,
                        function,
                        *success_target,
                        &success,
                        &mut incoming,
                        &mut pending,
                        &mut reported_joins,
                    );
                    merge_optional_guard_state(
                        self,
                        function,
                        *failure_target,
                        &state,
                        &mut incoming,
                        &mut pending,
                        &mut reported_joins,
                    );
                }
                Some(
                    MirTerminator::Return { .. }
                    | MirTerminator::ReturnShared { .. }
                    | MirTerminator::ReturnOptionalShared { .. },
                ) => {
                    if !state.active.is_empty() {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "optional payload guard remains active on normal return",
                        );
                    }
                }
                Some(terminator) => {
                    if let MirTerminator::CheckedCast { binding, .. } = terminator {
                        self.require_active_payload_guards(
                            function,
                            block,
                            &binding.view.source,
                            &state,
                        );
                    }
                    for target in terminator.successors() {
                        merge_optional_guard_state(
                            self,
                            function,
                            target,
                            &state,
                            &mut incoming,
                            &mut pending,
                            &mut reported_joins,
                        );
                    }
                }
                None => {}
            }
        }
    }

    fn verify_guarded_payload_instruction(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        instruction: &MirInstruction,
        state: &OptionalGuardState,
    ) {
        match instruction {
            MirInstruction::Assign(assignment) => match &assignment.rvalue.kind {
                MirRvalueKind::Load(source) | MirRvalueKind::OptionalPresence { source, .. } => {
                    self.require_active_payload_guards(function, block, source, state);
                }
                MirRvalueKind::TypeTest { source, .. } => {
                    self.require_active_payload_guards(function, block, &source.source, state);
                }
                _ => {}
            },
            MirInstruction::Call(call) => {
                if let Some(receiver) = &call.receiver {
                    let place = match receiver {
                        crate::mir::MirCallReceiver::Method(receiver) => &receiver.place,
                        crate::mir::MirCallReceiver::Interface(view) => &view.source,
                    };
                    self.require_active_payload_guards(function, block, place, state);
                }
                for argument in &call.arguments {
                    let place = match argument {
                        crate::mir::MirArgument::Place(place)
                        | crate::mir::MirArgument::OwnedPlace(place) => Some(place),
                        crate::mir::MirArgument::View(view) => Some(&view.source),
                        crate::mir::MirArgument::Value(_)
                        | crate::mir::MirArgument::SharedOwner(_) => None,
                    };
                    if let Some(place) = place {
                        self.require_active_payload_guards(function, block, place, state);
                    }
                }
            }
            MirInstruction::Store(store) => {
                self.require_active_payload_guards(function, block, &store.destination, state);
            }
            MirInstruction::Initialize(initialize) => {
                self.require_guarded_nested_destination(
                    function,
                    block,
                    &initialize.destination,
                    state,
                );
            }
            MirInstruction::CopyConstruct(copy) => {
                self.require_active_payload_guards(function, block, &copy.source, state);
                self.require_guarded_nested_destination(function, block, &copy.destination, state);
            }
            MirInstruction::CopyAssign(copy) => {
                self.require_active_payload_guards(function, block, &copy.source, state);
                self.require_active_payload_guards(function, block, &copy.destination, state);
            }
            MirInstruction::BindCheckedView(binding) => {
                self.require_active_payload_guards(function, block, &binding.view.source, state);
            }
            MirInstruction::EndOptionalView(end) => {
                self.require_active_payload_guards(function, block, &end.source, state);
            }
            _ => {}
        }
    }

    fn require_guarded_nested_destination(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        place: &MirPlace,
        state: &OptionalGuardState,
    ) {
        if place
            .projections
            .iter()
            .enumerate()
            .any(|(index, projection)| {
                matches!(
                    projection,
                    crate::mir::MirPlaceProjection::OptionalPayload(_)
                ) && index + 1 < place.projections.len()
            })
        {
            self.require_active_payload_guards(function, block, place, state);
        }
    }

    fn require_active_payload_guards(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        place: &MirPlace,
        state: &OptionalGuardState,
    ) {
        let mut root = MirPlace {
            base: place.base,
            projections: Vec::new(),
        };
        for projection in &place.projections {
            if let crate::mir::MirPlaceProjection::OptionalPayload(class) = projection {
                if !state
                    .active
                    .values()
                    .any(|(source, guarded_class)| source == &root && guarded_class == class)
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "optional payload place is used without its matching active guard",
                    );
                }
            }
            root.projections.push(*projection);
        }
    }

    fn verify_optional_place(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        place: &MirPlace,
    ) -> Option<MirPrimitiveType> {
        self.verify_optional_storage(function, block, place)
    }

    fn verify_optional_storage(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        place: &MirPlace,
    ) -> Option<MirPrimitiveType> {
        let verified = self.verify_place(function, block, place);
        let Some(MirType::OptionalPrimitive(payload)) = verified.map(|place| place.ty) else {
            self.block_error(
                function.callable(),
                block.id,
                "optional operation place is not primitive optional storage",
            );
            return None;
        };
        Some(payload)
    }

    fn verify_optional_source(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        source: &MirOptionalSource,
        expected: Option<MirPrimitiveType>,
        defined: &HashSet<ValueId>,
    ) {
        let actual = match source {
            MirOptionalSource::Absent => expected,
            MirOptionalSource::Present(value) => self
                .verify_value_use(function, block, *value, defined)
                .and_then(primitive_from_type),
            MirOptionalSource::Copy(place) => self.verify_optional_storage(function, block, place),
        };
        if expected.is_some() && actual.is_some() && expected != actual {
            self.block_error(
                function.callable(),
                block.id,
                "optional source payload type does not match its destination",
            );
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct OptionalGuardState {
    active: BTreeMap<OptionalGuardId, (MirPlace, crate::identity::ClassId)>,
    order: Vec<OptionalGuardId>,
    mutation_permits: HashSet<MirPlace>,
}

fn merge_optional_guard_state(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    target: crate::mir::BlockId,
    state: &OptionalGuardState,
    incoming: &mut [Option<OptionalGuardState>],
    pending: &mut VecDeque<crate::mir::BlockId>,
    reported_joins: &mut HashSet<crate::mir::BlockId>,
) {
    if target.callable() != function.callable() || target.index() >= incoming.len() {
        return;
    }
    match &incoming[target.index()] {
        None => {
            incoming[target.index()] = Some(state.clone());
            pending.push_back(target);
        }
        Some(existing) if existing != state => {
            if reported_joins.insert(target) {
                verifier.block_error(
                    function.callable(),
                    target,
                    "optional guard state differs across control-flow paths",
                );
            }
        }
        Some(_) => {}
    }
}

fn initialized_at_entry(function: MirDefinitionRef<'_>) -> HashSet<MirPlace> {
    let mut state = function
        .parameters()
        .iter()
        .filter(|storage| {
            function.storage(**storage).is_some_and(|storage| {
                matches!(
                    storage.ty,
                    MirType::OptionalPrimitive(_)
                        | MirType::OptionalClass(_)
                        | MirType::OptionalShared(_)
                )
            })
        })
        .map(|storage| {
            if function
                .storage(*storage)
                .is_some_and(|storage| matches!(storage.kind, MirStorageKind::AliasParameter(_)))
            {
                MirPlace::alias_parameter(*storage)
            } else {
                MirPlace::base(*storage)
            }
        })
        .collect::<HashSet<_>>();

    if matches!(
        function.callable(),
        crate::identity::CallableId::Method(_)
            | crate::identity::CallableId::Destructor(_)
            | crate::identity::CallableId::CopyAssignment(_)
    ) {
        for block in &function.body().blocks {
            for instruction in &block.instructions {
                match instruction {
                    MirInstruction::OptionalInitialize(initialize) => {
                        seed_projected(&mut state, &initialize.destination);
                        if let MirOptionalSource::Copy(source) = &initialize.source {
                            seed_projected(&mut state, source);
                        }
                    }
                    MirInstruction::OptionalAssign(assignment) => {
                        seed_projected(&mut state, &assignment.destination);
                        if let MirOptionalSource::Copy(source) = &assignment.source {
                            seed_projected(&mut state, source);
                        }
                    }
                    MirInstruction::OptionalSharedInitialize(initialize) => {
                        seed_projected(&mut state, &initialize.destination);
                        if let MirOptionalSharedSource::Copy(source) = &initialize.source {
                            seed_projected(&mut state, source);
                        }
                    }
                    MirInstruction::OptionalSharedAssign(assignment) => {
                        seed_projected(&mut state, &assignment.destination);
                        if let MirOptionalSharedSource::Copy(source) = &assignment.source {
                            seed_projected(&mut state, source);
                        }
                    }
                    MirInstruction::ClassOptionalInitialize(initialize) => {
                        seed_projected(&mut state, &initialize.destination);
                        if let crate::mir::MirClassOptionalSource::Copy(source) = &initialize.source
                        {
                            seed_projected(&mut state, source);
                        }
                    }
                    MirInstruction::ClassOptionalAssign(assignment) => {
                        seed_projected(&mut state, &assignment.destination);
                        if let crate::mir::MirClassOptionalSource::Copy(source) = &assignment.source
                        {
                            seed_projected(&mut state, source);
                        }
                    }
                    MirInstruction::Assign(assignment) => {
                        if let MirRvalueKind::OptionalPresence { source, .. } =
                            &assignment.rvalue.kind
                        {
                            seed_projected(&mut state, source);
                        }
                    }
                    _ => {}
                }
            }
            if let Some(MirTerminator::OptionalUnwrap { source, .. }) = &block.terminator {
                seed_projected(&mut state, source);
            }
            if let Some(MirTerminator::OptionalSharedUnwrap { unwrap, .. }) = &block.terminator {
                seed_projected(&mut state, &unwrap.source);
            }
        }
    }
    state
}

fn seed_projected(state: &mut HashSet<MirPlace>, place: &MirPlace) {
    if !place.projections.is_empty() {
        state.insert(place.clone());
    }
}

fn apply_initializations(
    program: &MirProgram,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    state: &mut HashSet<MirPlace>,
) {
    for instruction in &block.instructions {
        match instruction {
            MirInstruction::OptionalInitialize(initialize) => {
                state.insert(initialize.destination.clone());
            }
            MirInstruction::ClassOptionalInitialize(initialize) => {
                state.insert(initialize.destination.clone());
            }
            MirInstruction::OptionalSharedInitialize(initialize) => {
                state.insert(initialize.destination.clone());
            }
            MirInstruction::Call(call) => {
                if let Some(result) = call.shared_result {
                    if function
                        .storage(result)
                        .is_some_and(|storage| matches!(storage.ty, MirType::OptionalShared(_)))
                    {
                        state.insert(MirPlace::base(result));
                    }
                }
                if let Some(destination) = &call.destination {
                    if function
                        .storage(destination.base.storage())
                        .is_some_and(|storage| {
                            matches!(
                                storage.ty,
                                MirType::OptionalPrimitive(_)
                                    | MirType::OptionalClass(_)
                                    | MirType::OptionalShared(_)
                            )
                        })
                    {
                        state.insert(destination.clone());
                    } else if let Some(class) = complete_class_storage(function, destination) {
                        initialize_optional_fields(program, class, destination, state);
                    }
                }
            }
            MirInstruction::Initialize(initialize) => initialize_optional_fields(
                program,
                initialize.target.class(),
                &initialize.destination,
                state,
            ),
            MirInstruction::CopyConstruct(copy) => {
                initialize_optional_fields(program, copy.class, &copy.destination, state)
            }
            _ => {}
        }
    }
}

fn complete_class_storage(
    function: MirDefinitionRef<'_>,
    place: &MirPlace,
) -> Option<crate::identity::ClassId> {
    place
        .projections
        .is_empty()
        .then(|| function.storage(place.base.storage()))
        .flatten()
        .and_then(|storage| match storage.ty {
            MirType::Class(class) => Some(class),
            _ => None,
        })
}

fn initialize_optional_fields(
    program: &MirProgram,
    class: crate::identity::ClassId,
    root: &MirPlace,
    state: &mut HashSet<MirPlace>,
) {
    initialize_optional_fields_inner(program, class, root, state, &mut HashSet::new());
}

fn initialize_optional_fields_inner(
    program: &MirProgram,
    class: crate::identity::ClassId,
    root: &MirPlace,
    state: &mut HashSet<MirPlace>,
    visiting: &mut HashSet<crate::identity::ClassId>,
) {
    if !visiting.insert(class) {
        return;
    }
    if let Some(base) = program.direct_base(class) {
        initialize_optional_fields_inner(
            program,
            base,
            &root.clone().project_base(base),
            state,
            visiting,
        );
    }
    let Some(declaration) = program.class(class) else {
        visiting.remove(&class);
        return;
    };
    for field in &declaration.fields {
        let place = root.clone().project_field(field.id);
        match field.ty {
            MirType::OptionalPrimitive(_)
            | MirType::OptionalClass(_)
            | MirType::OptionalShared(_) => {
                state.insert(place);
            }
            MirType::Class(nested) => {
                initialize_optional_fields_inner(program, nested, &place, state, visiting)
            }
            _ => {}
        }
    }
    visiting.remove(&class);
}

fn require_initialized_source(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    source: &MirOptionalSource,
    state: &HashSet<MirPlace>,
) {
    if let MirOptionalSource::Copy(place) = source {
        require_initialized(
            verifier,
            function,
            block,
            place,
            state,
            "optional copy source",
        );
    }
}

fn require_initialized_class_source(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    source: &crate::mir::MirClassOptionalSource,
    state: &HashSet<MirPlace>,
) {
    if let crate::mir::MirClassOptionalSource::Copy(place) = source {
        require_initialized(
            verifier,
            function,
            block,
            place,
            state,
            "class optional copy source",
        );
    }
}

fn require_initialized_optional_shared_source(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    source: &MirOptionalSharedSource,
    state: &HashSet<MirPlace>,
) {
    let place = match source {
        MirOptionalSharedSource::Copy(place) => Some(place.clone()),
        MirOptionalSharedSource::Move(owner) => Some(MirPlace::base(*owner)),
        MirOptionalSharedSource::Absent | MirOptionalSharedSource::Present(_) => None,
    };
    if let Some(place) = place {
        require_initialized(
            verifier,
            function,
            block,
            &place,
            state,
            "optional shared copy source",
        );
    }
}

fn require_initialized(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    place: &MirPlace,
    state: &HashSet<MirPlace>,
    context: &'static str,
) {
    let complete_external_object = !place.projections.is_empty()
        && matches!(
            place.base,
            crate::mir::MirPlaceBase::SharedPointee(_)
                | crate::mir::MirPlaceBase::AliasParameter(_)
                | crate::mir::MirPlaceBase::CheckedView(_)
        );
    if !state.contains(place) && !complete_external_object {
        verifier.block_error(
            function.callable(),
            block.id,
            format!("{context} is not definitely initialized"),
        );
    }
}

fn primitive_from_type(ty: MirType) -> Option<MirPrimitiveType> {
    match ty {
        MirType::I64 => Some(MirPrimitiveType::I64),
        MirType::U64 => Some(MirPrimitiveType::U64),
        MirType::U8 => Some(MirPrimitiveType::U8),
        MirType::F64 => Some(MirPrimitiveType::F64),
        MirType::Bool => Some(MirPrimitiveType::Bool),
        _ => None,
    }
}
