//! Deterministic primitive array helpers specialized by canonical array ID.

use crate::{
    backend::{BackendError, Target},
    mir::{MirProgram, MirType},
};

use super::super::super::{
    layout::{DataLayout, ARRAY_OWNER_COUNT_OFFSET},
    machine::{AssemblyFunction, ByteRegister, Instruction, Label, Register},
    symbol,
};
use super::super::value;

const RUNTIME_FREE: &str = "ska_rt_free";

pub(super) fn lower_all(
    program: &MirProgram,
    data_layout: &DataLayout,
) -> Result<Vec<AssemblyFunction>, BackendError> {
    program
        .array_types
        .iter()
        .flat_map(|array| {
            [
                lower_initializer(array.id, array.element, data_layout),
                lower_release(array.id),
            ]
        })
        .collect()
}

fn lower_initializer(
    array: crate::identity::ArrayTypeId,
    element: MirType,
    data_layout: &DataLayout,
) -> Result<AssemblyFunction, BackendError> {
    let layout = data_layout.array(array).ok_or_else(|| {
        BackendError::new(
            Target::X86_64SysV,
            None,
            format!("array {array} has no helper layout"),
        )
    })?;
    let scale = u8::try_from(layout.stride()).map_err(|_| {
        BackendError::new(
            Target::X86_64SysV,
            None,
            format!("array {array} stride cannot be encoded"),
        )
    })?;
    let displacement = i32::try_from(layout.element_offset()).map_err(|_| {
        BackendError::new(
            Target::X86_64SysV,
            None,
            format!("array {array} element offset cannot be encoded"),
        )
    })?;
    let destination = value::indexed_memory(Register::Rdi, Register::Rsi, scale, displacement);
    let mut instructions = vec![Instruction::MoveImmediate64 {
        bits: 0,
        destination: Register::Rax,
    }];
    if matches!(element, MirType::U8 | MirType::Bool) {
        instructions.push(Instruction::MoveByte {
            source: ByteRegister::Al,
            destination,
        });
    } else {
        value::store_rax(destination, &mut instructions);
    }
    instructions.push(Instruction::Return);
    Ok(AssemblyFunction {
        symbol: symbol::array_initialize_element(array),
        exported: false,
        instructions,
    })
}

fn lower_release(array: crate::identity::ArrayTypeId) -> Result<AssemblyFunction, BackendError> {
    let complete = Label::new(format!(".Lska_array_{}_release_complete", array.index()));
    let instructions = vec![
        Instruction::Push(Register::Rbp),
        Instruction::Move {
            source: Register::Rsp.into(),
            destination: Register::Rbp.into(),
        },
        Instruction::Test(Register::Rdi),
        Instruction::JumpIfEqual(complete.clone()),
        Instruction::Move {
            source: value::memory(Register::Rdi, ARRAY_OWNER_COUNT_OFFSET),
            destination: Register::Rax.into(),
        },
        Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        },
        Instruction::Subtract {
            source: Register::R11,
            destination: Register::Rax,
        },
        Instruction::Move {
            source: Register::Rax.into(),
            destination: value::memory(Register::Rdi, ARRAY_OWNER_COUNT_OFFSET),
        },
        Instruction::Test(Register::Rax),
        Instruction::JumpIfNotZero(complete.clone()),
        Instruction::Call(RUNTIME_FREE.to_owned()),
        Instruction::Label(complete),
        Instruction::Leave,
        Instruction::Return,
    ];
    Ok(AssemblyFunction {
        symbol: symbol::array_release(array),
        exported: false,
        instructions,
    })
}
