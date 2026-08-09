//! Checked strong-count transitions for a handle already loaded in `rax`.

use crate::backend::x86_64_sysv::{
    layout::{SHARED_DYNAMIC_METADATA_OFFSET, SHARED_HEADER_SIZE},
    machine::{Instruction, Label, Operand, Register},
    runtime_trace::LocationReplacement,
};

use super::{
    super::{call, value},
    PRESERVED_HANDLE_STACK_SIZE, RUNTIME_FREE, STRONG_COUNT_OFFSET,
};

pub(in super::super) fn emit_retain_loaded_handle(
    invalid: Label,
    overflow: Label,
    output: &mut Vec<Instruction>,
) {
    let immortal = Label::new(format!("{}_immortal", overflow.name()));
    output.push(Instruction::Test(Register::Rax));
    output.push(Instruction::JumpIfEqual(invalid.clone()));
    output.push(Instruction::Move {
        source: value::memory(Register::Rax, STRONG_COUNT_OFFSET),
        destination: Register::Rcx.into(),
    });
    output.push(Instruction::Test(Register::Rcx));
    output.push(Instruction::JumpIfEqual(invalid));
    output.push(Instruction::MoveImmediate64 {
        bits: u64::MAX,
        destination: Register::R11,
    });
    output.push(Instruction::Compare {
        source: Register::R11,
        destination: Register::Rcx,
    });
    output.push(Instruction::JumpIfEqual(immortal.clone()));
    output.push(Instruction::MoveImmediate64 {
        bits: u64::MAX - 1,
        destination: Register::R11,
    });
    output.push(Instruction::Compare {
        source: Register::R11,
        destination: Register::Rcx,
    });
    output.push(Instruction::JumpIfEqual(overflow));
    output.push(Instruction::MoveImmediate64 {
        bits: 1,
        destination: Register::R11,
    });
    output.push(Instruction::Add {
        source: Register::R11,
        destination: Register::Rcx,
    });
    output.push(Instruction::Move {
        source: Register::Rcx.into(),
        destination: value::memory(Register::Rax, STRONG_COUNT_OFFSET),
    });
    output.push(Instruction::Label(immortal));
}

pub(in super::super) fn emit_release_loaded_handle(
    failure: Label,
    last: Label,
    complete: Label,
    finalizer_displacement: i32,
    location: Option<&LocationReplacement>,
    attribution: call::TraceAttribution,
    output: &mut Vec<Instruction>,
) {
    output.push(Instruction::Test(Register::Rax));
    output.push(Instruction::JumpIfEqual(failure.clone()));
    output.push(Instruction::Move {
        source: value::memory(Register::Rax, STRONG_COUNT_OFFSET),
        destination: Register::Rcx.into(),
    });
    output.push(Instruction::Test(Register::Rcx));
    output.push(Instruction::JumpIfEqual(failure.clone()));
    output.push(Instruction::MoveImmediate64 {
        bits: u64::MAX,
        destination: Register::R11,
    });
    output.push(Instruction::Compare {
        source: Register::R11,
        destination: Register::Rcx,
    });
    output.push(Instruction::JumpIfEqual(complete.clone()));
    output.push(Instruction::MoveImmediate64 {
        bits: 1,
        destination: Register::R11,
    });
    output.push(Instruction::Compare {
        source: Register::R11,
        destination: Register::Rcx,
    });
    output.push(Instruction::JumpIfEqual(last.clone()));
    output.push(Instruction::Subtract {
        source: Register::R11,
        destination: Register::Rcx,
    });
    output.push(Instruction::Move {
        source: Register::Rcx.into(),
        destination: value::memory(Register::Rax, STRONG_COUNT_OFFSET),
    });
    output.push(Instruction::Jump(complete.clone()));

    output.push(Instruction::Label(last));
    output.push(Instruction::MoveImmediate64 {
        bits: 0,
        destination: Register::R11,
    });
    output.push(Instruction::Move {
        source: Register::R11.into(),
        destination: value::memory(Register::Rax, STRONG_COUNT_OFFSET),
    });
    if let Some(location) = location {
        location.emit(output);
    }
    output.push(Instruction::Move {
        source: value::memory(Register::Rax, SHARED_DYNAMIC_METADATA_OFFSET),
        destination: Register::R11.into(),
    });
    output.push(Instruction::Test(Register::R11));
    output.push(Instruction::JumpIfEqual(failure.clone()));
    output.push(Instruction::Move {
        source: value::memory(Register::R11, finalizer_displacement),
        destination: Register::R11.into(),
    });
    output.push(Instruction::Test(Register::R11));
    output.push(Instruction::JumpIfEqual(failure.clone()));

    // Finalizers may recursively release arbitrary object graphs. Preserve the
    // canonical header itself instead of reloading a mutable owner place.
    output.push(Instruction::ReserveStack(PRESERVED_HANDLE_STACK_SIZE));
    output.push(Instruction::Move {
        source: Register::Rax.into(),
        destination: stack_handle(),
    });
    output.push(Instruction::LoadEffectiveAddress {
        source: value::memory(Register::Rax, SHARED_HEADER_SIZE as i32),
        destination: Register::Rdi,
    });
    output.push(call::indirect_instruction(Register::R11, attribution));
    output.push(Instruction::Move {
        source: stack_handle(),
        destination: Register::Rdi.into(),
    });
    output.push(Instruction::ReleaseStack(PRESERVED_HANDLE_STACK_SIZE));
    output.push(call::direct_instruction(
        RUNTIME_FREE,
        call::TraceAttribution::HardDefectOnly,
    ));
    output.push(Instruction::Jump(complete));

    output.push(Instruction::Label(failure));
    // Null, zero-count, missing-metadata, and missing-finalizer release states
    // contradict verified ownership or a published allocation's header.
    output.push(Instruction::Trap);
}

const fn stack_handle() -> Operand {
    Operand::Memory {
        base: Register::Rsp,
        displacement: 0,
    }
}
