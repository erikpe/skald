use super::{PilotError, UnsupportedPilot};
use crate::backend::{BackendInput, BackendRequiredRuntimeEntity, RuntimeTracePolicy};
use crate::{identity::CallableId, intrinsic::Intrinsic, mir::*};

fn unsupported(callable: Option<CallableId>, reason: impl Into<String>) -> PilotError {
    PilotError::Unsupported(UnsupportedPilot {
        callable,
        reason: reason.into(),
    })
}

/// An exhaustive whitelist of executable forms; new MIR forms require review.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn check(input: BackendInput<'_>) -> Result<(), PilotError> {
    let program = input.program();
    if !input.active_static_fields().is_empty() {
        return Err(unsupported(None, "active static storage"));
    }
    // Complete emission also retains generated metadata/lifecycle families.
    if !input.reachable_artifacts_only()
        && (!program.classes.is_empty()
            || !program.array_types.is_empty()
            || !program.optional_types.is_empty()
            || !program.optional_box_types.is_empty())
    {
        return Err(unsupported(
            None,
            "complete emission requires unsupported lifecycle/metadata families",
        ));
    }
    for entity in input.required_runtime_entities() {
        if !matches!(entity, BackendRequiredRuntimeEntity::FunctionType(_)) {
            return Err(unsupported(
                None,
                format!("unsupported required runtime entity {entity:?}"),
            ));
        }
    }
    for definition in program.executable_definitions() {
        let owner = Some(definition.callable());
        if definition.receiver().is_some()
            || matches!(definition, MirDefinitionRef::StaticInitializer(_))
        {
            return Err(unsupported(
                owner,
                "receiver-bearing or static initializer body",
            ));
        }
        for storage in definition.storage_entries() {
            scalar(program, storage.ty, owner)?;
            if !matches!(
                storage.kind,
                MirStorageKind::Local
                    | MirStorageKind::Parameter
                    | MirStorageKind::Temporary
                    | MirStorageKind::ScalarSpill
                    | MirStorageKind::PathCondition
                    | MirStorageKind::NormalizedPathActivation
            ) {
                return Err(unsupported(
                    owner,
                    format!("unsupported storage kind {:?}", storage.kind),
                ));
            }
        }
        for value in definition.values() {
            scalar(program, value.ty, owner)?;
        }
        let signature = program
            .callable_signature(definition.callable())
            .expect("verified signature");
        signature_check(program, signature.parameters, signature.return_type, owner)?;
        for block in &definition.body().blocks {
            for instruction in &block.instructions {
                match instruction {
                    MirInstruction::StorageLive(_) | MirInstruction::StorageDead(_) => {}
                    MirInstruction::Store(store) => place(&store.destination, owner)?,
                    MirInstruction::EndFullExpression(end) if end.temporaries.is_empty() => {}
                    MirInstruction::Assign(assign) => match &assign.rvalue.kind {
                        MirRvalueKind::Load(source) => place(source, owner)?,
                        MirRvalueKind::CallableAddress(address) => {
                            callable(program, address.target, owner)?;
                            let ty = program
                                .function_type(address.function_type)
                                .expect("verified function type");
                            signature_check(program, &ty.parameters, ty.result, owner)?;
                        }
                        MirRvalueKind::ConstantI64(_)
                        | MirRvalueKind::ConstantU64(_)
                        | MirRvalueKind::ConstantU8(_)
                        | MirRvalueKind::ConstantF64Bits(_)
                        | MirRvalueKind::ConstantBool(_)
                        | MirRvalueKind::PathCondition(_)
                        | MirRvalueKind::Unary { .. }
                        | MirRvalueKind::Binary { .. }
                        | MirRvalueKind::IntegerDivision { .. }
                        | MirRvalueKind::Shift { .. }
                        | MirRvalueKind::PrimitiveComparison { .. }
                        | MirRvalueKind::PrimitiveCast { .. }
                        | MirRvalueKind::CheckedF64ToInteger { .. } => {}
                        other => {
                            return Err(unsupported(owner, format!("unsupported rvalue {other:?}")))
                        }
                    },
                    MirInstruction::Call(call) => {
                        if call.receiver.is_some()
                            || call.destination.is_some()
                            || call.shared_result.is_some()
                            || call
                                .arguments
                                .iter()
                                .any(|argument| !matches!(argument, MirArgument::Value(_)))
                        {
                            return Err(unsupported(
                                owner,
                                "receiver, aggregate result or alias call argument",
                            ));
                        }
                        match call.target {
                            MirCallTarget::Static(method) => {
                                callable(program, method.into(), owner)?
                            }
                            MirCallTarget::Direct(function) => {
                                callable(program, function.into(), owner)?
                            }
                            MirCallTarget::Indirect(target) => {
                                let ty = program
                                    .function_type(target.function_type)
                                    .expect("verified indirect signature");
                                signature_check(program, &ty.parameters, ty.result, owner)?;
                            }
                            _ => return Err(unsupported(owner, "member dispatch")),
                        }
                    }
                    other => {
                        return Err(unsupported(
                            owner,
                            format!("unsupported instruction {other:?}"),
                        ))
                    }
                }
            }
            match block.terminator.as_ref().expect("verified terminator") {
                MirTerminator::Return { .. }
                | MirTerminator::Goto { .. }
                | MirTerminator::Branch { .. }
                | MirTerminator::ShiftCountCheck { .. }
                | MirTerminator::IntegerDivisorCheck { .. }
                | MirTerminator::PrimitiveCastRangeCheck { .. } => {}
                MirTerminator::Terminate {
                    reason:
                        MirTerminationReason::ShiftCountOutOfRange
                        | MirTerminationReason::IntegerDivisionByZero
                        | MirTerminationReason::IntegerRemainderByZero
                        | MirTerminationReason::PrimitiveCastOutOfRange,
                    ..
                } => {}
                other => {
                    return Err(unsupported(
                        owner,
                        format!("unsupported terminator {other:?}"),
                    ))
                }
            }
        }
    }
    // Source access is checked during enabled metadata planning, never here.
    debug_assert!(
        input.runtime_trace() != RuntimeTracePolicy::Omitted || input.sources().is_none()
    );
    Ok(())
}

fn place(place: &MirPlace, owner: Option<CallableId>) -> Result<(), PilotError> {
    if !matches!(place.base, MirPlaceBase::Storage(_)) || !place.projections.is_empty() {
        return Err(unsupported(owner, "nonlocal, projected or alias place"));
    }
    Ok(())
}

fn callable(
    program: &MirProgram,
    id: CallableId,
    owner: Option<CallableId>,
) -> Result<(), PilotError> {
    if let CallableId::Method(method) = id {
        let declaration = program.method(method).expect("verified method declaration");
        if declaration.kind != MirMethodKind::Static {
            return Err(unsupported(owner, "receiver-bearing callable"));
        }
        return signature_check(
            program,
            &declaration.parameters,
            declaration.return_type,
            owner,
        );
    }
    let CallableId::Function(function) = id else {
        return Err(unsupported(owner, "receiver-bearing callable address"));
    };
    let declaration = program
        .declarations
        .get(function)
        .expect("verified callable declaration");
    match declaration.linkage {
        MirFunctionLinkage::Intrinsic {
            intrinsic: Intrinsic::F64ToBits | Intrinsic::F64FromBits,
        } => {}
        MirFunctionLinkage::Intrinsic { intrinsic } => {
            return Err(unsupported(
                owner,
                format!("unsupported intrinsic {intrinsic:?}"),
            ))
        }
        MirFunctionLinkage::External { .. }
            if declaration.parameters.iter().any(|p| !p.ty.is_primitive())
                || !(declaration.return_type.is_primitive()
                    || declaration.return_type == MirType::Unit) =>
        {
            return Err(unsupported(owner, "non-scalar C external signature"))
        }
        _ => {}
    }
    signature_check(
        program,
        &declaration.parameters,
        declaration.return_type,
        owner,
    )
}

fn signature_check(
    program: &MirProgram,
    parameters: &[MirParameter],
    result: MirType,
    owner: Option<CallableId>,
) -> Result<(), PilotError> {
    for parameter in parameters {
        if parameter.mode != MirParameterMode::Value {
            return Err(unsupported(owner, "alias parameter"));
        }
        scalar(program, parameter.ty, owner)?;
    }
    scalar(program, result, owner)
}

fn scalar(program: &MirProgram, ty: MirType, owner: Option<CallableId>) -> Result<(), PilotError> {
    match ty {
        MirType::I64
        | MirType::U64
        | MirType::U8
        | MirType::Bool
        | MirType::F64
        | MirType::Unit => Ok(()),
        MirType::Function(id) => {
            // Canonical source types are finite structural signatures. Avoid a recursive
            // walker so a deep higher-order signature cannot exhaust the host stack.
            let mut pending = vec![id];
            let mut seen = std::collections::BTreeSet::new();
            while let Some(id) = pending.pop() {
                if !seen.insert(id) {
                    continue;
                }
                let signature = program.function_type(id).expect("verified function type");
                for parameter in &signature.parameters {
                    if parameter.mode != MirParameterMode::Value {
                        return Err(unsupported(owner, "function-pointer alias parameter"));
                    }
                }
                for ty in signature
                    .parameters
                    .iter()
                    .map(|p| p.ty)
                    .chain([signature.result])
                {
                    match ty {
                        MirType::Function(id) => pending.push(id),
                        MirType::I64
                        | MirType::U64
                        | MirType::U8
                        | MirType::Bool
                        | MirType::F64
                        | MirType::Unit => {}
                        _ => {
                            return Err(unsupported(
                                owner,
                                format!("unsupported function-pointer payload {ty}"),
                            ))
                        }
                    }
                }
            }
            Ok(())
        }
        _ => Err(unsupported(owner, format!("unsupported payload {ty}"))),
    }
}
