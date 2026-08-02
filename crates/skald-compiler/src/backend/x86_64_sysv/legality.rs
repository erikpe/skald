//! Target legality checks performed before instruction selection.

use crate::{
    backend::{BackendError, Target},
    identity::CallableId,
    mir::{
        verify_mir, MirCallTarget, MirInstruction, MirMethodCallTarget, MirMethodKind,
        MirParameter, MirProgram,
    },
};

use super::{abi, array_legality, dispatch::DispatchMetadata, layout::DataLayout};

pub(super) fn check(program: &MirProgram) -> Result<(DataLayout, DispatchMetadata), BackendError> {
    verify_mir(program).map_err(|errors| {
        BackendError::new(
            Target::X86_64SysV,
            None,
            format!("input MIR failed verification:\n{errors}"),
        )
    })?;
    reject_unimplemented_standard_io(program)?;
    array_legality::check(program)?;
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
                    MirInstruction::StorageLive(_) | MirInstruction::StorageDead(_) => {}
                    MirInstruction::Initialize(initialize) => {
                        check_member_target(
                            program,
                            function.callable(),
                            initialize.target.into(),
                        )?;
                    }
                    MirInstruction::SharedAllocate(allocation) => {
                        data_layout.shared_allocation_size(allocation.class)?;
                    }
                    MirInstruction::SharedInitialize(initialize) => {
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
                        MirCallTarget::Static(method) => {
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
                    MirInstruction::Assign(assignment) => {
                        if let crate::mir::MirRvalueKind::PrimitiveCast { operation, .. } =
                            assignment.rvalue.kind
                        {
                            if !primitive_cast_is_supported(operation) {
                                return Err(BackendError::new(
                                    Target::X86_64SysV,
                                    Some(function.callable()),
                                    format!(
                                        "primitive cast `{} -> {}` is not yet supported by this target",
                                        operation.source, operation.target
                                    ),
                                ));
                            }
                        }
                    }
                    MirInstruction::BindCheckedView(_)
                    | MirInstruction::EndCheckedView(_)
                    | MirInstruction::Store(_)
                    | MirInstruction::EndFullExpression(_) => {}
                    MirInstruction::SharedPublish(_)
                    | MirInstruction::SharedStatic(_)
                    | MirInstruction::SharedAdopt(_)
                    | MirInstruction::SharedCopy(_)
                    | MirInstruction::SharedFieldCopy(_)
                    | MirInstruction::SharedCast(_)
                    | MirInstruction::SharedMove(_)
                    | MirInstruction::SharedRelease(_)
                    | MirInstruction::SharedFieldInitialize(_)
                    | MirInstruction::OptionalInitialize(_)
                    | MirInstruction::OptionalSharedInitialize(_)
                    | MirInstruction::OptionalSharedAssign(_)
                    | MirInstruction::OptionalSharedCleanup(_)
                    | MirInstruction::SharedFieldReplace(_)
                    | MirInstruction::StringInitialize(_)
                    | MirInstruction::OptionalAssign(_)
                    | MirInstruction::ClassOptionalInitialize(_)
                    | MirInstruction::ClassOptionalAssign(_)
                    | MirInstruction::ClassOptionalPublish(_)
                    | MirInstruction::ClassOptionalCleanup(_)
                    | MirInstruction::EndOptionalView(_) => {}
                    MirInstruction::Array(_) => {}
                    MirInstruction::Io(_) => {
                        return Err(BackendError::new(
                            Target::X86_64SysV,
                            Some(function.callable()),
                            "standard-I/O MIR is not yet supported by the x86-64 target",
                        ));
                    }
                }
            }
        }
    }
    Ok((data_layout, dispatch))
}

fn reject_unimplemented_standard_io(program: &MirProgram) -> Result<(), BackendError> {
    for function in program.executable_definitions() {
        if function
            .body()
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(instruction, MirInstruction::Io(_)))
        {
            return Err(BackendError::new(
                Target::X86_64SysV,
                Some(function.callable()),
                "standard-I/O MIR is not yet supported by the x86-64 target",
            ));
        }
    }
    Ok(())
}

fn primitive_cast_is_supported(operation: crate::mir::MirPrimitiveCast) -> bool {
    use crate::mir::MirPrimitiveCastKind;

    match operation.kind() {
        MirPrimitiveCastKind::Identity | MirPrimitiveCastKind::IntegerBits => true,
        MirPrimitiveCastKind::ToBool | MirPrimitiveCastKind::FromBool => true,
        MirPrimitiveCastKind::ToF64 => true,
        MirPrimitiveCastKind::CheckedF64ToInteger => false,
    }
}

fn check_member_target(
    program: &MirProgram,
    caller: CallableId,
    target: CallableId,
) -> Result<(), BackendError> {
    let Some(definition) = program.member_definition(target) else {
        return Err(BackendError::new(
            Target::X86_64SysV,
            Some(caller),
            format!("member call target {target} has no MIR definition"),
        ));
    };
    let signature = program
        .callable_signature(target)
        .expect("verified member target must be declared");
    let has_receiver = match target {
        CallableId::Method(method) => program
            .method(method)
            .is_some_and(|method| matches!(method.kind, MirMethodKind::Instance { .. })),
        CallableId::Initializer(_)
        | CallableId::CopyConstructor(_)
        | CallableId::CopyAssignment(_)
        | CallableId::Destructor(_) => true,
        CallableId::Function(_) => unreachable!("member target cannot be a top-level function"),
    };
    debug_assert_eq!(definition.receiver.is_some(), has_receiver);
    if classify(signature.parameters, has_receiver).is_none() {
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
