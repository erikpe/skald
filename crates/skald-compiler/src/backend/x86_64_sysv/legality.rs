//! Target legality checks performed before instruction selection.

use crate::{
    backend::{BackendError, Target},
    mir::{verify_mir, MirInstruction, MirProgram},
};

use super::{abi, frame::FrameLayout};

pub(super) fn check(program: &MirProgram) -> Result<(), BackendError> {
    verify_mir(program).map_err(|errors| {
        BackendError::new(
            Target::X86_64SysV,
            None,
            format!("input MIR failed verification:\n{errors}"),
        )
    })?;

    for function in program.definitions.iter() {
        FrameLayout::plan(function)?;
        for parameter_index in 0..function.parameters.len() {
            if abi::incoming_argument(parameter_index).is_none() {
                return Err(abi_limit(function.function, "incoming argument area"));
            }
        }
        for block in &function.body.blocks {
            for instruction in &block.instructions {
                let MirInstruction::Call(call) = instruction else {
                    continue;
                };
                if abi::outgoing_stack_size(call.arguments.len()).is_none()
                    || call
                        .arguments
                        .iter()
                        .enumerate()
                        .skip(abi::ARGUMENT_REGISTERS.len())
                        .any(|(index, _)| abi::outgoing_argument_offset(index).is_none())
                {
                    return Err(abi_limit(function.function, "outgoing argument area"));
                }
            }
        }
    }
    Ok(())
}

fn abi_limit(function: crate::resolve::FunctionId, area: &str) -> BackendError {
    BackendError::new(
        Target::X86_64SysV,
        Some(function),
        format!("{area} exceeds the x86-64 ABI encoding limits"),
    )
}
