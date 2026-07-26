//! Private shared-handle helpers used by generated array lifecycle functions.

use crate::{
    backend::x86_64_sysv::{
        dispatch::DispatchMetadata,
        machine::{AssemblyFunction, Instruction, Label, Register},
        symbol,
    },
    mir::{MirProgram, MirType},
};

use super::{emit_release_loaded_handle, emit_retain_loaded_handle};

pub(super) fn lower_all(
    program: &MirProgram,
    dispatch: &DispatchMetadata,
) -> Vec<AssemblyFunction> {
    if !program.array_types.iter().any(|array| {
        matches!(
            array.element,
            MirType::Shared(_) | MirType::OptionalShared(_)
        )
    }) {
        return Vec::new();
    }
    vec![lower_retain(), lower_release(dispatch)]
}

fn lower_retain() -> AssemblyFunction {
    let failure = Label::new(".Lska_shared_handle_retain_invalid".to_owned());
    let mut instructions = vec![Instruction::Move {
        source: Register::Rdi.into(),
        destination: Register::Rax.into(),
    }];
    emit_retain_loaded_handle(failure.clone(), &mut instructions);
    instructions.extend([
        Instruction::Return,
        Instruction::Label(failure),
        Instruction::Trap,
    ]);
    AssemblyFunction {
        symbol: symbol::shared_handle_retain(),
        exported: false,
        instructions,
    }
}

fn lower_release(dispatch: &DispatchMetadata) -> AssemblyFunction {
    let failure = Label::new(".Lska_shared_handle_release_invalid".to_owned());
    let last = Label::new(".Lska_shared_handle_release_last".to_owned());
    let complete = Label::new(".Lska_shared_handle_release_complete".to_owned());
    let mut instructions = vec![
        Instruction::Push(Register::Rbp),
        Instruction::Move {
            source: Register::Rsp.into(),
            destination: Register::Rbp.into(),
        },
        Instruction::Move {
            source: Register::Rdi.into(),
            destination: Register::Rax.into(),
        },
    ];
    emit_release_loaded_handle(
        failure,
        last,
        complete.clone(),
        dispatch.finalizer_displacement(),
        &mut instructions,
    );
    instructions.extend([
        Instruction::Label(complete),
        Instruction::Leave,
        Instruction::Return,
    ]);
    AssemblyFunction {
        symbol: symbol::shared_handle_release(),
        exported: false,
        instructions,
    }
}
