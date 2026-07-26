//! Legality boundary for the first executable primitive inline-array profile.

use crate::{
    backend::{BackendError, Target},
    mir::{
        MirArrayAnchorKind, MirArrayDefaultElement, MirArrayFailure, MirArrayInstruction,
        MirArrayOwnership, MirArrayPositionKind, MirDefinitionRef, MirInstruction,
        MirParameterMode, MirPlace, MirPlaceBase, MirPlaceProjection, MirProgram, MirRvalueKind,
        MirStorageKind, MirTerminationReason, MirTerminator, MirType,
    },
};

pub(super) fn check(program: &MirProgram) -> Result<(), BackendError> {
    if program.array_types.is_empty() {
        return Ok(());
    }

    for array in program.array_types.iter() {
        if !matches!(
            array.element,
            MirType::I64 | MirType::U64 | MirType::U8 | MirType::F64 | MirType::Bool
        ) {
            return Err(error(
                None,
                format!(
                    "array {} has a non-primitive element type not yet supported by x86-64",
                    array.id
                ),
            ));
        }
    }
    for definition in program.executable_definitions() {
        check_definition(program, definition)?;
    }
    Ok(())
}

fn check_definition(
    program: &MirProgram,
    definition: MirDefinitionRef<'_>,
) -> Result<(), BackendError> {
    let signature = program
        .callable_signature(definition.callable())
        .expect("verified definition has a signature");
    if signature.parameters.iter().any(|parameter| {
        matches!(parameter.ty, MirType::Array(_)) && parameter.mode != MirParameterMode::Value
    }) {
        return Err(error(
            Some(definition.callable()),
            "array alias parameters are not yet supported by the x86-64 backend",
        ));
    }

    for storage in definition.storage_entries() {
        if let MirType::Array(_) = storage.ty {
            let supported = matches!(
                storage.kind,
                MirStorageKind::Local
                    | MirStorageKind::Parameter
                    | MirStorageKind::Return
                    | MirStorageKind::Argument
                    | MirStorageKind::ArrayBacking
                    | MirStorageKind::ArrayProduced
                    | MirStorageKind::ArrayAnchor(
                        MirArrayAnchorKind::InlineOwner | MirArrayAnchorKind::InlineBacking
                    )
            );
            if !supported {
                return Err(error(
                    Some(definition.callable()),
                    "only local primitive inline-array storage is executable on x86-64",
                ));
            }
        }
    }

    for block in &definition.body().blocks {
        for instruction in &block.instructions {
            check_instruction(program, definition, instruction)?;
        }
        check_terminator(definition, block.terminator.as_ref().unwrap())?;
    }
    Ok(())
}

fn check_instruction(
    program: &MirProgram,
    definition: MirDefinitionRef<'_>,
    instruction: &MirInstruction,
) -> Result<(), BackendError> {
    match instruction {
        MirInstruction::Assign(assignment) => match &assignment.rvalue.kind {
            MirRvalueKind::ArrayLength { source, .. } => {
                require_inline_array_place(program, definition, source)?;
            }
            MirRvalueKind::Load(source) if has_array_element_projection(source) => {
                require_primitive_element_place(program, definition, source)?;
            }
            _ => {}
        },
        MirInstruction::Store(store) if has_array_element_projection(&store.destination) => {
            require_primitive_element_place(program, definition, &store.destination)?;
        }
        MirInstruction::Array(array) => match array {
            MirArrayInstruction::Allocate {
                ownership: MirArrayOwnership::Inline,
                failure: MirArrayFailure::AllocationSize,
                ..
            }
            | MirArrayInstruction::InitializeNext {
                operation: MirArrayDefaultElement::Primitive,
                ..
            }
            | MirArrayInstruction::Publish { .. } => {}
            MirArrayInstruction::CopyNext {
                source,
                operation: crate::mir::MirArrayCopyElement::Primitive,
                ..
            } => {
                require_inline_array_place(program, definition, source)?;
            }
            MirArrayInstruction::Adopt { destination, .. } => {
                require_inline_array_place(program, definition, destination)?;
            }
            MirArrayInstruction::Replace { destination, .. } => {
                require_inline_array_place(program, definition, destination)?;
            }
            MirArrayInstruction::Release { owner, .. } => {
                require_inline_array_place(program, definition, owner)?;
            }
            MirArrayInstruction::AnchorBegin {
                owner,
                kind: MirArrayAnchorKind::InlineOwner | MirArrayAnchorKind::InlineBacking,
                ..
            } => {
                require_inline_array_place(program, definition, owner)?;
            }
            MirArrayInstruction::AnchorEnd { .. } => {}
            MirArrayInstruction::Normalize {
                owner,
                kind: MirArrayPositionKind::Element,
                ..
            } => {
                require_inline_array_place(program, definition, owner)?;
            }
            _ => {
                return Err(error(
                    Some(definition.callable()),
                    "verified array operation is outside the primitive inline-array execution profile",
                ));
            }
        },
        _ => {}
    }
    Ok(())
}

fn check_terminator(
    definition: MirDefinitionRef<'_>,
    terminator: &MirTerminator,
) -> Result<(), BackendError> {
    let supported = match terminator {
        MirTerminator::ArrayOperationCheck {
            failure: MirArrayFailure::AllocationSize,
            ..
        }
        | MirTerminator::ArrayPositionCheck {
            kind: MirArrayPositionKind::Element,
            ..
        }
        | MirTerminator::ArrayLoop { .. }
        | MirTerminator::Terminate {
            reason:
                MirTerminationReason::ArrayAllocationFailure
                | MirTerminationReason::ArrayIndexOutOfBounds,
            ..
        } => true,
        MirTerminator::ArrayOperationCheck { .. }
        | MirTerminator::ArrayPositionCheck { .. }
        | MirTerminator::Terminate {
            reason:
                MirTerminationReason::ArrayInvalidSliceBounds
                | MirTerminationReason::ArraySliceLengthMismatch,
            ..
        } => false,
        _ => true,
    };
    if supported {
        Ok(())
    } else {
        Err(error(
            Some(definition.callable()),
            "verified array control flow is outside the primitive inline-array execution profile",
        ))
    }
}

fn require_primitive_element_place(
    program: &MirProgram,
    definition: MirDefinitionRef<'_>,
    place: &MirPlace,
) -> Result<(), BackendError> {
    let Some(MirPlaceProjection::ArrayElement { .. }) = place.projections.last() else {
        return Err(error(
            Some(definition.callable()),
            "only primitive array element places are executable on x86-64",
        ));
    };
    let mut owner = MirPlace {
        base: place.base,
        projections: place.projections.clone(),
    };
    owner.projections.pop();
    require_inline_array_place(program, definition, &owner)
}

fn has_array_element_projection(place: &MirPlace) -> bool {
    place
        .projections
        .iter()
        .any(|projection| matches!(projection, MirPlaceProjection::ArrayElement { .. }))
}

fn require_inline_array_place(
    program: &MirProgram,
    definition: MirDefinitionRef<'_>,
    place: &MirPlace,
) -> Result<(), BackendError> {
    let storage = definition
        .storage(place.base.storage())
        .expect("verified array place has declared storage");
    let direct_owner = place.projections.is_empty()
        && matches!(place.base, MirPlaceBase::Storage(_))
        && matches!(
            storage.kind,
            MirStorageKind::Local
                | MirStorageKind::Parameter
                | MirStorageKind::Return
                | MirStorageKind::Argument
                | MirStorageKind::ArrayProduced
        )
        && matches!(storage.ty, MirType::Array(_));
    let field_owner = !place.projections.is_empty()
        && !matches!(
            storage.ty,
            MirType::Array(_) | MirType::Shared(crate::mir::MirSharedTarget::Array(_))
        )
        && projected_type(program, storage.ty, place) // verified projections
            .is_some_and(|ty| matches!(ty, MirType::Array(_)));
    if direct_owner || field_owner {
        Ok(())
    } else {
        Err(error(
            Some(definition.callable()),
            "array place is outside the primitive inline owning-boundary profile",
        ))
    }
}

fn projected_type(program: &MirProgram, mut ty: MirType, place: &MirPlace) -> Option<MirType> {
    if matches!(
        place.base,
        MirPlaceBase::SharedPointee(_) | MirPlaceBase::SharedAllocationPayload(_)
    ) {
        let MirType::Shared(target) = ty else {
            return None;
        };
        ty = target.ty();
    }
    for projection in &place.projections {
        ty = match *projection {
            MirPlaceProjection::Base(base) => MirType::Class(base),
            MirPlaceProjection::Field(field) => program.field(field)?.ty,
            MirPlaceProjection::OptionalPayload(class) => MirType::Class(class),
            MirPlaceProjection::ArrayElement { array, .. } => program.array_type(array)?.element,
        };
    }
    Some(ty)
}

fn error(
    callable: Option<crate::identity::CallableId>,
    message: impl Into<String>,
) -> BackendError {
    BackendError::new(Target::X86_64SysV, callable, message)
}
