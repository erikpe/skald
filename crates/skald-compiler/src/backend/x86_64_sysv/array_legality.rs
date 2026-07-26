//! Legality boundary for the first executable primitive inline-array profile.

use crate::{
    backend::{BackendError, Target},
    mir::{
        MirArrayAnchorKind, MirArrayDefaultElement, MirArrayFailure, MirArrayInstruction,
        MirArrayOwnership, MirDefinitionRef, MirInstruction, MirPlace, MirPlaceBase, MirProgram,
        MirRvalueKind, MirStorageKind, MirTerminationReason, MirTerminator, MirType,
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
    for class in program.classes.iter() {
        if class
            .fields
            .iter()
            .any(|field| matches!(field.ty, MirType::Array(_)))
        {
            return Err(error(
                None,
                "inline array fields are not yet supported by the x86-64 backend",
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
    if matches!(signature.return_type, MirType::Array(_))
        || signature
            .parameters
            .iter()
            .any(|parameter| matches!(parameter.ty, MirType::Array(_)))
    {
        return Err(error(
            Some(definition.callable()),
            "array parameters and results are not yet supported by the x86-64 backend",
        ));
    }

    for storage in definition.storage_entries() {
        if let MirType::Array(_) = storage.ty {
            let supported = matches!(
                storage.kind,
                MirStorageKind::Local
                    | MirStorageKind::ArrayBacking
                    | MirStorageKind::ArrayProduced
                    | MirStorageKind::ArrayAnchor(MirArrayAnchorKind::InlineOwner)
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
            check_instruction(definition, instruction)?;
        }
        check_terminator(definition, block.terminator.as_ref().unwrap())?;
    }
    Ok(())
}

fn check_instruction(
    definition: MirDefinitionRef<'_>,
    instruction: &MirInstruction,
) -> Result<(), BackendError> {
    match instruction {
        MirInstruction::Assign(assignment) => {
            if let MirRvalueKind::ArrayLength { source, .. } = &assignment.rvalue.kind {
                require_local_array_place(definition, source)?;
            }
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
            MirArrayInstruction::Adopt { destination, .. } => {
                require_local_array_place(definition, destination)?;
            }
            MirArrayInstruction::Release { owner, .. } => {
                require_local_array_place(definition, owner)?;
            }
            MirArrayInstruction::AnchorBegin {
                owner,
                kind: MirArrayAnchorKind::InlineOwner,
                ..
            } => {
                require_local_array_place(definition, owner)?;
            }
            MirArrayInstruction::AnchorEnd { .. } => {}
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
        | MirTerminator::ArrayLoop { .. }
        | MirTerminator::Terminate {
            reason: MirTerminationReason::ArrayAllocationFailure,
            ..
        } => true,
        MirTerminator::ArrayOperationCheck { .. }
        | MirTerminator::ArrayPositionCheck { .. }
        | MirTerminator::Terminate {
            reason:
                MirTerminationReason::ArrayIndexOutOfBounds
                | MirTerminationReason::ArrayInvalidSliceBounds
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

fn require_local_array_place(
    definition: MirDefinitionRef<'_>,
    place: &MirPlace,
) -> Result<(), BackendError> {
    let storage = definition
        .storage(place.base.storage())
        .expect("verified array place has declared storage");
    if place.projections.is_empty()
        && matches!(place.base, MirPlaceBase::Storage(_))
        && storage.kind == MirStorageKind::Local
        && matches!(storage.ty, MirType::Array(_))
    {
        Ok(())
    } else {
        Err(error(
            Some(definition.callable()),
            "only direct local inline-array places are executable on x86-64",
        ))
    }
}

fn error(
    callable: Option<crate::identity::CallableId>,
    message: impl Into<String>,
) -> BackendError {
    BackendError::new(Target::X86_64SysV, callable, message)
}
