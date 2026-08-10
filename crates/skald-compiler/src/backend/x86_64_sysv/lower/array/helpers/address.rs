//! Address and displacement materialization shared by generated helpers.

use crate::backend::{
    x86_64_sysv::{
        lower::value,
        machine::{Instruction, Operand, Register},
    },
    BackendError,
};

use super::helper_error;

pub(super) fn materialize_helper_element_addresses(
    stride: usize,
    displacement: i32,
) -> Vec<Instruction> {
    let stride = u64::try_from(stride).expect("array stride fits u64");
    vec![
        Instruction::Move {
            source: Register::Rdx.into(),
            destination: Register::Rax.into(),
        },
        Instruction::MoveImmediate64 {
            bits: stride,
            destination: Register::R11,
        },
        Instruction::Multiply {
            source: Register::R11,
            destination: Register::Rax,
        },
        Instruction::Add {
            source: Register::Rax,
            destination: Register::Rdi,
        },
        Instruction::LoadEffectiveAddress {
            source: value::memory(Register::Rdi, displacement),
            destination: Register::Rdi,
        },
        Instruction::Move {
            source: Register::Rcx.into(),
            destination: Register::Rax.into(),
        },
        Instruction::MoveImmediate64 {
            bits: stride,
            destination: Register::R11,
        },
        Instruction::Multiply {
            source: Register::R11,
            destination: Register::Rax,
        },
        Instruction::Add {
            source: Register::Rax,
            destination: Register::Rsi,
        },
        Instruction::LoadEffectiveAddress {
            source: value::memory(Register::Rsi, displacement),
            destination: Register::Rsi,
        },
    ]
}

pub(super) fn materialize_destroy_element_address(
    stride: usize,
    displacement: i32,
) -> Vec<Instruction> {
    vec![
        Instruction::Move {
            source: Register::Rsi.into(),
            destination: Register::Rax.into(),
        },
        Instruction::MoveImmediate64 {
            bits: u64::try_from(stride).expect("array stride fits u64"),
            destination: Register::R11,
        },
        Instruction::Multiply {
            source: Register::R11,
            destination: Register::Rax,
        },
        Instruction::Add {
            source: Register::Rax,
            destination: Register::Rdi,
        },
        Instruction::LoadEffectiveAddress {
            source: value::memory(Register::Rdi, displacement),
            destination: Register::Rdi,
        },
    ]
}

pub(super) fn offset_operand(operand: Operand, offset: i32) -> Result<Operand, BackendError> {
    match operand {
        Operand::Memory { base, displacement } => Ok(Operand::Memory {
            base,
            displacement: displacement
                .checked_add(offset)
                .ok_or_else(|| helper_error("array helper displacement exceeds x86-64"))?,
        }),
        Operand::IndexedMemory {
            base,
            index,
            scale,
            displacement,
        } => Ok(Operand::IndexedMemory {
            base,
            index,
            scale,
            displacement: displacement
                .checked_add(offset)
                .ok_or_else(|| helper_error("array helper displacement exceeds x86-64"))?,
        }),
        Operand::Register(_) => Err(helper_error(
            "array helper cannot offset a register operand",
        )),
    }
}
