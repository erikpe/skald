use crate::mir::{
    MirArgument, MirBasicBlock, MirClassOptionalSource, MirDefinitionRef, MirInstruction,
    MirOptionalSharedSource, MirOptionalSource, MirPlace, MirPlaceBase, MirPlaceProjection,
    MirRvalueKind, MirTerminator, MirType, StorageId,
};

use super::{
    super::super::{context::Verifier, path_state::PathStates},
    flow::Analysis,
    state::{is_optional, InitializationState},
};

pub(super) fn verify(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    analysis: &Analysis,
) {
    for block in &function.body().blocks {
        let Some(mut states) = analysis.state(block.id).cloned() else {
            continue;
        };
        for instruction in &block.instructions {
            states.update_states(|state| {
                verify_instruction(verifier, function, block, instruction, state);
            });
            end_condition_at_storage_death(
                verifier,
                function,
                block,
                instruction,
                analysis,
                &mut states,
            );
        }
        states.update_states(|state| verify_terminator(verifier, function, block, state));
    }
}

fn verify_instruction(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    instruction: &MirInstruction,
    state: &mut InitializationState,
) {
    match instruction {
        MirInstruction::StorageLive(operation) => {
            state.reset_storage(operation.storage);
        }
        MirInstruction::StorageDead(operation) => {
            require_finished_owned_optional_storage(
                verifier,
                function,
                block,
                operation.storage,
                state,
            );
            state.reset_storage(operation.storage);
        }
        MirInstruction::OptionalInitialize(initialize) => {
            require_initialized_source(verifier, function, block, &initialize.source, state);
            if !state.insert(initialize.destination.clone()) {
                verifier.block_error(
                    function.callable(),
                    block.id,
                    "optional storage is initialized more than once",
                );
            }
        }
        MirInstruction::OptionalAssign(assignment) => {
            require_initialized(
                verifier,
                function,
                block,
                &assignment.destination,
                state,
                "optional assignment destination",
            );
            require_initialized_source(verifier, function, block, &assignment.source, state);
        }
        MirInstruction::OptionalSharedInitialize(initialize) => {
            require_initialized_optional_shared_source(
                verifier,
                function,
                block,
                &initialize.source,
                state,
            );
            state.consume_moved_optional_shared_source(&initialize.source);
            if !state.insert(initialize.destination.clone()) {
                verifier.block_error(
                    function.callable(),
                    block.id,
                    "optional shared storage is initialized more than once",
                );
            }
        }
        MirInstruction::OptionalSharedAssign(assignment) => {
            require_initialized(
                verifier,
                function,
                block,
                &assignment.destination,
                state,
                "optional shared assignment destination",
            );
            require_initialized_optional_shared_source(
                verifier,
                function,
                block,
                &assignment.source,
                state,
            );
            state.consume_moved_optional_shared_source(&assignment.source);
        }
        MirInstruction::OptionalSharedCleanup(cleanup) => {
            require_initialized(
                verifier,
                function,
                block,
                &cleanup.destination,
                state,
                "optional shared cleanup destination",
            );
            state.remove(&cleanup.destination);
        }
        MirInstruction::ClassOptionalInitialize(initialize) => {
            require_initialized_class_source(verifier, function, block, &initialize.source, state);
            if !state.insert(initialize.destination.clone()) {
                verifier.block_error(
                    function.callable(),
                    block.id,
                    "class optional storage is initialized more than once",
                );
            }
        }
        MirInstruction::ClassOptionalAssign(assignment) => {
            require_initialized(
                verifier,
                function,
                block,
                &assignment.destination,
                state,
                "class optional assignment destination",
            );
            require_initialized_class_source(verifier, function, block, &assignment.source, state);
        }
        MirInstruction::ClassOptionalCleanup(cleanup) => {
            require_initialized(
                verifier,
                function,
                block,
                &cleanup.destination,
                state,
                "class optional cleanup destination",
            );
            state.remove(&cleanup.destination);
        }
        MirInstruction::Assign(assignment) => {
            if let MirRvalueKind::OptionalPresence { source, .. } = &assignment.rvalue.kind {
                require_initialized(
                    verifier,
                    function,
                    block,
                    source,
                    state,
                    "optional presence-test source",
                );
            }
        }
        MirInstruction::Call(call) => {
            consume_class_optional_arguments(verifier, function, block, &call.arguments, state);
            consume_optional_shared_arguments(verifier, function, block, &call.arguments, state);
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
                    .is_some_and(|storage| is_optional(storage.ty))
                {
                    if !state.insert(destination.clone()) {
                        verifier.block_error(
                            function.callable(),
                            block.id,
                            "call destination is already initialized",
                        );
                    }
                } else {
                    state.initialize_complete_class_storage(
                        verifier.program,
                        function,
                        destination,
                    );
                }
            }
        }
        MirInstruction::Initialize(initialize) => {
            consume_class_optional_arguments(
                verifier,
                function,
                block,
                &initialize.arguments,
                state,
            );
            consume_optional_shared_arguments(
                verifier,
                function,
                block,
                &initialize.arguments,
                state,
            );
            state.initialize_optional_fields(
                verifier.program,
                initialize.target.class(),
                &initialize.destination,
            );
        }
        MirInstruction::SharedInitialize(initialize) => {
            consume_class_optional_arguments(
                verifier,
                function,
                block,
                &initialize.arguments,
                state,
            );
            consume_optional_shared_arguments(
                verifier,
                function,
                block,
                &initialize.arguments,
                state,
            );
        }
        MirInstruction::CopyConstruct(copy) => {
            state.initialize_optional_fields(verifier.program, copy.class, &copy.destination);
        }
        _ => {}
    }
}

fn verify_terminator(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    state: &InitializationState,
) {
    if let Some(MirTerminator::OptionalUnwrap { source, .. }) = &block.terminator {
        require_initialized(
            verifier,
            function,
            block,
            source,
            state,
            "optional unwrap source",
        );
    }
    if let Some(MirTerminator::OptionalSharedUnwrap { unwrap, .. }) = &block.terminator {
        require_initialized(
            verifier,
            function,
            block,
            &unwrap.source,
            state,
            "optional shared unwrap source",
        );
    }
    match &block.terminator {
        Some(MirTerminator::BeginOptionalView { begin, .. }) => require_initialized(
            verifier,
            function,
            block,
            &begin.source,
            state,
            "optional-view source",
        ),
        Some(MirTerminator::CheckOptionalMutation { source, .. }) => require_initialized(
            verifier,
            function,
            block,
            source,
            state,
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
            if function
                .storage(return_storage)
                .is_some_and(|storage| is_optional(storage.ty))
            {
                require_initialized(
                    verifier,
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

fn end_condition_at_storage_death(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    instruction: &MirInstruction,
    analysis: &Analysis,
    states: &mut PathStates<InitializationState>,
) {
    let MirInstruction::StorageDead(operation) = instruction else {
        return;
    };
    let Some(condition) = analysis.activation_condition(operation.storage) else {
        return;
    };
    let mut incompatible = false;
    let missing = states.end_condition(condition, |existing, incoming| {
        incompatible = true;
        existing.merge(incoming);
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

fn require_finished_owned_optional_storage(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    storage: StorageId,
    state: &InitializationState,
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
    state: &mut InitializationState,
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
    state: &mut InitializationState,
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

fn require_initialized_source(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    source: &MirOptionalSource,
    state: &InitializationState,
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
    source: &MirClassOptionalSource,
    state: &InitializationState,
) {
    if let MirClassOptionalSource::Copy(place) = source {
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
    state: &InitializationState,
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
    state: &InitializationState,
    context: &'static str,
) {
    let complete_external_object = !place.projections.is_empty()
        && matches!(
            place.base,
            MirPlaceBase::SharedPointee(_)
                | MirPlaceBase::AliasParameter(_)
                | MirPlaceBase::CheckedView(_)
        );
    let array_element = place
        .projections
        .iter()
        .any(|projection| matches!(projection, MirPlaceProjection::ArrayElement { .. }));
    if !state.contains(place) && !complete_external_object && !array_element {
        verifier.block_error(
            function.callable(),
            block.id,
            format!("{context} is not definitely initialized"),
        );
    }
}
