//! Incoming and outgoing System V call lowering.

use crate::{
    backend::{BackendError, Target},
    identity::CallableId,
    mir::{
        MirArgument, MirCall, MirCallTarget, MirCallableSignature, MirDefinitionRef,
        MirFunctionLinkage, MirInitialize, MirParameter, MirPlace, MirType, ValueId,
    },
};

use super::{
    super::{
        abi::{ArgumentLocation, CallLayout},
        machine::{ByteRegister, Instruction, Register, XmmRegister},
    },
    value, FrameLayout, InstructionSelector,
};

pub(super) fn spill_parameters(
    signature: MirCallableSignature<'_>,
    function: MirDefinitionRef<'_>,
    frame: &FrameLayout,
    output: &mut Vec<Instruction>,
) -> Result<(), BackendError> {
    let layout =
        classify_call(signature.parameters, function.receiver().is_some()).ok_or_else(|| {
            argument_area_error(
                function,
                "incoming argument area exceeds the x86-64 ABI encoding limits",
            )
        })?;

    if let Some(receiver) = function.receiver() {
        let incoming = layout
            .receiver()
            .expect("receiver-aware layout has a receiver location")
            .incoming()
            .ok_or_else(|| {
                argument_area_error(function, "incoming receiver area exceeds x86-64 limits")
            })?;
        let destination = value::frame_storage(frame, receiver);
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
                unreachable!("receiver is always integer-class")
            }
        }
    }

    for (storage, location) in function.parameters().iter().zip(layout.locations()) {
        let incoming = location.incoming().ok_or_else(|| {
            argument_area_error(
                function,
                "incoming argument area exceeds the x86-64 ABI encoding limits",
            )
        })?;
        let ty = function
            .storage(*storage)
            .expect("verified parameter storage must exist")
            .ty;
        let destination = value::frame_storage(frame, *storage);
        match incoming {
            ArgumentLocation::IntegerRegister(register) if ty == MirType::U8 => {
                value::load_rax(register.into(), output);
                value::store_canonical_rax(ty, destination, output);
            }
            ArgumentLocation::IntegerRegister(register) => output.push(Instruction::Move {
                source: register.into(),
                destination,
            }),
            ArgumentLocation::SseRegister(register) => {
                value::store_float(register, value::float_operand(destination), output)
            }
            ArgumentLocation::Stack(displacement) if ty == MirType::F64 => {
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
                value::store_canonical_rax(ty, destination, output);
            }
        }
    }
    Ok(())
}

impl InstructionSelector<'_, '_> {
    pub(super) fn select_call(&mut self, call: &MirCall) -> Result<(), BackendError> {
        let (target, receiver) = match call.target {
            MirCallTarget::Direct(function) => (CallableId::Function(function), None),
            MirCallTarget::Method(method) => (
                CallableId::Method(method),
                Some(
                    call.receiver
                        .as_ref()
                        .expect("verified method call has a receiver"),
                ),
            ),
        };
        self.select_callable(target, receiver, &call.arguments, call.result)
    }

    pub(super) fn select_initialize(
        &mut self,
        initialize: &MirInitialize,
    ) -> Result<(), BackendError> {
        self.select_callable(
            CallableId::Initializer(initialize.target),
            Some(&initialize.destination),
            &initialize.arguments,
            None,
        )
    }

    fn select_callable(
        &mut self,
        target: CallableId,
        receiver: Option<&MirPlace>,
        arguments: &[MirArgument],
        result: Option<ValueId>,
    ) -> Result<(), BackendError> {
        let signature = self
            .program
            .callable_signature(target)
            .expect("verified call target must be declared");
        let layout = classify_call(signature.parameters, receiver.is_some()).ok_or_else(|| {
            argument_area_error(
                self.function,
                "outgoing argument area exceeds the x86-64 ABI encoding limits",
            )
        })?;

        if layout.stack_size() != 0 {
            self.output
                .push(Instruction::ReserveStack(layout.stack_size()));
        }
        if let Some(receiver) = receiver {
            let location = layout
                .receiver()
                .expect("receiver-aware layout has a receiver location");
            let ArgumentLocation::IntegerRegister(register) = location else {
                unreachable!("receiver is always the first integer-class argument")
            };
            self.materialize_place_address(receiver, register);
        }
        for ((argument, ty), location) in arguments
            .iter()
            .zip(signature.parameters)
            .zip(layout.locations())
        {
            let MirArgument::Value(argument) = argument else {
                unreachable!("target legality rejects alias arguments before selection")
            };
            self.select_argument(*argument, ty.ty, *location);
        }

        self.output
            .push(Instruction::Call(super::super::symbol::callable(
                self.program,
                target,
            )));
        if layout.stack_size() != 0 {
            self.output
                .push(Instruction::ReleaseStack(layout.stack_size()));
        }
        self.normalize_external_bool_result(target, signature.return_type);
        if let Some(result) = result {
            self.store_call_result(signature.return_type, result);
        }
        Ok(())
    }

    fn select_argument(&mut self, argument: ValueId, ty: MirType, location: ArgumentLocation) {
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

    fn normalize_external_bool_result(&mut self, target: CallableId, return_type: MirType) {
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

fn classify_call(parameters: &[MirParameter], has_receiver: bool) -> Option<CallLayout> {
    let parameter_types: Vec<_> = parameters.iter().map(|parameter| parameter.ty).collect();
    if has_receiver {
        CallLayout::classify_with_receiver(&parameter_types)
    } else {
        CallLayout::classify(&parameter_types)
    }
}

fn argument_area_error(function: MirDefinitionRef<'_>, message: &'static str) -> BackendError {
    BackendError::new(Target::X86_64SysV, Some(function.callable()), message)
}
