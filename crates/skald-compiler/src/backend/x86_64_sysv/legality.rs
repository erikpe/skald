//! Target legality checks performed before instruction selection.

use crate::{
    backend::{BackendError, Target},
    identity::CallableId,
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

    for function in program.executable_definitions() {
        let signature = program
            .callable_signature(function.callable())
            .expect("verified definition must be declared");
        if classify(signature.parameter_types, function.receiver().is_some()).is_none() {
            return Err(abi_limit(function.callable(), "incoming argument area"));
        }
        for block in &function.body().blocks {
            for instruction in &block.instructions {
                match instruction {
                    MirInstruction::Initialize(initialize) => {
                        check_member_target(
                            program,
                            function.callable(),
                            initialize.target.into(),
                        )?;
                    }
                    MirInstruction::Call(call) => match call.target {
                        MirCallTarget::Method(method) => {
                            check_member_target(program, function.callable(), method.into())?;
                        }
                        MirCallTarget::Direct(target) => {
                            let target = program
                                .declarations
                                .get(target)
                                .expect("verified call target must be declared");
                            if abi::CallLayout::classify(&target.parameter_types).is_none() {
                                return Err(abi_limit(
                                    function.callable(),
                                    "outgoing argument area",
                                ));
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

fn check_member_target(
    program: &MirProgram,
    caller: CallableId,
    target: CallableId,
) -> Result<(), BackendError> {
    if program.member_definition(target).is_none() {
        return Err(BackendError::new(
            Target::X86_64SysV,
            Some(caller),
            format!("member call target {target} has no MIR definition"),
        ));
    }
    let signature = program
        .callable_signature(target)
        .expect("verified member target must be declared");
    if classify(signature.parameter_types, true).is_none() {
        return Err(abi_limit(caller, "outgoing argument area"));
    }
    Ok(())
}

fn classify(types: &[crate::mir::MirType], has_receiver: bool) -> Option<abi::CallLayout> {
    if has_receiver {
        abi::CallLayout::classify_with_receiver(types)
    } else {
        abi::CallLayout::classify(types)
    }
}

fn abi_limit(function: CallableId, area: &str) -> BackendError {
    BackendError::new(
        Target::X86_64SysV,
        Some(function),
        format!("{area} exceeds the x86-64 ABI encoding limits"),
    )
}
