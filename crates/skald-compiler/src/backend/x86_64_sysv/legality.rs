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
        let declaration = program
            .declarations
            .get(function.function)
            .expect("verified definition must be declared");
        if abi::CallLayout::classify(&declaration.parameter_types).is_none() {
            return Err(abi_limit(function.function, "incoming argument area"));
        }
        for block in &function.body.blocks {
            for instruction in &block.instructions {
                let MirInstruction::Call(call) = instruction else {
                    continue;
                };
                let crate::mir::MirCallTarget::Direct(target) = call.target;
                let target = program
                    .declarations
                    .get(target)
                    .expect("verified call target must be declared");
                if abi::CallLayout::classify(&target.parameter_types).is_none() {
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
