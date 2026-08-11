use crate::mir::{
    MirArgument, MirBasicBlock, MirClassOptionalSource, MirDefinitionRef, MirInstruction,
    MirOptionalSharedSource, MirOptionalSource, MirPlace, MirPlaceBase, MirPlaceProjection,
    MirRvalueKind, MirTerminator, StorageId,
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
        MirInstruction::AggregateOptionalInitialize(initialize) => {
            if let crate::mir::MirAggregateOptionalSource::Copy(source) = &initialize.source {
                require_initialized(
                    verifier,
                    function,
                    block,
                    source,
                    state,
                    "nested optional copy source",
                );
            }
            let inserted = if matches!(
                initialize.source,
                crate::mir::MirAggregateOptionalSource::Unpublished
            ) {
                state.reserve(initialize.destination.clone())
            } else {
                state.insert(initialize.destination.clone())
            };
            if !inserted {
                verifier.block_error(
                    function.callable(),
                    block.id,
                    "nested optional storage is initialized more than once",
                );
            }
        }
        MirInstruction::AggregateOptionalAssign(assignment) => {
            require_initialized(
                verifier,
                function,
                block,
                &assignment.destination,
                state,
                "nested optional assignment destination",
            );
            if let crate::mir::MirAggregateOptionalSource::Copy(source) = &assignment.source {
                require_initialized(
                    verifier,
                    function,
                    block,
                    source,
                    state,
                    "nested optional copy source",
                );
            }
        }
        MirInstruction::AggregateOptionalPublish(publish) => {
            if !state.publish(&publish.destination) {
                verifier.block_error(
                    function.callable(),
                    block.id,
                    "nested optional publication requires one unpublished destination",
                );
            }
        }
        MirInstruction::AggregateOptionalCleanup(cleanup) => {
            require_initialized(
                verifier,
                function,
                block,
                &cleanup.destination,
                state,
                "nested optional cleanup destination",
            );
            state.remove(&cleanup.destination);
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
        MirInstruction::Array(crate::mir::MirArrayInstruction::CompleteElement {
            backing,
            prefix,
            ..
        }) => state.complete_array_element(*backing, *prefix),
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
                    .is_some_and(|storage| verifier.optional_shared(storage.ty).is_some())
                {
                    state.insert(MirPlace::base(result));
                }
            }
            if let Some(destination) = &call.destination {
                let destination_type = destination
                    .base
                    .local_storage()
                    .and_then(|storage| function.storage(storage))
                    .map(|storage| storage.ty)
                    .or_else(|| match destination.base {
                        MirPlaceBase::StaticField(field)
                        | MirPlaceBase::StaticLifecycleDestination(field) => {
                            verifier.program.static_field(field).map(|field| field.ty)
                        }
                        _ => None,
                    });
                if destination_type.is_some_and(is_optional) {
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
        incompatible |= !existing.has_same_owned_state(incoming);
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
    let ty = function.storage(storage).map(|storage| storage.ty);
    let message = if ty.is_some_and(|ty| verifier.optional_class(ty).is_some()) {
        "initialized class optional reaches storage-dead without cleanup or ownership transfer"
    } else if ty.is_some_and(|ty| verifier.optional_shared(ty).is_some()) {
        "initialized optional shared reaches storage-dead without cleanup or ownership transfer"
    } else {
        return;
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
            .storage(place.base.expect_local_storage())
            .is_some_and(|storage| verifier.optional_class(storage.ty).is_some())
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
            .is_some_and(|entry| verifier.optional_shared(entry.ty).is_some())
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
    let published_optional_box = place.projections.is_empty()
        && matches!(place.base, MirPlaceBase::SharedPointee(owner)
        if function.storage(owner).is_some_and(|storage| {
            matches!(storage.ty, crate::mir::MirType::Shared(
                crate::mir::MirSharedTarget::OptionalBox(_)
            ))
        }));
    let array_element = place
        .projections
        .iter()
        .any(|projection| matches!(projection, MirPlaceProjection::ArrayElement { .. }));
    if !state.contains(place)
        && !complete_external_object
        && !published_optional_box
        && !array_element
    {
        verifier.block_error(
            function.callable(),
            block.id,
            format!("{context} is not definitely initialized"),
        );
    }
}
