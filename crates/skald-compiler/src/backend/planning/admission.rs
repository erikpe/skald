use super::{facts::UnsupportedProgram, AdmissionError};
use crate::backend::{BackendInput, BackendRequiredRuntimeEntity, RuntimeTracePolicy};
use crate::{identity::CallableId, intrinsic::Intrinsic, mir::*};

fn unsupported(callable: Option<CallableId>, reason: impl Into<String>) -> AdmissionError {
    AdmissionError::Unsupported(UnsupportedProgram {
        callable,
        reason: reason.into(),
    })
}

/// An exhaustive whitelist of executable forms; new MIR forms require review.
pub(super) fn check(input: BackendInput<'_>) -> Result<(), AdmissionError> {
    let program = input.program();
    if !input.active_static_fields().is_empty() {
        return Err(unsupported(None, "active static storage"));
    }
    // Complete emission retains every declared generated lifecycle family.
    // Class metadata, copy helpers, finalizers and shared-owner helpers are
    // complete; wrapper/container helper families remain staged separately.
    if !input.reachable_artifacts_only()
        && (!program.array_types.is_empty()
            || !program.optional_types.is_empty()
            || !program.optional_box_types.is_empty())
    {
        return Err(unsupported(
            None,
            "complete emission requires unsupported lifecycle/metadata families",
        ));
    }
    for entity in input.required_runtime_entities() {
        if !matches!(
            entity,
            BackendRequiredRuntimeEntity::FunctionType(_)
                | BackendRequiredRuntimeEntity::ClassDispatch(_)
                | BackendRequiredRuntimeEntity::VirtualFamily(_)
                | BackendRequiredRuntimeEntity::InterfaceRequirement(_)
                | BackendRequiredRuntimeEntity::ArrayLifecycle(_)
                | BackendRequiredRuntimeEntity::OptionalLifecycle(_)
                | BackendRequiredRuntimeEntity::OptionalBoxLayout(_)
                | BackendRequiredRuntimeEntity::LiteralBacking(_)
        ) {
            return Err(unsupported(
                None,
                format!("unsupported required runtime entity {entity:?}"),
            ));
        }
    }
    for definition in program.executable_definitions() {
        let owner = Some(definition.callable());
        if matches!(definition, MirDefinitionRef::StaticInitializer(_)) {
            return Err(unsupported(owner, "static initializer body"));
        }
        for storage in definition.storage_entries() {
            payload(program, storage.ty, owner)?;
            if !matches!(
                storage.kind,
                MirStorageKind::Return
                    | MirStorageKind::Receiver
                    | MirStorageKind::Parameter
                    | MirStorageKind::AliasParameter(_)
                    | MirStorageKind::Local
                    | MirStorageKind::Argument
                    | MirStorageKind::Temporary
                    | MirStorageKind::SharedAnchor
                    | MirStorageKind::SharedAllocation
                    | MirStorageKind::ScalarSpill
                    | MirStorageKind::PrimitiveAlias
                    | MirStorageKind::CheckedView(_)
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
            payload(program, value.ty, owner)?;
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
                    MirInstruction::EndFullExpression(end)
                        if end
                            .temporaries
                            .iter()
                            .all(|cleanup| supported_cleanup(program, cleanup.target)) => {}
                    MirInstruction::Cleanup(cleanup)
                        if supported_cleanup(program, cleanup.target) => {}
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
                        MirRvalueKind::TypeTest { source, .. } => {
                            place(&source.source, owner)?;
                            origin(&source.origin, owner)?;
                        }
                        other => {
                            return Err(unsupported(owner, format!("unsupported rvalue {other:?}")))
                        }
                    },
                    MirInstruction::Call(call) => {
                        if let Some(destination) = &call.destination {
                            place(destination, owner)?;
                        }
                        if let Some(receiver) = &call.receiver {
                            match receiver {
                                MirCallReceiver::Method(receiver) => {
                                    place(&receiver.place, owner)?;
                                    origin(&receiver.origin, owner)?;
                                }
                                MirCallReceiver::Interface(view) => {
                                    place(&view.source, owner)?;
                                    origin(&view.origin, owner)?;
                                }
                            }
                        }
                        for argument in &call.arguments {
                            match argument {
                                MirArgument::Value(_) | MirArgument::SharedOwner(_) => {}
                                MirArgument::Place(place) | MirArgument::OwnedPlace(place) => {
                                    self::place(place, owner)?
                                }
                                MirArgument::View(view) => {
                                    self::place(&view.source, owner)?;
                                    origin(&view.origin, owner)?;
                                }
                            }
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
                            MirCallTarget::Method(MirMethodCallTarget::Direct(method)) => {
                                callable(program, method.into(), owner)?
                            }
                            MirCallTarget::Method(MirMethodCallTarget::Virtual {
                                selected,
                                ..
                            }) => callable(program, selected.into(), owner)?,
                            MirCallTarget::Interface(target) => {
                                let requirement = program
                                    .interface_requirement(target.requirement)
                                    .expect("verified interface requirement");
                                signature_check(
                                    program,
                                    &requirement.parameters,
                                    requirement.return_type,
                                    owner,
                                )?;
                            }
                        }
                    }
                    MirInstruction::Initialize(initialize) => {
                        place(&initialize.destination, owner)?;
                        let declaration = program
                            .initializer(initialize.target)
                            .expect("verified initializer declaration");
                        signature_check(program, &declaration.parameters, MirType::Unit, owner)?;
                        arguments(&initialize.arguments, owner)?;
                    }
                    MirInstruction::CopyConstruct(copy)
                        if supported_constructor_copy(program, copy.operation) =>
                    {
                        place(&copy.destination, owner)?;
                        place(&copy.source, owner)?;
                    }
                    MirInstruction::CopyAssign(copy)
                        if supported_assignment_copy(program, copy.operation) =>
                    {
                        place(&copy.destination, owner)?;
                        place(&copy.source, owner)?;
                    }
                    MirInstruction::BindCheckedView(binding) => {
                        place(&binding.view.source, owner)?;
                        origin(&binding.view.origin, owner)?;
                    }
                    MirInstruction::EndCheckedView(_) => {}
                    MirInstruction::SharedAllocate(allocation)
                        if matches!(allocation.target, MirSharedAllocationTarget::Class(_)) =>
                    {
                        if let MirSharedAllocationMode::Copy { source } = &allocation.mode {
                            place(source, owner)?;
                        }
                    }
                    MirInstruction::SharedInitialize(initialize) => {
                        let declaration = program
                            .initializer(initialize.target)
                            .expect("verified shared initializer declaration");
                        signature_check(program, &declaration.parameters, MirType::Unit, owner)?;
                        arguments(&initialize.arguments, owner)?;
                    }
                    MirInstruction::SharedPublish(_) | MirInstruction::SharedAdopt(_) => {}
                    MirInstruction::SharedStatic(_) => {}
                    MirInstruction::SharedCopy(_) | MirInstruction::SharedMove(_) => {}
                    MirInstruction::SharedFieldCopy(copy) => place(&copy.source, owner)?,
                    MirInstruction::SharedCast(cast) if supported_shared_cast(cast) => {
                        shared_cast_source(&cast.source, owner)?;
                    }
                    MirInstruction::SharedRelease(_) => {}
                    MirInstruction::SharedFieldInitialize(initialize) => {
                        place(&initialize.destination, owner)?;
                    }
                    MirInstruction::SharedFieldReplace(replace) => {
                        place(&replace.destination, owner)?;
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
                        | MirTerminationReason::PrimitiveCastOutOfRange
                        | MirTerminationReason::ObjectCastFailure,
                    ..
                } => {}
                MirTerminator::CheckedCast { binding, .. } => {
                    place(&binding.view.source, owner)?;
                    origin(&binding.view.origin, owner)?;
                }
                MirTerminator::ReturnShared { .. } => {}
                MirTerminator::SharedCast { cast, .. } if supported_shared_cast(cast) => {
                    shared_cast_source(&cast.source, owner)?;
                }
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

fn supported_cleanup(program: &MirProgram, root: crate::identity::ClassId) -> bool {
    let mut pending = vec![root];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(class) = pending.pop() {
        if !seen.insert(class) {
            continue;
        }
        let Some(class) = program.classes.get(class) else {
            return false;
        };
        for step in &class.destruction.steps {
            match *step {
                MirDestructionStep::UserBody(_) | MirDestructionStep::SharedField(_) => {}
                MirDestructionStep::Base(base) => pending.push(base),
                MirDestructionStep::Field(field) => match program
                    .class(field.class())
                    .and_then(|class| class.field(field))
                    .map(|field| field.ty)
                {
                    Some(MirType::Class(class)) => pending.push(class),
                    _ => return false,
                },
                _ => return false,
            }
        }
    }
    true
}

fn supported_constructor_copy(
    program: &MirProgram,
    root: MirSelectedCopyOperation<crate::identity::CopyConstructorId>,
) -> bool {
    let mut pending = vec![root];
    while let Some(operation) = pending.pop() {
        let class = match operation {
            MirSelectedCopyOperation::User(id) => id.class(),
            MirSelectedCopyOperation::Synthesized(class) => class,
        };
        let Some(capability) = program.class(class).map(|class| &class.copy_constructor) else {
            return false;
        };
        match (operation, capability) {
            (MirSelectedCopyOperation::User(id), MirCopyCapability::User(copy))
                if copy.operation == id =>
            {
                if let Some(base) = copy.base {
                    pending.push(base.operation);
                }
            }
            (
                MirSelectedCopyOperation::Synthesized(selected),
                MirCopyCapability::Synthesized(copy),
            ) if copy.class == selected => {
                if let Some(base) = copy.base {
                    pending.push(base.operation);
                }
                for field in &copy.fields {
                    match *field {
                        MirSynthesizedFieldCopy::Scalar { .. }
                        | MirSynthesizedFieldCopy::Shared { .. } => {}
                        MirSynthesizedFieldCopy::Class { operation, .. } => pending.push(operation),
                        _ => return false,
                    }
                }
            }
            _ => return false,
        }
    }
    true
}

fn supported_assignment_copy(
    program: &MirProgram,
    root: MirSelectedCopyOperation<crate::identity::CopyAssignmentId>,
) -> bool {
    let mut pending = vec![root];
    while let Some(operation) = pending.pop() {
        let class = match operation {
            MirSelectedCopyOperation::User(id) => id.class(),
            MirSelectedCopyOperation::Synthesized(class) => class,
        };
        let Some(capability) = program.class(class).map(|class| &class.copy_assignment) else {
            return false;
        };
        match (operation, capability) {
            (MirSelectedCopyOperation::User(id), MirCopyCapability::User(copy))
                if copy.operation == id =>
            {
                if let Some(base) = copy.base {
                    pending.push(base.operation);
                }
            }
            (
                MirSelectedCopyOperation::Synthesized(selected),
                MirCopyCapability::Synthesized(copy),
            ) if copy.class == selected => {
                if let Some(base) = copy.base {
                    pending.push(base.operation);
                }
                for field in &copy.fields {
                    match *field {
                        MirSynthesizedFieldCopy::Scalar { .. }
                        | MirSynthesizedFieldCopy::Shared { .. } => {}
                        MirSynthesizedFieldCopy::Class { operation, .. } => pending.push(operation),
                        _ => return false,
                    }
                }
            }
            _ => return false,
        }
    }
    true
}

fn place(place: &MirPlace, owner: Option<CallableId>) -> Result<(), AdmissionError> {
    if matches!(place.base, MirPlaceBase::ArrayAlias(_)) {
        return Err(unsupported(
            owner,
            "array-alias place before its binding owner",
        ));
    }
    Ok(())
}

fn supported_shared_cast(cast: &MirSharedCast) -> bool {
    !matches!(
        cast.target,
        MirSharedTarget::Array(_) | MirSharedTarget::OptionalBox(_)
    ) && !matches!(
        cast.source.target(),
        MirSharedTarget::Array(_) | MirSharedTarget::OptionalBox(_)
    )
}

fn shared_cast_source(
    source: &MirSharedCastSource,
    owner: Option<CallableId>,
) -> Result<(), AdmissionError> {
    match source {
        MirSharedCastSource::Owner { .. } => Ok(()),
        MirSharedCastSource::Field { place: source, .. } => place(source, owner),
    }
}

fn arguments(arguments: &[MirArgument], owner: Option<CallableId>) -> Result<(), AdmissionError> {
    for argument in arguments {
        match argument {
            MirArgument::Value(_) | MirArgument::SharedOwner(_) => {}
            MirArgument::Place(argument_place) | MirArgument::OwnedPlace(argument_place) => {
                place(argument_place, owner)?
            }
            MirArgument::View(view) => {
                place(&view.source, owner)?;
                origin(&view.origin, owner)?;
            }
        }
    }
    Ok(())
}

fn origin(origin: &MirObjectOrigin, owner: Option<CallableId>) -> Result<(), AdmissionError> {
    match origin {
        MirObjectOrigin::Exact { complete, .. } => place(complete, owner),
        MirObjectOrigin::Forwarded { .. } | MirObjectOrigin::Shared { .. } => Ok(()),
    }
}

fn callable(
    program: &MirProgram,
    id: CallableId,
    owner: Option<CallableId>,
) -> Result<(), AdmissionError> {
    if let CallableId::Method(method) = id {
        let declaration = program.method(method).expect("verified method declaration");
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
) -> Result<(), AdmissionError> {
    for parameter in parameters {
        payload(program, parameter.ty, owner)?;
    }
    payload(program, result, owner)
}

fn payload(
    program: &MirProgram,
    ty: MirType,
    _owner: Option<CallableId>,
) -> Result<(), AdmissionError> {
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
                        | MirType::Unit
                        | MirType::Array(_)
                        | MirType::Class(_)
                        | MirType::Interface(_)
                        | MirType::Obj
                        | MirType::Shared(_)
                        | MirType::Optional(_) => {}
                    }
                }
            }
            Ok(())
        }
        MirType::Array(_)
        | MirType::Class(_)
        | MirType::Interface(_)
        | MirType::Obj
        | MirType::Shared(_)
        | MirType::Optional(_) => Ok(()),
    }
}
