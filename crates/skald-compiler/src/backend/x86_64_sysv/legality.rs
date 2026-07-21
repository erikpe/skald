//! Target legality checks performed before instruction selection.

use crate::{
    backend::{BackendError, Target},
    identity::CallableId,
    mir::{verify_mir, MirCallTarget, MirInstruction, MirParameter, MirParameterMode, MirProgram},
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
        reject_alias_parameters(function.callable(), signature.parameters)?;
        if classify(signature.parameters, function.receiver().is_some()).is_none() {
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
                            reject_alias_parameters(function.callable(), &target.parameters)?;
                            if classify(&target.parameters, false).is_none() {
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
    reject_alias_parameters(caller, signature.parameters)?;
    if classify(signature.parameters, true).is_none() {
        return Err(abi_limit(caller, "outgoing argument area"));
    }
    Ok(())
}

fn classify(parameters: &[MirParameter], has_receiver: bool) -> Option<abi::CallLayout> {
    let types: Vec<_> = parameters.iter().map(|parameter| parameter.ty).collect();
    if has_receiver {
        abi::CallLayout::classify_with_receiver(&types)
    } else {
        abi::CallLayout::classify(&types)
    }
}

fn reject_alias_parameters(
    caller: CallableId,
    parameters: &[MirParameter],
) -> Result<(), BackendError> {
    if parameters
        .iter()
        .any(|parameter| parameter.mode != MirParameterMode::Value)
    {
        return Err(BackendError::new(
            Target::X86_64SysV,
            Some(caller),
            "alias parameter ABI lowering is not implemented",
        ));
    }
    Ok(())
}

fn abi_limit(function: CallableId, area: &str) -> BackendError {
    BackendError::new(
        Target::X86_64SysV,
        Some(function),
        format!("{area} exceeds the x86-64 ABI encoding limits"),
    )
}
