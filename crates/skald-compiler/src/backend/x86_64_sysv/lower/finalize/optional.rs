//! Recursive cleanup for canonical optional payloads stored at raw addresses.

use crate::{
    backend::{BackendError, Target},
    identity::{OptionalBoxTypeId, OptionalTypeId},
    mir::{MirOptionalStorage, MirProgram},
};

use super::super::super::{
    dispatch::DispatchMetadata,
    layout::DataLayout,
    machine::{AssemblyFunction, Instruction, Label, Register},
    symbol,
};
use super::super::{call, ownership::emit_release_loaded_handle};
use super::{
    finalizer_label_stem, load_complete_address, memory, release_labels, select_plan,
    FinalizerIdentity, COMPLETE_HOME, FINALIZER_FRAME_SIZE,
};

pub(super) fn lower_box(
    program: &MirProgram,
    data_layout: &DataLayout,
    dispatch: &DispatchMetadata,
    target: OptionalBoxTypeId,
    optional: OptionalTypeId,
) -> Result<AssemblyFunction, BackendError> {
    let mut instructions = vec![
        Instruction::Push(Register::Rbp),
        Instruction::Move {
            source: Register::Rsp.into(),
            destination: Register::Rbp.into(),
        },
        Instruction::ReserveStack(FINALIZER_FRAME_SIZE),
        Instruction::Move {
            source: Register::Rdi.into(),
            destination: memory(Register::Rbp, COMPLETE_HOME),
        },
    ];
    emit_cleanup_at(
        program,
        data_layout,
        dispatch,
        FinalizerIdentity::OptionalBox(target),
        optional,
        0,
        &mut instructions,
    )?;
    instructions.extend([Instruction::Leave, Instruction::Return]);
    Ok(AssemblyFunction {
        symbol: symbol::optional_box_finalizer(target),
        exported: false,
        instructions,
    })
}

pub(super) fn emit_cleanup_at(
    program: &MirProgram,
    data_layout: &DataLayout,
    dispatch: &DispatchMetadata,
    identity: FinalizerIdentity,
    optional: OptionalTypeId,
    offset: i32,
    output: &mut Vec<Instruction>,
) -> Result<(), BackendError> {
    let metadata = program
        .optional_type(optional)
        .ok_or_else(|| finalizer_error(format!("unknown optional {optional}")))?;
    match metadata.storage {
        MirOptionalStorage::Scalar => Ok(()),
        MirOptionalStorage::SharedOwner(_) => {
            let finished = cleanup_label(program, identity, "shared", output.len());
            load_state(offset, output);
            output.push(Instruction::JumpIfEqual(finished.clone()));
            let labels = release_labels(program, identity, None, output.len());
            emit_release_loaded_handle(
                labels.failure,
                labels.last,
                labels.complete.clone(),
                dispatch.finalizer_displacement(),
                None,
                call::TraceAttribution::InheritedSourceOperation,
                output,
            );
            output.push(Instruction::Label(labels.complete));
            output.push(Instruction::Label(finished));
            Ok(())
        }
        MirOptionalStorage::InlineClass(class) => {
            let finished = cleanup_label(program, identity, "class", output.len());
            load_state(offset, output);
            output.push(Instruction::JumpIfEqual(finished.clone()));
            let payload = payload_offset(data_layout, optional)?;
            select_plan(
                program,
                data_layout,
                dispatch,
                identity,
                class,
                checked_payload_address(offset, payload)?,
                output,
            )?;
            output.push(Instruction::Label(finished));
            Ok(())
        }
        MirOptionalStorage::Nested(nested) => {
            let finished = cleanup_label(program, identity, "optional", output.len());
            load_state(offset, output);
            output.push(Instruction::JumpIfEqual(finished.clone()));
            let payload = payload_offset(data_layout, optional)?;
            emit_cleanup_at(
                program,
                data_layout,
                dispatch,
                identity,
                nested,
                checked_payload_address(offset, payload)?,
                output,
            )?;
            output.push(Instruction::Label(finished));
            Ok(())
        }
        MirOptionalStorage::InlineArray(array) => {
            let finished = cleanup_label(program, identity, "array", output.len());
            load_state(offset, output);
            output.push(Instruction::JumpIfEqual(finished.clone()));
            let payload = payload_offset(data_layout, optional)?;
            load_complete_address(
                checked_payload_address(offset, payload)?,
                Register::R11,
                output,
            );
            output.push(Instruction::Move {
                source: memory(Register::R11, 0),
                destination: Register::Rdi.into(),
            });
            output.push(call::direct_instruction(
                symbol::array_release(array),
                call::TraceAttribution::InheritedSourceOperation,
            ));
            output.push(Instruction::Label(finished));
            Ok(())
        }
    }
}

fn load_state(offset: i32, output: &mut Vec<Instruction>) {
    load_complete_address(offset, Register::R11, output);
    output.push(Instruction::Move {
        source: memory(Register::R11, 0),
        destination: Register::Rax.into(),
    });
    output.push(Instruction::Test(Register::Rax));
}

fn cleanup_label(
    program: &MirProgram,
    identity: FinalizerIdentity,
    payload: &str,
    index: usize,
) -> Label {
    Label::new(format!(
        ".Lska.{}.finalize_nested_{}_{}",
        finalizer_label_stem(program, identity),
        payload,
        index
    ))
}

fn payload_offset(data_layout: &DataLayout, optional: OptionalTypeId) -> Result<i32, BackendError> {
    i32::try_from(data_layout.optional_type(optional)?.payload_offset())
        .map_err(|_| finalizer_error("optional payload offset exceeds target limits"))
}

fn checked_payload_address(offset: i32, payload: i32) -> Result<i32, BackendError> {
    offset
        .checked_add(payload)
        .ok_or_else(|| finalizer_error("optional payload address exceeds target limits"))
}

fn finalizer_error(message: impl Into<String>) -> BackendError {
    BackendError::new(Target::X86_64SysV, None, message)
}
