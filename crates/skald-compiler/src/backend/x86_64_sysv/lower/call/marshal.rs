//! Incoming and outgoing System V argument/result marshaling.

use crate::{
    backend::{BackendError, Target},
    identity::{CallableId, ClassId},
    mir::{
        MirArgument, MirCallableSignature, MirDefinitionRef, MirFunctionLinkage, MirParameter,
        MirParameterMode, MirPlace, MirType, StorageId, ValueId,
    },
};

use super::super::super::{
    abi::{ArgumentLocation, CallLayout, ObjectOriginLocations, ParameterLocations},
    machine::{ByteRegister, Instruction, Operand, Register, XmmRegister},
};
use super::super::{
    object_abi::{ObjectOriginOperand, ReceiverOperand},
    value, FrameLayout, InstructionSelector,
};

pub(super) fn spill_parameters(
    signature: MirCallableSignature<'_>,
    function: MirDefinitionRef<'_>,
    frame: &FrameLayout,
    output: &mut Vec<Instruction>,
) -> Result<(), BackendError> {
    let has_return_destination = function.return_storage().is_some_and(|storage| {
        function.storage(storage).is_some_and(|storage| {
            matches!(
                storage.ty,
                MirType::Class(_)
                    | MirType::OptionalPrimitive(_)
                    | MirType::OptionalClass(_)
                    | MirType::Array(_)
            )
        })
    });
    let layout = classify_call(
        signature.parameters,
        function.receiver().is_some(),
        has_return_destination,
    )
    .ok_or_else(|| {
        argument_area_error(
            function,
            "incoming argument area exceeds the x86-64 ABI encoding limits",
        )
    })?;

    if let Some(return_storage) = function.return_storage().filter(|_| has_return_destination) {
        let incoming = layout
            .return_destination()
            .expect("object-returning layout has a return destination")
            .incoming()
            .ok_or_else(|| {
                argument_area_error(
                    function,
                    "incoming return destination exceeds x86-64 limits",
                )
            })?;
        let destination = value::frame_storage(frame, return_storage);
        match incoming {
            ArgumentLocation::IntegerRegister(register) => output.push(Instruction::Move {
                source: register.into(),
                destination,
            }),
            ArgumentLocation::Stack(displacement) => {
                value::load_rax(value::memory(Register::Rbp, displacement), output);
                value::store_rax(destination, output);
            }
            ArgumentLocation::SseRegister(_) => {
                unreachable!("return destination is always integer-class")
            }
        }
    }

    if let Some(receiver) = function.receiver() {
        let incoming = layout
            .receiver_locations()
            .expect("receiver-aware layout has a receiver location")
            .incoming()
            .ok_or_else(|| {
                argument_area_error(function, "incoming receiver area exceeds x86-64 limits")
            })?;
        spill_integer(
            incoming.address(),
            value::frame_storage(frame, receiver),
            output,
        );
        let homes = frame
            .object_origin(receiver)
            .expect("receiver storage has object-origin homes");
        spill_integer(
            incoming.complete(),
            value::memory(Register::Rbp, homes.complete()),
            output,
        );
        spill_integer(
            incoming.metadata(),
            value::memory(Register::Rbp, homes.metadata()),
            output,
        );
    }

    for ((storage, parameter), location) in function
        .parameters()
        .iter()
        .zip(signature.parameters)
        .zip(layout.parameter_locations())
    {
        let incoming = location.incoming().ok_or_else(|| {
            argument_area_error(
                function,
                "incoming argument area exceeds the x86-64 ABI encoding limits",
            )
        })?;
        let destination = value::frame_storage(frame, *storage);
        match incoming.value() {
            ArgumentLocation::IntegerRegister(register)
                if parameter.mode == MirParameterMode::Value && parameter.ty == MirType::U8 =>
            {
                value::load_rax(register.into(), output);
                value::store_canonical_rax(parameter.ty, destination, output);
            }
            ArgumentLocation::IntegerRegister(register) => output.push(Instruction::Move {
                source: register.into(),
                destination,
            }),
            ArgumentLocation::SseRegister(register) => {
                value::store_float(register, value::float_operand(destination), output)
            }
            ArgumentLocation::Stack(displacement)
                if parameter.mode == MirParameterMode::Value && parameter.ty == MirType::F64 =>
            {
                value::load_float(
                    value::float_memory(Register::Rbp, displacement),
                    XmmRegister::Xmm14,
                    output,
                );
                value::store_float(
                    XmmRegister::Xmm14,
                    value::float_operand(destination),
                    output,
                );
            }
            ArgumentLocation::Stack(displacement) => {
                value::load_rax(value::memory(Register::Rbp, displacement), output);
                if parameter.mode == MirParameterMode::Value {
                    value::canonicalize_rax(parameter.ty, output);
                }
                value::store_rax(destination, output);
            }
        }
        if let Some(origin) = incoming.origin() {
            let homes = frame
                .object_origin(*storage)
                .expect("alias parameter storage has object-origin homes");
            spill_integer(
                origin.complete(),
                value::memory(Register::Rbp, homes.complete()),
                output,
            );
            spill_integer(
                origin.metadata(),
                value::memory(Register::Rbp, homes.metadata()),
                output,
            );
        }
    }
    Ok(())
}

impl InstructionSelector<'_, '_> {
    pub(super) fn marshal_shared_initializer_inputs(
        &mut self,
        signature: MirCallableSignature<'_>,
        allocation: StorageId,
        dynamic_class: ClassId,
        arguments: &[MirArgument],
    ) -> Result<CallLayout, BackendError> {
        self.marshal_shared_initializer_handle_inputs(
            signature,
            value::frame_storage(self.frame, allocation),
            dynamic_class,
            arguments,
        )
    }

    pub(super) fn marshal_shared_initializer_handle_inputs(
        &mut self,
        signature: MirCallableSignature<'_>,
        handle: Operand,
        dynamic_class: ClassId,
        arguments: &[MirArgument],
    ) -> Result<CallLayout, BackendError> {
        let layout = classify_call(signature.parameters, true, false).ok_or_else(|| {
            argument_area_error(
                self.function,
                "outgoing shared initializer area exceeds x86-64 limits",
            )
        })?;
        if layout.stack_size() != 0 {
            self.output
                .push(Instruction::ReserveStack(layout.stack_size()));
        }
        let receiver = layout
            .receiver_locations()
            .expect("shared initializer layout has a receiver");
        self.select_shared_payload_address(handle, receiver.address());
        self.select_shared_payload_address(handle, receiver.complete());
        self.select_metadata_symbol(dynamic_class, receiver.metadata());
        for ((argument, parameter), location) in arguments
            .iter()
            .zip(signature.parameters)
            .zip(layout.parameter_locations())
        {
            self.marshal_argument(argument, *parameter, *location)?;
        }
        Ok(layout)
    }

    fn select_shared_payload_address(&mut self, handle: Operand, location: ArgumentLocation) {
        let destination = match location {
            ArgumentLocation::IntegerRegister(register) => register,
            ArgumentLocation::Stack(_) => Register::Rax,
            ArgumentLocation::SseRegister(_) => {
                unreachable!("shared payload addresses are integer-class")
            }
        };
        self.output.push(Instruction::Move {
            source: handle,
            destination: destination.into(),
        });
        self.output.push(Instruction::LoadEffectiveAddress {
            source: value::memory(
                destination,
                super::super::super::layout::SHARED_HEADER_SIZE as i32,
            ),
            destination,
        });
        if let ArgumentLocation::Stack(displacement) = location {
            value::store_rax(value::memory(Register::Rsp, displacement), self.output);
        }
    }

    pub(super) fn marshal_call_inputs(
        &mut self,
        signature: MirCallableSignature<'_>,
        indirect: bool,
        return_destination: Option<&MirPlace>,
        receiver: Option<ReceiverOperand<'_>>,
        arguments: &[MirArgument],
    ) -> Result<CallLayout, BackendError> {
        let layout = classify_call(
            signature.parameters,
            receiver.is_some(),
            return_destination.is_some(),
        )
        .ok_or_else(|| {
            argument_area_error(
                self.function,
                "outgoing argument area exceeds the x86-64 ABI encoding limits",
            )
        })?;

        if layout.stack_size() != 0 {
            self.output
                .push(Instruction::ReserveStack(layout.stack_size()));
        }
        if let Some(return_destination) = return_destination {
            let location = layout
                .return_destination()
                .expect("object-returning call layout has a return destination");
            let ArgumentLocation::IntegerRegister(register) = location else {
                unreachable!("return destination is the first integer-class argument")
            };
            self.materialize_place_address(return_destination, register)?;
        }
        if let Some(receiver) = receiver {
            let locations = layout
                .receiver_locations()
                .expect("receiver-aware layout has a receiver location");
            if indirect {
                self.select_origin_complete(receiver.origin, locations.address())?;
            } else {
                self.select_place_address(receiver.place, locations.address())?;
            }
            self.select_object_origin(
                receiver.origin,
                ObjectOriginLocations::new(locations.complete(), locations.metadata()),
            )?;
        }
        for ((argument, parameter), location) in arguments
            .iter()
            .zip(signature.parameters)
            .zip(layout.parameter_locations())
        {
            self.marshal_argument(argument, *parameter, *location)?;
        }
        Ok(layout)
    }

    pub(super) fn finish_call(
        &mut self,
        layout: &CallLayout,
        direct_target: Option<CallableId>,
        return_type: MirType,
        result: Option<ValueId>,
        shared_result: Option<StorageId>,
    ) {
        if layout.stack_size() != 0 {
            self.output
                .push(Instruction::ReleaseStack(layout.stack_size()));
        }
        self.normalize_external_bool_result(direct_target, return_type);
        if let Some(result) = result {
            self.store_call_result(return_type, result);
        }
        if let Some(result) = shared_result {
            value::store_rax(value::frame_storage(self.frame, result), self.output);
        }
    }

    fn marshal_argument(
        &mut self,
        argument: &MirArgument,
        parameter: MirParameter,
        locations: ParameterLocations,
    ) -> Result<(), BackendError> {
        match (argument, parameter.mode) {
            (MirArgument::Value(argument), MirParameterMode::Value) => {
                self.marshal_value_argument(*argument, parameter.ty, locations.value());
            }
            (MirArgument::Place(place), MirParameterMode::ReadOnlyAlias)
            | (MirArgument::Place(place), MirParameterMode::MutableAlias) => {
                if matches!(
                    parameter.ty,
                    MirType::OptionalPrimitive(_) | MirType::OptionalClass(_) | MirType::Array(_)
                ) {
                    self.select_place_address(place, locations.value())?;
                } else {
                    self.select_inferred_alias(place, locations)?;
                }
            }
            (
                MirArgument::View(crate::mir::MirObjectView { source, origin, .. }),
                MirParameterMode::ReadOnlyAlias | MirParameterMode::MutableAlias,
            ) => {
                self.select_place_address(source, locations.value())?;
                self.select_object_origin(
                    ObjectOriginOperand::Mir(origin),
                    locations
                        .origin()
                        .expect("alias layout carries object-origin locations"),
                )?;
            }
            (MirArgument::OwnedPlace(place), MirParameterMode::Value)
                if matches!(
                    parameter.ty,
                    MirType::Class(_)
                        | MirType::OptionalPrimitive(_)
                        | MirType::OptionalClass(_)
                        | MirType::Array(_)
                ) =>
            {
                self.select_place_address(place, locations.value())?;
            }
            (MirArgument::SharedOwner(owner), MirParameterMode::Value)
                if matches!(
                    parameter.ty,
                    MirType::Shared(_) | MirType::OptionalShared(_)
                ) =>
            {
                self.marshal_shared_owner(*owner, locations.value());
            }
            _ => unreachable!("verified argument kind must match its parameter mode"),
        }
        Ok(())
    }

    fn marshal_shared_owner(&mut self, owner: StorageId, location: ArgumentLocation) {
        let source = value::frame_storage(self.frame, owner);
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
                unreachable!("shared owners are integer-class")
            }
        }
    }

    fn marshal_value_argument(
        &mut self,
        argument: ValueId,
        ty: MirType,
        location: ArgumentLocation,
    ) {
        let source = value::frame_value(self.frame, argument);
        match location {
            ArgumentLocation::IntegerRegister(register) => self.output.push(Instruction::Move {
                source,
                destination: register.into(),
            }),
            ArgumentLocation::SseRegister(register) => {
                value::load_float(value::float_operand(source), register, self.output)
            }
            ArgumentLocation::Stack(displacement) if ty == MirType::F64 => {
                value::load_float(
                    value::float_operand(source),
                    XmmRegister::Xmm14,
                    self.output,
                );
                value::store_float(
                    XmmRegister::Xmm14,
                    value::float_memory(Register::Rsp, displacement),
                    self.output,
                );
            }
            ArgumentLocation::Stack(displacement) => {
                value::load_rax(source, self.output);
                value::store_rax(value::memory(Register::Rsp, displacement), self.output);
            }
        }
    }

    fn normalize_external_bool_result(
        &mut self,
        direct_target: Option<CallableId>,
        return_type: MirType,
    ) {
        let Some(target) = direct_target else {
            return;
        };
        let external = target.as_function().is_some_and(|function| {
            self.program
                .declarations
                .get(function)
                .is_some_and(|declaration| {
                    matches!(declaration.linkage, MirFunctionLinkage::External { .. })
                })
        });
        if return_type == MirType::Bool && external {
            self.output.push(Instruction::ZeroExtendByte {
                source: ByteRegister::Al,
                destination: Register::Rax,
            });
        }
    }

    fn store_call_result(&mut self, ty: MirType, result: ValueId) {
        let destination = value::frame_value(self.frame, result);
        if ty == MirType::F64 {
            value::store_float(
                XmmRegister::Xmm0,
                value::float_operand(destination),
                self.output,
            );
        } else {
            value::store_canonical_rax(ty, destination, self.output);
        }
    }
}

fn spill_integer(location: ArgumentLocation, destination: Operand, output: &mut Vec<Instruction>) {
    match location {
        ArgumentLocation::IntegerRegister(register) => output.push(Instruction::Move {
            source: register.into(),
            destination,
        }),
        ArgumentLocation::Stack(displacement) => {
            value::load_rax(value::memory(Register::Rbp, displacement), output);
            value::store_rax(destination, output);
        }
        ArgumentLocation::SseRegister(_) => {
            unreachable!("object ABI components are always integer-class")
        }
    }
}

fn classify_call(
    parameters: &[MirParameter],
    has_receiver: bool,
    has_return_destination: bool,
) -> Option<CallLayout> {
    CallLayout::classify_internal_call(parameters, has_receiver, has_return_destination)
}

fn argument_area_error(function: MirDefinitionRef<'_>, message: &'static str) -> BackendError {
    BackendError::new(Target::X86_64SysV, Some(function.callable()), message)
}
