//! Target legality checks performed before instruction selection.

use crate::{
    backend::{BackendError, Target},
    identity::CallableId,
    mir::{verify_mir, MirArgument, MirCallTarget, MirInstruction, MirParameter, MirProgram},
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

    if uses_static_polymorphism(program) {
        return Err(BackendError::new(
            Target::X86_64SysV,
            None,
            "static inheritance and object views are not supported by the x86-64 backend yet",
        ));
    }

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
                        MirCallTarget::Method(method) => {
                            check_member_target(program, function.callable(), method.into())?;
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
                    | MirInstruction::Store(_)
                    | MirInstruction::EndFullExpression(_) => {}
                }
            }
        }
    }
    Ok(data_layout)
}

fn uses_static_polymorphism(program: &MirProgram) -> bool {
    program
        .classes
        .iter()
        .any(|class| class.direct_base.is_some())
        || program
            .declarations
            .iter()
            .flat_map(|declaration| {
                declaration
                    .parameters
                    .iter()
                    .map(|parameter| parameter.ty)
                    .chain(std::iter::once(declaration.return_type))
            })
            .chain(program.classes.iter().flat_map(|class| {
                class
                    .initializers
                    .iter()
                    .flat_map(|initializer| {
                        initializer.parameters.iter().map(|parameter| parameter.ty)
                    })
                    .chain(class.methods.iter().flat_map(|method| {
                        method
                            .parameters
                            .iter()
                            .map(|parameter| parameter.ty)
                            .chain(std::iter::once(method.return_type))
                    }))
            }))
            .any(|ty| ty == crate::mir::MirType::Obj)
        || program.executable_definitions().any(|definition| {
            definition.body().blocks.iter().any(|block| {
                block.instructions.iter().any(|instruction| {
                    let arguments = match instruction {
                        MirInstruction::Call(call) => &call.arguments,
                        MirInstruction::Initialize(initialize) => &initialize.arguments,
                        _ => return false,
                    };
                    arguments
                        .iter()
                        .any(|argument| matches!(argument, MirArgument::View(_)))
                })
            })
        })
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
