//! Target legality checks performed before instruction selection.

use crate::{
    backend::{BackendError, Target},
    identity::FunctionId,
    mir::{verify_mir, MirCallTarget, MirInstruction, MirProgram},
};

use super::{abi, layout::DataLayout};

pub(super) fn check(program: &MirProgram) -> Result<DataLayout, BackendError> {
    verify_mir(program).map_err(|errors| {
        BackendError::new(
            Target::X86_64SysV,
            None,
            format!("input MIR failed verification:\n{errors}"),
        )
    })?;

    let data_layout = DataLayout::compute(program)?;

    for function in program.definitions.iter() {
        let declaration = program
            .declarations
            .get(function.function)
            .expect("verified definition must be declared");
        if abi::CallLayout::classify(&declaration.parameter_types).is_none() {
            return Err(abi_limit(function.function, "incoming argument area"));
        }
        for block in &function.body.blocks {
            for instruction in &block.instructions {
                match instruction {
                    MirInstruction::Initialize(_) => {
                        return Err(object_calling_not_supported(function.function));
                    }
                    MirInstruction::Call(call) => match call.target {
                        MirCallTarget::Method(_) => {
                            return Err(object_calling_not_supported(function.function));
                        }
                        MirCallTarget::Direct(target) => {
                            let target = program
                                .declarations
                                .get(target)
                                .expect("verified call target must be declared");
                            if abi::CallLayout::classify(&target.parameter_types).is_none() {
                                return Err(abi_limit(function.function, "outgoing argument area"));
                            }
                        }
                    },
                    MirInstruction::Assign(_) | MirInstruction::Store(_) => {}
                }
            }
        }
    }
    Ok(data_layout)
}

fn object_calling_not_supported(function: FunctionId) -> BackendError {
    BackendError::new(
        Target::X86_64SysV,
        Some(function),
        "inline-object initialization and receiver calls require OBJ4 lowering",
    )
}

fn abi_limit(function: FunctionId, area: &str) -> BackendError {
    BackendError::new(
        Target::X86_64SysV,
        Some(function),
        format!("{area} exceeds the x86-64 ABI encoding limits"),
    )
}
