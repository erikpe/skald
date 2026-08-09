//! Private shared-handle helpers used by generated lifecycle functions.

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
    let array_lifecycle_needs_helpers = program.array_types.iter().any(|array| {
        matches!(
            array.element,
            MirType::Shared(_) | MirType::OptionalShared(_)
        )
    });
    let static_lifecycle_needs_helpers =
        program.static_lifecycle.as_ref().is_some_and(|lifecycle| {
            lifecycle.shutdown().iter().any(|region| {
                matches!(
                    region.cleanup,
                    crate::mir::MirStaticValueCleanup::Shared(_)
                        | crate::mir::MirStaticValueCleanup::OptionalShared(_)
                )
            })
        });
    let mut helpers = Vec::new();
    if array_lifecycle_needs_helpers {
        helpers.push(lower_retain());
    }
    if array_lifecycle_needs_helpers || static_lifecycle_needs_helpers {
        helpers.push(lower_release(dispatch));
    }
    helpers
}

fn lower_retain() -> AssemblyFunction {
    let invalid = Label::new(".Lska_shared_handle_retain_invalid".to_owned());
    let overflow = Label::new(".Lska_shared_handle_retain_overflow".to_owned());
    let mut instructions = vec![Instruction::Move {
        source: Register::Rdi.into(),
        destination: Register::Rax.into(),
    }];
    emit_retain_loaded_handle(invalid.clone(), overflow.clone(), &mut instructions);
    instructions.extend([Instruction::Return, Instruction::Label(overflow)]);
    super::super::terminator::emit_ownership_overflow(&mut instructions);
    instructions.extend([
        Instruction::Label(invalid),
        // Array lifecycle helpers pass only verified live shared handles.
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
