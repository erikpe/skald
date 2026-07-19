//! Target legality checks performed before instruction selection.

use crate::{
    backend::{BackendError, Target},
    mir::{verify_mir, MirInstruction, MirProgram, MirRvalueKind},
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

    for function in program.functions.iter() {
        if function.body.blocks.len() != 1 {
            return Err(BackendError::new(
                Target::X86_64SysV,
                Some(function.id),
                format!(
                    "the initial backend supports exactly one basic block, found {}",
                    function.body.blocks.len()
                ),
            ));
        }
        if function.body.blocks[0].id != function.body.entry {
            return Err(BackendError::new(
                Target::X86_64SysV,
                Some(function.id),
                "the sole basic block is not the function entry block",
            ));
        }

        FrameLayout::plan(function)?;
        for parameter_index in 0..function.parameters.len() {
            if abi::incoming_argument(parameter_index).is_none() {
                return Err(abi_limit(function.id, "incoming argument area"));
            }
        }
        for block in &function.body.blocks {
            for instruction in &block.instructions {
                let MirInstruction::Assign(assignment) = instruction else {
                    continue;
                };
                if let MirRvalueKind::DirectCall { arguments, .. } = &assignment.rvalue.kind {
                    if abi::outgoing_stack_size(arguments.len()).is_none()
                        || arguments
                            .iter()
                            .enumerate()
                            .skip(abi::ARGUMENT_REGISTERS.len())
                            .any(|(index, _)| abi::outgoing_argument_offset(index).is_none())
                    {
                        return Err(abi_limit(function.id, "outgoing argument area"));
                    }
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
