//! Polymorphic receiver and object-alias ABI lowering.

use crate::{
    backend::BackendError,
    identity::{ClassId, InterfaceRequirementId, VirtualSlotId},
    mir::{MirObjectOrigin, MirPlace, MirPlaceProjection, MirType, StorageId},
};

use super::{
    super::{
        abi::{ArgumentLocation, ObjectOriginLocations, ParameterLocations},
        dispatch::DispatchMetadata,
        layout::SHARED_DYNAMIC_METADATA_OFFSET,
        machine::{Instruction, Operand, Register},
    },
    value, InstructionSelector,
};

#[derive(Clone, Copy)]
pub(super) struct ReceiverOperand<'mir> {
    pub(super) place: &'mir MirPlace,
    pub(super) origin: ObjectOriginOperand<'mir>,
}

#[derive(Clone, Copy)]
pub(super) enum ObjectOriginOperand<'mir> {
    Mir(&'mir MirObjectOrigin),
    Exact {
        complete: &'mir MirPlace,
        dynamic_class: ClassId,
    },
}

impl InstructionSelector<'_, '_> {
    pub(super) fn select_inferred_alias(
        &mut self,
        place: &MirPlace,
        locations: ParameterLocations,
    ) -> Result<(), BackendError> {
        self.select_place_address(place, locations.value())?;
        let origin_locations = locations
            .origin()
            .expect("alias layout carries object-origin locations");
        if let Some((complete, dynamic_class)) = self.projected_exact_object(place) {
            return self.select_object_origin(
                ObjectOriginOperand::Exact {
                    complete: &complete,
                    dynamic_class,
                },
                origin_locations,
            );
        }
        if let Some((complete, dynamic_class)) = self.static_exact_object(place) {
            return self.select_object_origin(
                ObjectOriginOperand::Exact {
                    complete: &complete,
                    dynamic_class,
                },
                origin_locations,
            );
        }
        let carrier = place.base.expect_local_storage();
        if self.frame.object_origin(carrier).is_some() {
            self.select_forwarded_origin(carrier, origin_locations);
            return Ok(());
        }
        let MirType::Class(dynamic_class) = self
            .function
            .storage(carrier)
            .expect("verified alias place base names storage")
            .ty
        else {
            unreachable!("an exact object alias place has a class-typed owning root")
        };
        let mut complete = place.clone();
        complete.projections.clear();
        self.select_object_origin(
            ObjectOriginOperand::Exact {
                complete: &complete,
                dynamic_class,
            },
            origin_locations,
        )
    }

    fn static_exact_object(&self, place: &MirPlace) -> Option<(MirPlace, ClassId)> {
        let field = match place.base {
            crate::mir::MirPlaceBase::StaticField(field)
            | crate::mir::MirPlaceBase::StaticLifecycleDestination(field) => field,
            _ => return None,
        };
        let MirType::Class(dynamic_class) = self.program.static_field(field)?.ty else {
            return None;
        };
        let mut complete = place.clone();
        complete.projections.clear();
        Some((complete, dynamic_class))
    }

    fn projected_exact_object(&self, place: &MirPlace) -> Option<(MirPlace, ClassId)> {
        let mut exact = None;

        for (index, projection) in place.projections.iter().enumerate() {
            let establishes_complete_object = !matches!(projection, MirPlaceProjection::Base(_));
            let ty = match *projection {
                MirPlaceProjection::Base(base) => MirType::Class(base),
                MirPlaceProjection::Field(field) => {
                    self.program
                        .field(field)
                        .expect("verified field projection names a declared field")
                        .ty
                }
                MirPlaceProjection::OptionalPayload(class) => MirType::Class(class),
                MirPlaceProjection::NestedOptionalPayload(optional)
                | MirPlaceProjection::CheckedOptionalPayload(optional) => {
                    self.program
                        .optional_type(optional)
                        .expect("verified optional projection names metadata")
                        .payload
                }
                MirPlaceProjection::ArrayElement { array, .. } => {
                    self.program
                        .array_type(array)
                        .expect("verified array projection names a declared array type")
                        .element
                }
            };
            if let (true, MirType::Class(class)) = (establishes_complete_object, ty) {
                let mut complete = place.clone();
                complete.projections.truncate(index + 1);
                exact = Some((complete, class));
            }
        }
        exact
    }

    pub(super) fn select_place_address(
        &mut self,
        place: &MirPlace,
        location: ArgumentLocation,
    ) -> Result<(), BackendError> {
        match location {
            ArgumentLocation::IntegerRegister(register) => {
                self.materialize_place_address(place, register)?;
            }
            ArgumentLocation::Stack(displacement) => {
                self.materialize_place_address(place, Register::Rax)?;
                value::store_rax(value::memory(Register::Rsp, displacement), self.output);
            }
            ArgumentLocation::SseRegister(_) => {
                unreachable!("object addresses are always integer-class")
            }
        }
        Ok(())
    }

    pub(super) fn select_object_origin(
        &mut self,
        origin: ObjectOriginOperand<'_>,
        locations: ObjectOriginLocations,
    ) -> Result<(), BackendError> {
        match origin {
            ObjectOriginOperand::Mir(MirObjectOrigin::Exact {
                complete,
                dynamic_class,
            }) => {
                self.select_place_address(complete, locations.complete())?;
                self.select_metadata_symbol(*dynamic_class, locations.metadata());
            }
            ObjectOriginOperand::Exact {
                complete,
                dynamic_class,
            } => {
                self.select_place_address(complete, locations.complete())?;
                self.select_metadata_symbol(dynamic_class, locations.metadata());
            }
            ObjectOriginOperand::Mir(MirObjectOrigin::Forwarded { carrier, .. }) => {
                self.select_forwarded_origin(*carrier, locations);
            }
            ObjectOriginOperand::Mir(MirObjectOrigin::Shared { owner, .. }) => {
                self.select_place_address(&MirPlace::shared_pointee(*owner), locations.complete())?;
                self.select_shared_metadata(*owner, locations.metadata());
            }
        }
        Ok(())
    }

    pub(super) fn select_origin_complete(
        &mut self,
        origin: ObjectOriginOperand<'_>,
        location: ArgumentLocation,
    ) -> Result<(), BackendError> {
        match origin {
            ObjectOriginOperand::Mir(MirObjectOrigin::Exact { complete, .. }) => {
                self.select_place_address(complete, location)
            }
            ObjectOriginOperand::Exact { complete, .. } => {
                self.select_place_address(complete, location)
            }
            ObjectOriginOperand::Mir(MirObjectOrigin::Forwarded { carrier, .. }) => {
                let homes = self
                    .frame
                    .object_origin(*carrier)
                    .expect("verified forwarded carrier has object-origin homes");
                self.select_frame_word(homes.complete(), location);
                Ok(())
            }
            ObjectOriginOperand::Mir(MirObjectOrigin::Shared { owner, .. }) => {
                self.select_place_address(&MirPlace::shared_pointee(*owner), location)
            }
        }
    }

    pub(super) fn load_origin_metadata(
        &mut self,
        origin: ObjectOriginOperand<'_>,
        destination: Register,
    ) {
        match origin {
            ObjectOriginOperand::Mir(MirObjectOrigin::Forwarded { carrier, .. }) => {
                let metadata = self
                    .frame
                    .object_origin(*carrier)
                    .expect("verified forwarded carrier has object-origin homes")
                    .metadata();
                self.output.push(Instruction::Move {
                    source: value::memory(Register::Rbp, metadata),
                    destination: destination.into(),
                });
            }
            ObjectOriginOperand::Mir(MirObjectOrigin::Exact { dynamic_class, .. }) => {
                self.load_table_address(*dynamic_class, destination);
            }
            ObjectOriginOperand::Exact { dynamic_class, .. } => {
                self.load_table_address(dynamic_class, destination);
            }
            ObjectOriginOperand::Mir(MirObjectOrigin::Shared { owner, .. }) => {
                self.load_shared_metadata(*owner, destination);
            }
        }
    }

    pub(super) fn store_object_origin(
        &mut self,
        origin: ObjectOriginOperand<'_>,
        destination: StorageId,
    ) -> Result<(), BackendError> {
        let homes = self
            .frame
            .object_origin(destination)
            .expect("indirect object-view carriers have origin homes");
        self.select_origin_complete(origin, ArgumentLocation::IntegerRegister(Register::Rax))?;
        value::store_rax(value::memory(Register::Rbp, homes.complete()), self.output);
        self.load_origin_metadata(origin, Register::Rax);
        value::store_rax(value::memory(Register::Rbp, homes.metadata()), self.output);
        Ok(())
    }

    pub(super) fn select_virtual_target(
        &mut self,
        origin: ObjectOriginOperand<'_>,
        slot: VirtualSlotId,
    ) -> Result<(), BackendError> {
        let displacement = DispatchMetadata::slot_displacement(slot)?;
        self.select_dispatch_target(origin, displacement);
        Ok(())
    }

    pub(super) fn select_interface_target(
        &mut self,
        origin: ObjectOriginOperand<'_>,
        requirement: InterfaceRequirementId,
    ) -> Result<(), BackendError> {
        let displacement = self.dispatch.requirement_displacement(requirement)?;
        self.select_dispatch_target(origin, displacement);
        Ok(())
    }

    fn select_dispatch_target(&mut self, origin: ObjectOriginOperand<'_>, displacement: i32) {
        match origin {
            ObjectOriginOperand::Mir(MirObjectOrigin::Forwarded { carrier, .. }) => {
                let metadata = self
                    .frame
                    .object_origin(*carrier)
                    .expect("verified forwarded carrier has object-origin homes")
                    .metadata();
                self.output.push(Instruction::Move {
                    source: value::memory(Register::Rbp, metadata),
                    destination: Register::R11.into(),
                });
            }
            ObjectOriginOperand::Mir(MirObjectOrigin::Exact { dynamic_class, .. }) => {
                self.load_table_address(*dynamic_class, Register::R11);
            }
            ObjectOriginOperand::Exact { dynamic_class, .. } => {
                self.load_table_address(dynamic_class, Register::R11);
            }
            ObjectOriginOperand::Mir(MirObjectOrigin::Shared { owner, .. }) => {
                self.load_shared_metadata(*owner, Register::R11);
            }
        }
        self.output.push(Instruction::Move {
            source: Operand::Memory {
                base: Register::R11,
                displacement,
            },
            destination: Register::R11.into(),
        });
    }

    fn select_forwarded_origin(&mut self, carrier: StorageId, locations: ObjectOriginLocations) {
        let homes = self
            .frame
            .object_origin(carrier)
            .expect("verified forwarded carrier has object-origin homes");
        self.select_frame_word(homes.complete(), locations.complete());
        self.select_frame_word(homes.metadata(), locations.metadata());
    }

    fn load_shared_metadata(&mut self, owner: StorageId, destination: Register) {
        self.output.push(Instruction::Move {
            source: value::frame_storage(self.frame, owner),
            destination: destination.into(),
        });
        self.output.push(Instruction::Move {
            source: value::memory(destination, SHARED_DYNAMIC_METADATA_OFFSET),
            destination: destination.into(),
        });
    }

    fn select_shared_metadata(&mut self, owner: StorageId, location: ArgumentLocation) {
        self.load_shared_metadata(owner, Register::Rax);
        match location {
            ArgumentLocation::IntegerRegister(register) => {
                if register != Register::Rax {
                    self.output.push(Instruction::Move {
                        source: Register::Rax.into(),
                        destination: register.into(),
                    });
                }
            }
            ArgumentLocation::Stack(displacement) => {
                value::store_rax(value::memory(Register::Rsp, displacement), self.output);
            }
            ArgumentLocation::SseRegister(_) => {
                unreachable!("object metadata is always integer-class")
            }
        }
    }

    fn select_frame_word(&mut self, home: i32, location: ArgumentLocation) {
        let source = value::memory(Register::Rbp, home);
        match location {
            ArgumentLocation::IntegerRegister(register) => {
                self.output.push(Instruction::Move {
                    source,
                    destination: register.into(),
                });
            }
            ArgumentLocation::Stack(displacement) => {
                value::load_rax(source, self.output);
                value::store_rax(value::memory(Register::Rsp, displacement), self.output);
            }
            ArgumentLocation::SseRegister(_) => {
                unreachable!("object metadata is always integer-class")
            }
        }
    }

    pub(super) fn select_metadata_symbol(&mut self, class: ClassId, location: ArgumentLocation) {
        let symbol = self.dispatch.table_symbol(self.program, class);
        match location {
            ArgumentLocation::IntegerRegister(destination) => {
                self.output.push(Instruction::LoadSymbolAddress {
                    symbol,
                    destination,
                });
            }
            ArgumentLocation::Stack(displacement) => {
                self.output.push(Instruction::LoadSymbolAddress {
                    symbol,
                    destination: Register::Rax,
                });
                value::store_rax(value::memory(Register::Rsp, displacement), self.output);
            }
            ArgumentLocation::SseRegister(_) => {
                unreachable!("object metadata is always integer-class")
            }
        }
    }

    fn load_table_address(&mut self, class: ClassId, destination: Register) {
        let symbol = self.dispatch.table_symbol(self.program, class);
        self.output.push(Instruction::LoadSymbolAddress {
            symbol,
            destination,
        });
    }
}
