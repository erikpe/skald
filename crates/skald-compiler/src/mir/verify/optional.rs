//! Primitive optional structure and definite-initialization verification.

use std::collections::{HashSet, VecDeque};

use super::{
    super::model::{
        MirBasicBlock, MirDefinitionRef, MirInstruction, MirOptionalSource, MirPlace,
        MirPrimitiveType, MirProgram, MirRvalueKind, MirStorageKind, MirTerminator, MirType,
        StorageId, ValueId,
    },
    context::Verifier,
};

impl Verifier<'_> {
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
        self.verify_optional_storage(function, block, source);
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
                        if let Some(destination) = &call.destination {
                            if function
                                .storage(destination.base.storage())
                                .is_some_and(|storage| {
                                    matches!(storage.ty, MirType::OptionalPrimitive(_))
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
            if matches!(block.terminator, Some(MirTerminator::Return { .. })) {
                if let Some(return_storage) = function.return_storage() {
                    let place = MirPlace::base(return_storage);
                    if function
                        .storage(return_storage)
                        .is_some_and(|storage| matches!(storage.ty, MirType::OptionalPrimitive(_)))
                    {
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

fn initialized_at_entry(function: MirDefinitionRef<'_>) -> HashSet<MirPlace> {
    let mut state = function
        .parameters()
        .iter()
        .filter(|storage| {
            function
                .storage(**storage)
                .is_some_and(|storage| matches!(storage.ty, MirType::OptionalPrimitive(_)))
        })
        .map(|storage| MirPlace::base(*storage))
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
            MirInstruction::Call(call) => {
                if let Some(destination) = &call.destination {
                    if function
                        .storage(destination.base.storage())
                        .is_some_and(|storage| matches!(storage.ty, MirType::OptionalPrimitive(_)))
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
            MirType::OptionalPrimitive(_) => {
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

fn require_initialized(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    place: &MirPlace,
    state: &HashSet<MirPlace>,
    context: &'static str,
) {
    if !state.contains(place) {
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
