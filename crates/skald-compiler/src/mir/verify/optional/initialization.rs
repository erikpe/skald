//! Path-sensitive definite initialization for optional storage.

use std::collections::{HashSet, VecDeque};

use super::super::{
    super::model::{
        MirBasicBlock, MirDefinitionRef, MirInstruction, MirOptionalSharedSource,
        MirOptionalSource, MirPlace, MirProgram, MirRvalueKind, MirStorageKind, MirTerminator,
        MirType,
    },
    context::Verifier,
};

impl Verifier<'_> {
    pub(in crate::mir::verify) fn verify_optional_initialization(
        &mut self,
        function: MirDefinitionRef<'_>,
    ) {
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
