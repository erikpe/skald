use std::collections::HashSet;

use crate::{
    identity::{CallableId, ClassId},
    mir::{
        MirArgument, MirBasicBlock, MirClassOptionalSource, MirDefinitionRef, MirInstruction,
        MirOptionalSharedSource, MirOptionalSource, MirPlace, MirProgram, MirRvalueKind,
        MirStorageKind, MirTerminator, MirType, StorageId,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct InitializationState {
    places: HashSet<MirPlace>,
}

impl InitializationState {
    pub(super) fn at_entry(program: &MirProgram, function: MirDefinitionRef<'_>) -> Self {
        let static_optionals = program.classes.iter().flat_map(|class| {
            class
                .static_fields
                .iter()
                .filter(|field| {
                    matches!(
                        field.ty,
                        MirType::OptionalPrimitive(_) | MirType::OptionalClass(_)
                    )
                })
                .map(|field| MirPlace::static_field(field.id))
        });
        let mut state = Self {
            places: function
                .parameters()
                .iter()
                .filter(|storage| {
                    function
                        .storage(**storage)
                        .is_some_and(|storage| is_optional(storage.ty))
                })
                .map(|storage| {
                    if function.storage(*storage).is_some_and(|storage| {
                        matches!(storage.kind, MirStorageKind::AliasParameter(_))
                    }) {
                        MirPlace::alias_parameter(*storage)
                    } else {
                        MirPlace::base(*storage)
                    }
                })
                .chain(static_optionals)
                .collect(),
        };

        if matches!(
            function.callable(),
            CallableId::Method(_) | CallableId::Destructor(_) | CallableId::CopyAssignment(_)
        ) {
            state.seed_projected_uses(function);
        }
        state
    }

    pub(super) fn contains(&self, place: &MirPlace) -> bool {
        self.places.contains(place)
    }

    pub(super) fn insert(&mut self, place: MirPlace) -> bool {
        self.places.insert(place)
    }

    pub(super) fn remove(&mut self, place: &MirPlace) {
        self.places.remove(place);
    }

    pub(super) fn reset_storage(&mut self, storage: StorageId) {
        self.places
            .retain(|place| place.base.local_storage() != Some(storage));
    }

    pub(super) fn merge(&mut self, incoming: &Self) {
        self.places.retain(|place| incoming.places.contains(place));
    }

    pub(super) fn apply_block(
        &mut self,
        program: &MirProgram,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
    ) {
        for instruction in &block.instructions {
            match instruction {
                MirInstruction::StorageLive(operation) => {
                    self.reset_storage(operation.storage);
                }
                MirInstruction::StorageDead(operation) => {
                    self.reset_storage(operation.storage);
                }
                MirInstruction::OptionalInitialize(initialize) => {
                    self.insert(initialize.destination.clone());
                }
                MirInstruction::ClassOptionalInitialize(initialize) => {
                    self.insert(initialize.destination.clone());
                }
                MirInstruction::ClassOptionalCleanup(cleanup) => {
                    self.remove(&cleanup.destination);
                }
                MirInstruction::OptionalSharedInitialize(initialize) => {
                    self.consume_moved_optional_shared_source(&initialize.source);
                    self.insert(initialize.destination.clone());
                }
                MirInstruction::OptionalSharedAssign(assignment) => {
                    self.consume_moved_optional_shared_source(&assignment.source);
                }
                MirInstruction::OptionalSharedCleanup(cleanup) => {
                    self.remove(&cleanup.destination);
                }
                MirInstruction::Call(call) => {
                    self.transfer_class_optional_arguments(function, &call.arguments);
                    self.transfer_optional_shared_arguments(function, &call.arguments);
                    if let Some(result) = call.shared_result {
                        if function
                            .storage(result)
                            .is_some_and(|storage| matches!(storage.ty, MirType::OptionalShared(_)))
                        {
                            self.insert(MirPlace::base(result));
                        }
                    }
                    if let Some(destination) = &call.destination {
                        if function
                            .storage(destination.base.expect_local_storage())
                            .is_some_and(|storage| is_optional(storage.ty))
                        {
                            self.insert(destination.clone());
                        } else {
                            self.initialize_complete_class_storage(program, function, destination);
                        }
                    }
                }
                MirInstruction::Initialize(initialize) => {
                    self.transfer_class_optional_arguments(function, &initialize.arguments);
                    self.transfer_optional_shared_arguments(function, &initialize.arguments);
                    self.initialize_optional_fields(
                        program,
                        initialize.target.class(),
                        &initialize.destination,
                    );
                }
                MirInstruction::SharedInitialize(initialize) => {
                    self.transfer_class_optional_arguments(function, &initialize.arguments);
                    self.transfer_optional_shared_arguments(function, &initialize.arguments);
                }
                MirInstruction::CopyConstruct(copy) => {
                    self.initialize_optional_fields(program, copy.class, &copy.destination);
                }
                _ => {}
            }
        }
    }

    pub(super) fn consume_moved_optional_shared_source(
        &mut self,
        source: &MirOptionalSharedSource,
    ) {
        if let MirOptionalSharedSource::Move(storage) = source {
            self.remove(&MirPlace::base(*storage));
        }
    }

    pub(super) fn initialize_complete_class_storage(
        &mut self,
        program: &MirProgram,
        function: MirDefinitionRef<'_>,
        place: &MirPlace,
    ) {
        let Some(class) = complete_class_storage(function, place) else {
            return;
        };
        self.initialize_optional_fields(program, class, place);
    }

    pub(super) fn initialize_optional_fields(
        &mut self,
        program: &MirProgram,
        class: ClassId,
        root: &MirPlace,
    ) {
        self.initialize_optional_fields_inner(program, class, root, &mut HashSet::new());
    }

    fn seed_projected_uses(&mut self, function: MirDefinitionRef<'_>) {
        for block in &function.body().blocks {
            for instruction in &block.instructions {
                match instruction {
                    MirInstruction::OptionalInitialize(initialize) => {
                        self.seed_projected(&initialize.destination);
                        if let MirOptionalSource::Copy(source) = &initialize.source {
                            self.seed_projected(source);
                        }
                    }
                    MirInstruction::OptionalAssign(assignment) => {
                        self.seed_projected(&assignment.destination);
                        if let MirOptionalSource::Copy(source) = &assignment.source {
                            self.seed_projected(source);
                        }
                    }
                    MirInstruction::OptionalSharedInitialize(initialize) => {
                        self.seed_projected(&initialize.destination);
                        if let MirOptionalSharedSource::Copy(source) = &initialize.source {
                            self.seed_projected(source);
                        }
                    }
                    MirInstruction::OptionalSharedAssign(assignment) => {
                        self.seed_projected(&assignment.destination);
                        if let MirOptionalSharedSource::Copy(source) = &assignment.source {
                            self.seed_projected(source);
                        }
                    }
                    MirInstruction::ClassOptionalInitialize(initialize) => {
                        self.seed_projected(&initialize.destination);
                        if let MirClassOptionalSource::Copy(source) = &initialize.source {
                            self.seed_projected(source);
                        }
                    }
                    MirInstruction::ClassOptionalAssign(assignment) => {
                        self.seed_projected(&assignment.destination);
                        if let MirClassOptionalSource::Copy(source) = &assignment.source {
                            self.seed_projected(source);
                        }
                    }
                    MirInstruction::Assign(assignment) => {
                        if let MirRvalueKind::OptionalPresence { source, .. } =
                            &assignment.rvalue.kind
                        {
                            self.seed_projected(source);
                        }
                    }
                    _ => {}
                }
            }
            if let Some(MirTerminator::OptionalUnwrap { source, .. }) = &block.terminator {
                self.seed_projected(source);
            }
            if let Some(MirTerminator::OptionalSharedUnwrap { unwrap, .. }) = &block.terminator {
                self.seed_projected(&unwrap.source);
            }
        }
    }

    fn seed_projected(&mut self, place: &MirPlace) {
        if !place.projections.is_empty() {
            self.insert(place.clone());
        }
    }

    fn transfer_class_optional_arguments(
        &mut self,
        function: MirDefinitionRef<'_>,
        arguments: &[MirArgument],
    ) {
        for argument in arguments {
            let MirArgument::OwnedPlace(place) = argument else {
                continue;
            };
            if function
                .storage(place.base.expect_local_storage())
                .is_some_and(|storage| matches!(storage.ty, MirType::OptionalClass(_)))
            {
                self.remove(place);
            }
        }
    }

    fn transfer_optional_shared_arguments(
        &mut self,
        function: MirDefinitionRef<'_>,
        arguments: &[MirArgument],
    ) {
        for argument in arguments {
            let MirArgument::SharedOwner(storage) = argument else {
                continue;
            };
            if function
                .storage(*storage)
                .is_some_and(|entry| matches!(entry.ty, MirType::OptionalShared(_)))
            {
                self.remove(&MirPlace::base(*storage));
            }
        }
    }

    fn initialize_optional_fields_inner(
        &mut self,
        program: &MirProgram,
        class: ClassId,
        root: &MirPlace,
        visiting: &mut HashSet<ClassId>,
    ) {
        if !visiting.insert(class) {
            return;
        }
        if let Some(base) = program.direct_base(class) {
            self.initialize_optional_fields_inner(
                program,
                base,
                &root.clone().project_base(base),
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
                    self.insert(place);
                }
                MirType::Class(nested) => {
                    self.initialize_optional_fields_inner(program, nested, &place, visiting);
                }
                _ => {}
            }
        }
        visiting.remove(&class);
    }
}

pub(super) fn is_optional(ty: MirType) -> bool {
    matches!(
        ty,
        MirType::OptionalPrimitive(_) | MirType::OptionalClass(_) | MirType::OptionalShared(_)
    )
}

fn complete_class_storage(function: MirDefinitionRef<'_>, place: &MirPlace) -> Option<ClassId> {
    place
        .projections
        .is_empty()
        .then(|| function.storage(place.base.expect_local_storage()))
        .flatten()
        .and_then(|storage| match storage.ty {
            MirType::Class(class) => Some(class),
            _ => None,
        })
}
