//! Target legality checks performed before instruction selection.

use crate::{
    backend::{BackendError, Target},
    identity::CallableId,
    mir::{
        verify_mir, MirCallTarget, MirInstruction, MirMethodCallTarget, MirParameter, MirProgram,
        MirRvalueKind, MirTerminator,
    },
};

use super::{abi, dispatch::DispatchMetadata, layout::DataLayout};

pub(super) fn check(program: &MirProgram) -> Result<(DataLayout, DispatchMetadata), BackendError> {
    verify_mir(program).map_err(|errors| {
        BackendError::new(
            Target::X86_64SysV,
            None,
            format!("input MIR failed verification:\n{errors}"),
        )
    })?;
    reject_unsupported_type_operations(program)?;
    let dispatch = DispatchMetadata::compute(program)?;
    let data_layout = DataLayout::compute(program)?;

    for function in program.executable_definitions() {
        let signature = program
            .callable_signature(function.callable())
            .expect("verified definition must be declared");
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
                        MirCallTarget::Method(MirMethodCallTarget::Direct(method)) => {
                            check_member_target(program, function.callable(), method.into())?;
                        }
                        MirCallTarget::Method(MirMethodCallTarget::Virtual {
                            selected, ..
                        }) => {
                            check_member_target(program, function.callable(), selected.into())?;
                        }
                        MirCallTarget::Direct(target) => {
                            let target = program
                                .declarations
                                .get(target)
                                .expect("verified call target must be declared");
                            if classify(&target.parameters, false).is_none() {
                                return Err(abi_limit(
                                    function.callable(),
                                    "outgoing argument area",
                                ));
                            }
                        }
                        MirCallTarget::Interface(target) => {
                            let requirement = program
                                .interface_requirement(target.requirement)
                                .expect("verified interface target must be declared");
                            if classify(&requirement.parameters, true).is_none() {
                                return Err(abi_limit(
                                    function.callable(),
                                    "outgoing interface argument area",
                                ));
                            }
                        }
                    },
                    // Cleanup targets and their complete destruction plans are
                    // verified before target layout and instruction selection.
                    MirInstruction::Cleanup(_) => {}
                    MirInstruction::CopyConstruct(copy) => {
                        if let crate::mir::MirSelectedCopyOperation::User(target) = copy.operation {
                            check_member_target(program, function.callable(), target.into())?;
                        }
                    }
                    MirInstruction::CopyAssign(copy) => {
                        if let crate::mir::MirSelectedCopyOperation::User(target) = copy.operation {
                            check_member_target(program, function.callable(), target.into())?;
                        }
                    }
                    MirInstruction::Assign(_)
                    | MirInstruction::BindNarrowedAlias(_)
                    | MirInstruction::EndNarrowedAlias(_)
                    | MirInstruction::Store(_)
                    | MirInstruction::EndFullExpression(_) => {}
                }
            }
        }
    }
    Ok((data_layout, dispatch))
}

fn reject_unsupported_type_operations(program: &MirProgram) -> Result<(), BackendError> {
    for function in program.executable_definitions() {
        for block in &function.body().blocks {
            if matches!(
                block.terminator,
                Some(MirTerminator::CheckedNarrow { .. } | MirTerminator::Terminate { .. })
            ) || block
                .instructions
                .iter()
                .any(|instruction| match instruction {
                    MirInstruction::Assign(assignment) => {
                        matches!(assignment.rvalue.kind, MirRvalueKind::TypeTest { .. })
                    }
                    MirInstruction::BindNarrowedAlias(_) | MirInstruction::EndNarrowedAlias(_) => {
                        true
                    }
                    _ => false,
                })
            {
                return Err(type_operations_unavailable(function.callable()));
            }
        }
    }
    Ok(())
}

fn type_operations_unavailable(function: CallableId) -> BackendError {
    BackendError::new(
        Target::X86_64SysV,
        Some(function),
        "runtime type operations are not supported by the x86-64 backend yet",
    )
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
    if classify(signature.parameters, true).is_none() {
        return Err(abi_limit(caller, "outgoing argument area"));
    }
    Ok(())
}

fn classify(parameters: &[MirParameter], has_receiver: bool) -> Option<abi::CallLayout> {
    if has_receiver {
        abi::CallLayout::classify_with_receiver(parameters)
    } else {
        abi::CallLayout::classify(parameters)
    }
}

fn abi_limit(function: CallableId, area: &str) -> BackendError {
    BackendError::new(
        Target::X86_64SysV,
        Some(function),
        format!("{area} exceeds the x86-64 ABI encoding limits"),
    )
}
