//! Legality boundary for executable inline and shared outer arrays.

use crate::{
    backend::{BackendError, Target},
    mir::{
        MirArrayAnchorKind, MirArrayDefaultElement, MirArrayFailure, MirArrayInstruction,
        MirArrayOwnership, MirArrayPositionKind, MirDefinitionRef, MirInstruction, MirPlace,
        MirPlaceBase, MirPlaceProjection, MirProgram, MirRvalueKind, MirStorageKind,
        MirTerminationReason, MirTerminator, MirType,
    },
};

pub(super) fn check(program: &MirProgram) -> Result<(), BackendError> {
    if program.array_types.is_empty() {
        return Ok(());
    }

    for array in program.array_types.iter() {
        if !matches!(
            array.element,
            MirType::I64
                | MirType::U64
                | MirType::U8
                | MirType::F64
                | MirType::Bool
                | MirType::OptionalPrimitive(_)
                | MirType::Class(_)
                | MirType::OptionalClass(_)
                | MirType::Array(_)
                | MirType::Shared(_)
                | MirType::OptionalShared(_)
        ) {
            return Err(error(
                None,
                format!(
                    "array {} has an element type not supported by x86-64",
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
                    | MirStorageKind::ArraySlice
                    | MirStorageKind::ArrayAnchor(_)
                    | MirStorageKind::ArrayAlias(_)
                    | MirStorageKind::AliasParameter(_)
            );
            if !supported {
                return Err(error(
                    Some(definition.callable()),
                    "array storage is outside the executable inline/shared array profile",
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
                require_executable_array_place(program, definition, source)?;
            }
            MirRvalueKind::Load(source) if has_array_element_projection(source) => {
                require_executable_element_place(program, definition, source)?;
            }
            _ => {}
        },
        MirInstruction::Store(store) if has_array_element_projection(&store.destination) => {
            require_executable_element_place(program, definition, &store.destination)?;
        }
        MirInstruction::Array(array) => match array {
            MirArrayInstruction::Allocate {
                ownership: MirArrayOwnership::Inline | MirArrayOwnership::Shared,
                failure: MirArrayFailure::AllocationSize,
                ..
            }
            | MirArrayInstruction::AllocateElements {
                ownership: MirArrayOwnership::Inline | MirArrayOwnership::Shared,
                failure: MirArrayFailure::AllocationSize,
                ..
            }
            | MirArrayInstruction::InitializeElement { .. }
            | MirArrayInstruction::Publish { .. }
            | MirArrayInstruction::PublishShared { .. } => {}
            MirArrayInstruction::InitializeNext {
                operation:
                    MirArrayDefaultElement::Primitive
                    | MirArrayDefaultElement::OptionalAbsent
                    | MirArrayDefaultElement::Class { .. }
                    | MirArrayDefaultElement::ArrayEmpty(_)
                    | MirArrayDefaultElement::SharedClass { .. }
                    | MirArrayDefaultElement::SharedArrayEmpty(_),
                ..
            } => {}
            MirArrayInstruction::CopyNext {
                source,
                operation:
                    crate::mir::MirArrayCopyElement::Primitive
                    | crate::mir::MirArrayCopyElement::OptionalPrimitive
                    | crate::mir::MirArrayCopyElement::Class { .. }
                    | crate::mir::MirArrayCopyElement::OptionalClass { .. }
                    | crate::mir::MirArrayCopyElement::Array(_)
                    | crate::mir::MirArrayCopyElement::Shared(_)
                    | crate::mir::MirArrayCopyElement::OptionalShared(_),
                ..
            } => {
                require_executable_array_place(program, definition, source)?;
            }
            MirArrayInstruction::ElementAssign {
                destination,
                operation: crate::mir::MirArrayAssignElement::Shared(_),
                ..
            } => {
                require_executable_element_place(program, definition, destination)?;
            }
            MirArrayInstruction::Adopt { destination, .. } => {
                require_executable_array_place(program, definition, destination)?;
            }
            MirArrayInstruction::Replace { destination, .. } => {
                require_executable_array_place(program, definition, destination)?;
            }
            MirArrayInstruction::Release { owner, .. } => {
                require_executable_array_place(program, definition, owner)?;
            }
            MirArrayInstruction::DestroyNext {
                owner,
                operation:
                    crate::mir::MirArrayDestroyElement::Trivial
                    | crate::mir::MirArrayDestroyElement::Class(_)
                    | crate::mir::MirArrayDestroyElement::OptionalClass(_)
                    | crate::mir::MirArrayDestroyElement::Array(_)
                    | crate::mir::MirArrayDestroyElement::Shared(_)
                    | crate::mir::MirArrayDestroyElement::OptionalShared(_),
                ..
            } => {
                require_executable_array_place(program, definition, owner)?;
            }
            MirArrayInstruction::AnchorBegin {
                owner,
                kind:
                    MirArrayAnchorKind::InlineOwner
                    | MirArrayAnchorKind::InlineBacking
                    | MirArrayAnchorKind::StableSharedOwner
                    | MirArrayAnchorKind::CopiedSharedOwner
                    | MirArrayAnchorKind::AdoptedSharedOwner
                    | MirArrayAnchorKind::SecuredOptionalSharedOwner,
                ..
            } => {
                require_executable_array_place(program, definition, owner)?;
            }
            MirArrayInstruction::AnchorEnd { .. } => {}
            MirArrayInstruction::AliasBind { source, .. } => {
                require_executable_element_place(program, definition, source)?;
            }
            MirArrayInstruction::Normalize {
                owner,
                kind: MirArrayPositionKind::Element | MirArrayPositionKind::SliceBound,
                ..
            } => {
                require_executable_array_place(program, definition, owner)?;
            }
            MirArrayInstruction::Offset { owner, .. } => {
                require_executable_array_place(program, definition, owner)?;
            }
            MirArrayInstruction::Boundary { owner, .. } => {
                require_executable_array_place(program, definition, owner)?;
            }
            MirArrayInstruction::SliceBoundsCheck { .. } => {}
            MirArrayInstruction::SliceLengthCheck { source, .. } => {
                require_executable_array_place(program, definition, source)?;
            }
            MirArrayInstruction::SliceCopy { source, .. } => {
                require_executable_array_place(program, definition, source)?;
            }
            MirArrayInstruction::SliceAssignNext {
                destination,
                source,
                ..
            } => {
                require_executable_array_place(program, definition, destination)?;
                require_executable_array_place(program, definition, source)?;
            }
            _ => {
                return Err(error(
                    Some(definition.callable()),
                    "verified array operation is outside the executable array profile",
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
            failure:
                MirArrayFailure::AllocationSize
                | MirArrayFailure::InvalidSliceBounds
                | MirArrayFailure::SliceLengthMismatch,
            ..
        }
        | MirTerminator::ArrayPositionCheck {
            kind:
                MirArrayPositionKind::Element
                | MirArrayPositionKind::SliceBound
                | MirArrayPositionKind::RangeOffset,
            ..
        }
        | MirTerminator::ArrayLoop { .. }
        | MirTerminator::Terminate {
            reason:
                MirTerminationReason::ArrayAllocationFailure
                | MirTerminationReason::ArrayIndexOutOfBounds
                | MirTerminationReason::ArrayInvalidSliceBounds
                | MirTerminationReason::ArraySliceLengthMismatch,
            ..
        } => true,
        _ => true,
    };
    if supported {
        Ok(())
    } else {
        Err(error(
            Some(definition.callable()),
            "verified array control flow is outside the executable array profile",
        ))
    }
}

fn require_executable_element_place(
    program: &MirProgram,
    definition: MirDefinitionRef<'_>,
    place: &MirPlace,
) -> Result<(), BackendError> {
    let Some(element_index) = place
        .projections
        .iter()
        .rposition(|projection| matches!(projection, MirPlaceProjection::ArrayElement { .. }))
    else {
        return Err(error(
            Some(definition.callable()),
            "only inline array element places are executable on x86-64",
        ));
    };
    let mut owner = MirPlace {
        base: place.base,
        projections: place.projections.clone(),
    };
    owner.projections.truncate(element_index);
    require_executable_array_place(program, definition, &owner)
}

fn has_array_element_projection(place: &MirPlace) -> bool {
    place
        .projections
        .iter()
        .any(|projection| matches!(projection, MirPlaceProjection::ArrayElement { .. }))
}

fn require_executable_array_place(
    program: &MirProgram,
    definition: MirDefinitionRef<'_>,
    place: &MirPlace,
) -> Result<(), BackendError> {
    let local_storage = place
        .base
        .local_storage()
        .and_then(|storage| definition.storage(storage));
    let root_ty = match place.base {
        MirPlaceBase::StaticField(field) => program.static_field(field).map(|field| field.ty),
        _ => local_storage.map(|storage| storage.ty),
    }
    .expect("verified array place has a declared root");
    let direct_static = place.projections.is_empty()
        && matches!(place.base, MirPlaceBase::StaticField(_))
        && matches!(root_ty, MirType::Array(_));
    let direct_local = place.projections.is_empty()
        && matches!(
            place.base,
            MirPlaceBase::Storage(_)
                | MirPlaceBase::AliasParameter(_)
                | MirPlaceBase::ArrayAlias(_)
        )
        && local_storage.is_some_and(|storage| {
            matches!(
                storage.kind,
                MirStorageKind::Local
                    | MirStorageKind::Parameter
                    | MirStorageKind::Return
                    | MirStorageKind::Argument
                    | MirStorageKind::ArrayProduced
                    | MirStorageKind::ArraySlice
                    | MirStorageKind::ArrayAnchor(_)
                    | MirStorageKind::ArrayAlias(_)
                    | MirStorageKind::AliasParameter(_)
            ) && matches!(storage.ty, MirType::Array(_))
        });
    let projected_owner = (!place.projections.is_empty()
        || matches!(place.base, MirPlaceBase::SharedPointee(_)))
        && projected_type(program, root_ty, place) // verified projections
            .is_some_and(|ty| matches!(ty, MirType::Array(_)));
    if direct_static || direct_local || projected_owner {
        Ok(())
    } else {
        Err(error(
            Some(definition.callable()),
            "array place is outside the executable inline/shared owning-boundary profile",
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
