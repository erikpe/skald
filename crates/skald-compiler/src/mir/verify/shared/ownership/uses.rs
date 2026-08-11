use crate::mir::{
    BlockId, MirArgument, MirCallReceiver, MirInstruction, MirObjectOrigin, MirPlace, MirPlaceBase,
    MirRvalueKind, MirSharedCast, MirSharedCastSource, MirType, StorageId,
};

use super::{state::SharedState, SharedOwnershipAnalysis};

impl SharedOwnershipAnalysis<'_, '_> {
    pub(super) fn require_shared_cast_source(
        &mut self,
        block: BlockId,
        state: &SharedState,
        cast: &MirSharedCast,
    ) {
        match &cast.source {
            MirSharedCastSource::Owner { storage, .. } => {
                self.reject_static_owner(block, state, *storage, "shared cast");
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

    pub(super) fn check_pointee_uses(
        &mut self,
        block: BlockId,
        state: &SharedState,
        instruction: &MirInstruction,
    ) {
        match instruction {
            MirInstruction::Assign(assignment) => match &assignment.rvalue.kind {
                MirRvalueKind::Load(place)
                | MirRvalueKind::OptionalPresence { source: place, .. } => {
                    self.require_live_pointee(block, state, place)
                }
                MirRvalueKind::OptionalBoxPresence { owner, .. } => {
                    if !state.live_owners.contains(owner) {
                        self.error(block, "optional-box presence test requires a live owner");
                    }
                }
                MirRvalueKind::TypeTest { source, .. } => {
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
                        MirCallReceiver::Method(receiver) => {
                            self.require_live_pointee(block, state, &receiver.place);
                            self.require_live_shared_origin(block, state, &receiver.origin);
                        }
                        MirCallReceiver::Interface(view) => {
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
            MirInstruction::OptionalInitialize(operation) => {
                self.require_live_pointee(block, state, &operation.destination);
                if let crate::mir::MirOptionalSource::Copy(source) = &operation.source {
                    self.require_live_pointee(block, state, source);
                }
            }
            MirInstruction::OptionalAssign(operation) => {
                self.require_live_pointee(block, state, &operation.destination);
                if let crate::mir::MirOptionalSource::Copy(source) = &operation.source {
                    self.require_live_pointee(block, state, source);
                }
            }
            MirInstruction::AggregateOptionalInitialize(operation) => {
                self.require_live_pointee(block, state, &operation.destination);
                if let crate::mir::MirAggregateOptionalSource::Copy(source) = &operation.source {
                    self.require_live_pointee(block, state, source);
                }
            }
            MirInstruction::AggregateOptionalAssign(operation) => {
                self.require_live_pointee(block, state, &operation.destination);
                if let crate::mir::MirAggregateOptionalSource::Copy(source) = &operation.source {
                    self.require_live_pointee(block, state, source);
                }
            }
            MirInstruction::AggregateOptionalPublish(operation) => {
                self.require_live_pointee(block, state, &operation.destination)
            }
            MirInstruction::AggregateOptionalCleanup(operation) => {
                self.require_live_pointee(block, state, &operation.destination)
            }
            MirInstruction::ClassOptionalInitialize(operation) => self
                .check_class_optional_pointee_uses(
                    block,
                    state,
                    &operation.destination,
                    &operation.source,
                ),
            MirInstruction::ClassOptionalAssign(operation) => self
                .check_class_optional_pointee_uses(
                    block,
                    state,
                    &operation.destination,
                    &operation.source,
                ),
            MirInstruction::ClassOptionalPublish(operation) => {
                self.require_live_pointee(block, state, &operation.destination)
            }
            MirInstruction::ClassOptionalCleanup(operation) => {
                self.require_live_pointee(block, state, &operation.destination)
            }
            MirInstruction::OptionalSharedInitialize(operation) => self
                .check_optional_shared_pointee_uses(
                    block,
                    state,
                    &operation.destination,
                    &operation.source,
                ),
            MirInstruction::OptionalSharedAssign(operation) => self
                .check_optional_shared_pointee_uses(
                    block,
                    state,
                    &operation.destination,
                    &operation.source,
                ),
            MirInstruction::OptionalSharedCleanup(operation) => {
                self.require_live_pointee(block, state, &operation.destination)
            }
            _ => {}
        }
    }

    fn check_class_optional_pointee_uses(
        &mut self,
        block: BlockId,
        state: &SharedState,
        destination: &MirPlace,
        source: &crate::mir::MirClassOptionalSource,
    ) {
        self.require_live_pointee(block, state, destination);
        match source {
            crate::mir::MirClassOptionalSource::Present(source)
            | crate::mir::MirClassOptionalSource::Copy(source) => {
                self.require_live_pointee(block, state, source)
            }
            crate::mir::MirClassOptionalSource::Absent => {}
        }
    }

    fn check_optional_shared_pointee_uses(
        &mut self,
        block: BlockId,
        state: &SharedState,
        destination: &MirPlace,
        source: &crate::mir::MirOptionalSharedSource,
    ) {
        self.require_live_pointee(block, state, destination);
        if let crate::mir::MirOptionalSharedSource::Copy(source) = source {
            self.require_live_pointee(block, state, source);
        }
    }

    pub(super) fn require_live_pointee(
        &mut self,
        block: BlockId,
        state: &SharedState,
        place: &MirPlace,
    ) {
        let owner = match place.base {
            MirPlaceBase::SharedPointee(owner) | MirPlaceBase::OptionalBoxPayload { owner, .. } => {
                owner
            }
            _ => return,
        };
        if !state.live_owners.contains(&owner) {
            self.error(block, "shared pointee is used without a live owner");
        }
    }

    pub(super) fn require_live_shared_origin(
        &mut self,
        block: BlockId,
        state: &SharedState,
        origin: &MirObjectOrigin,
    ) {
        let MirObjectOrigin::Shared {
            owner,
            exact_dynamic_class,
            ..
        } = origin
        else {
            return;
        };
        if !state.live_owners.contains(owner) {
            self.error(block, "shared object origin is used without a live owner");
        }
        if let Some(class) = exact_dynamic_class {
            let exact_origin = state
                .owner_origins
                .get(owner)
                .and_then(|origin| self.function.storage(*origin))
                .is_some_and(|origin| match origin.ty {
                    MirType::Class(origin) => origin == *class,
                    MirType::Optional(optional) => self
                        .verifier
                        .program
                        .exact_optional_box_type(optional)
                        .is_some_and(|box_type| box_type.exact_dynamic_class == Some(*class)),
                    _ => false,
                });
            if !exact_origin {
                self.error(
                    block,
                    "shared object origin exact dynamic provenance does not match its allocation",
                );
            }
        }
    }

    pub(super) fn reject_static_owner(
        &mut self,
        block: BlockId,
        state: &SharedState,
        owner: StorageId,
        operation: &'static str,
    ) {
        if state.static_owners.contains_key(&owner) {
            self.error(
                block,
                format!(
                    "{operation} cannot consume static literal backing before string initialization"
                ),
            );
        }
    }
}
