//! Path-sensitive definite initialization for optional storage.

use std::collections::{HashMap, HashSet};

use super::super::{
    super::model::{
        MirArgument, MirBasicBlock, MirDefinitionRef, MirInstruction, MirOptionalSharedSource,
        MirOptionalSource, MirPlace, MirProgram, MirRvalueKind, MirStorageKind, MirTerminator,
        MirType,
    },
    context::Verifier,
    dataflow::ForwardDataflow,
    path_state::{condition_reads, PathStates},
};

impl Verifier<'_> {
    pub(in crate::mir::verify) fn verify_optional_initialization(
        &mut self,
        function: MirDefinitionRef<'_>,
    ) {
        let entry_state = initialized_at_entry(function);
        let activation_conditions: HashMap<_, _> = function
            .path_conditions()
            .iter()
            .map(|condition| (condition.activation, condition.id))
            .collect();
        let condition_reads = condition_reads(function);
        let mut flow = ForwardDataflow::new(function.callable(), function.body().blocks.len());
        flow.seed(
            function.body().entry,
            PathStates::initial(entry_state.clone()),
        );
        loop {
            while let Some((block_id, mut states)) = flow.pop() {
                let Some(block) = function.block(block_id) else {
                    continue;
                };
                for state in states.states_mut() {
                    apply_initializations(self.program, function, block, state);
                }
                collapse_conditions_at_storage_death(&mut states, block, &activation_conditions);
                if let Some(MirTerminator::Branch {
                    condition,
                    true_target,
                    false_target,
                    ..
                }) = &block.terminator
                {
                    if let Some(path_condition) = condition_reads.get(condition).copied() {
                        for (target, active) in [(*true_target, true), (*false_target, false)] {
                            let (selected, _) = states.select(path_condition, active);
                            merge_initialization_states(
                                function, block.id, target, &selected, &mut flow,
                            );
                        }
                        continue;
                    }
                }
                for successor in block.terminator.iter().flat_map(MirTerminator::successors) {
                    merge_initialization_states(function, block.id, successor, &states, &mut flow);
                }
            }
            if !flow.seed_next_component(
                &function.body().blocks,
                PathStates::initial(entry_state.clone()),
            ) {
                break;
            }
        }

        for block in &function.body().blocks {
            let Some(mut states) = flow.state(block.id).cloned() else {
                continue;
            };
            for instruction in &block.instructions {
                for state in states.states_mut() {
                    match instruction {
                        MirInstruction::StorageLive(operation) => {
                            reset_storage_places(state, operation.storage);
                        }
                        MirInstruction::StorageDead(operation) => {
                            require_finished_owned_optional_storage(
                                self,
                                function,
                                block,
                                operation.storage,
                                state,
                            );
                            reset_storage_places(state, operation.storage);
                        }
                        MirInstruction::OptionalInitialize(initialize) => {
                            require_initialized_source(
                                self,
                                function,
                                block,
                                &initialize.source,
                                state,
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
                                state,
                                "optional assignment destination",
                            );
                            require_initialized_source(
                                self,
                                function,
                                block,
                                &assignment.source,
                                state,
                            );
                        }
                        MirInstruction::OptionalSharedInitialize(initialize) => {
                            require_initialized_optional_shared_source(
                                self,
                                function,
                                block,
                                &initialize.source,
                                state,
                            );
                            consume_moved_optional_shared_source(&initialize.source, state);
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
                                state,
                                "optional shared assignment destination",
                            );
                            require_initialized_optional_shared_source(
                                self,
                                function,
                                block,
                                &assignment.source,
                                state,
                            );
                            consume_moved_optional_shared_source(&assignment.source, state);
                        }
                        MirInstruction::OptionalSharedCleanup(cleanup) => {
                            require_initialized(
                                self,
                                function,
                                block,
                                &cleanup.destination,
                                state,
                                "optional shared cleanup destination",
                            );
                            state.remove(&cleanup.destination);
                        }
                        MirInstruction::ClassOptionalInitialize(initialize) => {
                            require_initialized_class_source(
                                self,
                                function,
                                block,
                                &initialize.source,
                                state,
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
                                state,
                                "class optional assignment destination",
                            );
                            require_initialized_class_source(
                                self,
                                function,
                                block,
                                &assignment.source,
                                state,
                            );
                        }
                        MirInstruction::ClassOptionalCleanup(cleanup) => {
                            require_initialized(
                                self,
                                function,
                                block,
                                &cleanup.destination,
                                state,
                                "class optional cleanup destination",
                            );
                            state.remove(&cleanup.destination);
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
                                    state,
                                    "optional presence-test source",
                                );
                            }
                        }
                        MirInstruction::Call(call) => {
                            consume_class_optional_arguments(
                                self,
                                function,
                                block,
                                &call.arguments,
                                state,
                            );
                            consume_optional_shared_arguments(
                                self,
                                function,
                                block,
                                &call.arguments,
                                state,
                            );
                            if let Some(result) = call.shared_result {
                                if function.storage(result).is_some_and(|storage| {
                                    matches!(storage.ty, MirType::OptionalShared(_))
                                }) {
                                    state.insert(MirPlace::base(result));
                                }
                            }
                            if let Some(destination) = &call.destination {
                                if function.storage(destination.base.storage()).is_some_and(
                                    |storage| {
                                        matches!(
                                            storage.ty,
                                            MirType::OptionalPrimitive(_)
                                                | MirType::OptionalClass(_)
                                                | MirType::OptionalShared(_)
                                        )
                                    },
                                ) && !state.insert(destination.clone())
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
                                        state,
                                    );
                                }
                            }
                        }
                        MirInstruction::Initialize(initialize) => {
                            consume_class_optional_arguments(
                                self,
                                function,
                                block,
                                &initialize.arguments,
                                state,
                            );
                            consume_optional_shared_arguments(
                                self,
                                function,
                                block,
                                &initialize.arguments,
                                state,
                            );
                            initialize_optional_fields(
                                self.program,
                                initialize.target.class(),
                                &initialize.destination,
                                state,
                            );
                        }
                        MirInstruction::SharedInitialize(initialize) => {
                            consume_class_optional_arguments(
                                self,
                                function,
                                block,
                                &initialize.arguments,
                                state,
                            );
                            consume_optional_shared_arguments(
                                self,
                                function,
                                block,
                                &initialize.arguments,
                                state,
                            );
                        }
                        MirInstruction::CopyConstruct(copy) => initialize_optional_fields(
                            self.program,
                            copy.class,
                            &copy.destination,
                            state,
                        ),
                        _ => {}
                    }
                }
                end_condition_at_storage_death(
                    self,
                    function,
                    block,
                    instruction,
                    &activation_conditions,
                    &mut states,
                );
            }
            for state in states.states_mut() {
                if let Some(MirTerminator::OptionalUnwrap { source, .. }) = &block.terminator {
                    require_initialized(
                        self,
                        function,
                        block,
                        source,
                        state,
                        "optional unwrap source",
                    );
                }
                if let Some(MirTerminator::OptionalSharedUnwrap { unwrap, .. }) = &block.terminator
                {
                    require_initialized(
                        self,
                        function,
                        block,
                        &unwrap.source,
                        state,
                        "optional shared unwrap source",
                    );
                }
                match &block.terminator {
                    Some(MirTerminator::BeginOptionalView { begin, .. }) => require_initialized(
                        self,
                        function,
                        block,
                        &begin.source,
                        state,
                        "optional-view source",
                    ),
                    Some(MirTerminator::CheckOptionalMutation { source, .. }) => {
                        require_initialized(
                            self,
                            function,
                            block,
                            source,
                            state,
                            "optional mutation-check source",
                        )
                    }
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
                                state,
                                "optional return destination",
                            );
                        }
                    }
                }
            }
        }
    }
}

fn merge_initialization_states(
    function: MirDefinitionRef<'_>,
    predecessor: crate::mir::BlockId,
    target: crate::mir::BlockId,
    states: &PathStates<HashSet<MirPlace>>,
    flow: &mut ForwardDataflow<PathStates<HashSet<MirPlace>>>,
) {
    if states.is_empty() {
        return;
    }
    let selected = states
        .on_edge(function, predecessor, target)
        .unwrap_or_else(|_| states.clone());
    flow.merge(target, &selected, |existing, incoming| {
        existing.merge(incoming, merge_definitely_initialized)
    });
}

fn merge_definitely_initialized(existing: &mut HashSet<MirPlace>, incoming: &HashSet<MirPlace>) {
    existing.retain(|place| incoming.contains(place));
}

fn collapse_conditions_at_storage_death(
    states: &mut PathStates<HashSet<MirPlace>>,
    block: &MirBasicBlock,
    activation_conditions: &HashMap<crate::mir::StorageId, crate::mir::PathConditionId>,
) {
    for instruction in &block.instructions {
        let MirInstruction::StorageDead(operation) = instruction else {
            continue;
        };
        let Some(condition) = activation_conditions.get(&operation.storage).copied() else {
            continue;
        };
        states.end_condition(condition, |existing, incoming| {
            merge_definitely_initialized(existing, incoming);
        });
    }
}

fn end_condition_at_storage_death(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    instruction: &MirInstruction,
    activation_conditions: &HashMap<crate::mir::StorageId, crate::mir::PathConditionId>,
    states: &mut PathStates<HashSet<MirPlace>>,
) {
    let MirInstruction::StorageDead(operation) = instruction else {
        return;
    };
    let Some(condition) = activation_conditions.get(&operation.storage).copied() else {
        return;
    };
    let mut incompatible = false;
    let missing = states.end_condition(condition, |existing, incoming| {
        incompatible = true;
        merge_definitely_initialized(existing, incoming);
    });
    if incompatible {
        verifier.block_error(
            function.callable(),
            block.id,
            format!(
                "conditional optional initialization state remains when path condition {condition} ends"
            ),
        );
    }
    if missing {
        verifier.block_error(
            function.callable(),
            block.id,
            format!(
                "path condition {condition} ends outside its selected optional-initialization region"
            ),
        );
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
            MirInstruction::StorageLive(operation) => {
                reset_storage_places(state, operation.storage);
            }
            MirInstruction::StorageDead(operation) => {
                reset_storage_places(state, operation.storage);
            }
            MirInstruction::OptionalInitialize(initialize) => {
                state.insert(initialize.destination.clone());
            }
            MirInstruction::ClassOptionalInitialize(initialize) => {
                state.insert(initialize.destination.clone());
            }
            MirInstruction::ClassOptionalCleanup(cleanup) => {
                state.remove(&cleanup.destination);
            }
            MirInstruction::OptionalSharedInitialize(initialize) => {
                consume_moved_optional_shared_source(&initialize.source, state);
                state.insert(initialize.destination.clone());
            }
            MirInstruction::OptionalSharedAssign(assignment) => {
                consume_moved_optional_shared_source(&assignment.source, state);
            }
            MirInstruction::OptionalSharedCleanup(cleanup) => {
                state.remove(&cleanup.destination);
            }
            MirInstruction::Call(call) => {
                transfer_class_optional_arguments(function, &call.arguments, state);
                transfer_optional_shared_arguments(function, &call.arguments, state);
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
            MirInstruction::Initialize(initialize) => {
                transfer_class_optional_arguments(function, &initialize.arguments, state);
                transfer_optional_shared_arguments(function, &initialize.arguments, state);
                initialize_optional_fields(
                    program,
                    initialize.target.class(),
                    &initialize.destination,
                    state,
                );
            }
            MirInstruction::SharedInitialize(initialize) => {
                transfer_class_optional_arguments(function, &initialize.arguments, state);
                transfer_optional_shared_arguments(function, &initialize.arguments, state);
            }
            MirInstruction::CopyConstruct(copy) => {
                initialize_optional_fields(program, copy.class, &copy.destination, state)
            }
            _ => {}
        }
    }
}

fn reset_storage_places(state: &mut HashSet<MirPlace>, storage: crate::mir::StorageId) {
    state.retain(|place| place.base.storage() != storage);
}

fn require_finished_owned_optional_storage(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    storage: crate::mir::StorageId,
    state: &HashSet<MirPlace>,
) {
    if !state.contains(&MirPlace::base(storage)) {
        return;
    }
    let message = match function.storage(storage).map(|storage| storage.ty) {
        Some(MirType::OptionalClass(_)) => {
            "initialized class optional reaches storage-dead without cleanup or ownership transfer"
        }
        Some(MirType::OptionalShared(_)) => {
            "initialized optional shared reaches storage-dead without cleanup or ownership transfer"
        }
        _ => return,
    };
    verifier.block_error(function.callable(), block.id, message);
}

fn consume_class_optional_arguments(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    arguments: &[MirArgument],
    state: &mut HashSet<MirPlace>,
) {
    for argument in arguments {
        let MirArgument::OwnedPlace(place) = argument else {
            continue;
        };
        if !function
            .storage(place.base.storage())
            .is_some_and(|storage| matches!(storage.ty, MirType::OptionalClass(_)))
        {
            continue;
        }
        require_initialized(
            verifier,
            function,
            block,
            place,
            state,
            "class optional value argument",
        );
        state.remove(place);
    }
}

fn consume_optional_shared_arguments(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    arguments: &[MirArgument],
    state: &mut HashSet<MirPlace>,
) {
    for argument in arguments {
        let MirArgument::SharedOwner(storage) = argument else {
            continue;
        };
        if !function
            .storage(*storage)
            .is_some_and(|entry| matches!(entry.ty, MirType::OptionalShared(_)))
        {
            continue;
        }
        let place = MirPlace::base(*storage);
        require_initialized(
            verifier,
            function,
            block,
            &place,
            state,
            "optional shared value argument",
        );
        state.remove(&place);
    }
}

fn transfer_class_optional_arguments(
    function: MirDefinitionRef<'_>,
    arguments: &[MirArgument],
    state: &mut HashSet<MirPlace>,
) {
    for argument in arguments {
        let MirArgument::OwnedPlace(place) = argument else {
            continue;
        };
        if function
            .storage(place.base.storage())
            .is_some_and(|storage| matches!(storage.ty, MirType::OptionalClass(_)))
        {
            state.remove(place);
        }
    }
}

fn transfer_optional_shared_arguments(
    function: MirDefinitionRef<'_>,
    arguments: &[MirArgument],
    state: &mut HashSet<MirPlace>,
) {
    for argument in arguments {
        let MirArgument::SharedOwner(storage) = argument else {
            continue;
        };
        if function
            .storage(*storage)
            .is_some_and(|entry| matches!(entry.ty, MirType::OptionalShared(_)))
        {
            state.remove(&MirPlace::base(*storage));
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

fn consume_moved_optional_shared_source(
    source: &MirOptionalSharedSource,
    state: &mut HashSet<MirPlace>,
) {
    if let MirOptionalSharedSource::Move(storage) = source {
        state.remove(&MirPlace::base(*storage));
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
    let array_element = place.projections.iter().any(|projection| {
        matches!(
            projection,
            crate::mir::MirPlaceProjection::ArrayElement { .. }
        )
    });
    if !state.contains(place) && !complete_external_object && !array_element {
        verifier.block_error(
            function.callable(),
            block.id,
            format!("{context} is not definitely initialized"),
        );
    }
}
