//! Incoming and outgoing System V call lowering.

use crate::{
    backend::{BackendError, Target},
    mir::{
        MirCall, MirCallTarget, MirFunctionDeclaration, MirFunctionDefinition, MirFunctionLinkage,
        MirType, ValueId,
    },
};

use super::{
    super::{
        abi::{ArgumentLocation, CallLayout},
        machine::{ByteRegister, Instruction, Register, XmmRegister},
    },
    symbol_for, value, FrameLayout, InstructionSelector,
};

pub(super) fn spill_parameters(
    declaration: &MirFunctionDeclaration,
    function: &MirFunctionDefinition,
    frame: &FrameLayout,
    output: &mut Vec<Instruction>,
) -> Result<(), BackendError> {
    let layout = CallLayout::classify(&declaration.parameter_types).ok_or_else(|| {
        argument_area_error(
            function,
            "incoming argument area exceeds the x86-64 ABI encoding limits",
        )
    })?;

    for (storage, location) in function.parameters.iter().zip(layout.locations()) {
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
        let MirCallTarget::Direct(target_id) = call.target else {
            unreachable!("target legality rejects method calls before selection")
        };
        debug_assert!(call.receiver.is_none());
        let target = self
            .program
            .declarations
            .get(target_id)
            .expect("verified call target must be declared");
        let layout = CallLayout::classify(&target.parameter_types).ok_or_else(|| {
            argument_area_error(
                self.function,
                "outgoing argument area exceeds the x86-64 ABI encoding limits",
            )
        })?;

        if layout.stack_size() != 0 {
            self.output
                .push(Instruction::ReserveStack(layout.stack_size()));
        }
        for ((argument, ty), location) in call
            .arguments
            .iter()
            .zip(&target.parameter_types)
            .zip(layout.locations())
        {
            self.select_argument(*argument, *ty, *location);
        }

        self.output.push(Instruction::Call(symbol_for(target)));
        if layout.stack_size() != 0 {
            self.output
                .push(Instruction::ReleaseStack(layout.stack_size()));
        }
        self.normalize_external_bool_result(target);
        if let Some(result) = call.result {
            self.store_call_result(target.return_type, result);
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

    fn normalize_external_bool_result(&mut self, target: &MirFunctionDeclaration) {
        if target.return_type == MirType::Bool
            && matches!(target.linkage, MirFunctionLinkage::External { .. })
        {
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

fn argument_area_error(function: &MirFunctionDefinition, message: &'static str) -> BackendError {
    BackendError::new(Target::X86_64SysV, Some(function.function), message)
}
