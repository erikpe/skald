//! Deterministic primitive array helpers specialized by canonical array ID.

use crate::{
    backend::{BackendError, Target},
    mir::{MirProgram, MirType},
};

use super::super::super::{
    layout::{DataLayout, ARRAY_LENGTH_OFFSET, ARRAY_OWNER_COUNT_OFFSET},
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
                lower_copier(array.id, array.element, data_layout),
                lower_clone(array.id, array.element, data_layout),
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

fn lower_clone(
    array: crate::identity::ArrayTypeId,
    element: MirType,
    data_layout: &DataLayout,
) -> Result<AssemblyFunction, BackendError> {
    let layout = data_layout.array(array).ok_or_else(|| {
        BackendError::new(
            Target::X86_64SysV,
            None,
            format!("array {array} has no clone layout"),
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
    let stem = format!(".Lska_array_{}_clone", array.index());
    let empty = Label::new(format!("{stem}_empty"));
    let header = Label::new(format!("{stem}_header"));
    let body = Label::new(format!("{stem}_body"));
    let complete = Label::new(format!("{stem}_complete"));
    let source_home = value::memory(Register::Rbp, -8);
    let length_home = value::memory(Register::Rbp, -16);
    let destination_home = value::memory(Register::Rbp, -24);
    let source_element = value::indexed_memory(Register::Rsi, Register::Rcx, scale, displacement);
    let destination_element =
        value::indexed_memory(Register::Rdi, Register::Rcx, scale, displacement);
    let mut instructions = vec![
        Instruction::Push(Register::Rbp),
        Instruction::Move {
            source: Register::Rsp.into(),
            destination: Register::Rbp.into(),
        },
        Instruction::ReserveStack(32),
        Instruction::Test(Register::Rdi),
        Instruction::JumpIfEqual(empty.clone()),
        Instruction::Move {
            source: Register::Rdi.into(),
            destination: source_home,
        },
        Instruction::Move {
            source: value::memory(Register::Rdi, ARRAY_LENGTH_OFFSET),
            destination: Register::Rax.into(),
        },
        Instruction::Move {
            source: Register::Rax.into(),
            destination: length_home,
        },
        Instruction::MoveImmediate64 {
            bits: u64::try_from(layout.stride()).expect("array stride fits u64"),
            destination: Register::R11,
        },
        Instruction::Multiply {
            source: Register::R11,
            destination: Register::Rax,
        },
        Instruction::MoveImmediate64 {
            bits: u64::try_from(layout.element_offset()).expect("array offset fits u64"),
            destination: Register::R11,
        },
        Instruction::Add {
            source: Register::R11,
            destination: Register::Rax,
        },
        Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdi.into(),
        },
        Instruction::Call("ska_rt_alloc".to_owned()),
        Instruction::Move {
            source: Register::Rax.into(),
            destination: destination_home,
        },
        Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdx.into(),
        },
        Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::Rax,
        },
        Instruction::Move {
            source: Register::Rax.into(),
            destination: value::memory(Register::Rdx, ARRAY_OWNER_COUNT_OFFSET),
        },
        Instruction::Move {
            source: length_home,
            destination: Register::Rax.into(),
        },
        Instruction::Move {
            source: Register::Rax.into(),
            destination: value::memory(Register::Rdx, ARRAY_LENGTH_OFFSET),
        },
        Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rcx,
        },
        Instruction::Label(header.clone()),
        Instruction::Move {
            source: length_home,
            destination: Register::R11.into(),
        },
        Instruction::Compare {
            source: Register::R11,
            destination: Register::Rcx,
        },
        Instruction::JumpIfBelow(body.clone()),
        Instruction::Jump(complete.clone()),
        Instruction::Label(body),
        Instruction::Move {
            source: source_home,
            destination: Register::Rsi.into(),
        },
        Instruction::Move {
            source: destination_home,
            destination: Register::Rdi.into(),
        },
    ];
    if matches!(element, MirType::U8 | MirType::Bool) {
        instructions.push(Instruction::LoadZeroExtendByte {
            source: source_element,
            destination: Register::Rax,
        });
        instructions.push(Instruction::MoveByte {
            source: ByteRegister::Al,
            destination: destination_element,
        });
    } else {
        instructions.push(Instruction::Move {
            source: source_element,
            destination: Register::Rax.into(),
        });
        instructions.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: destination_element,
        });
    }
    instructions.extend([
        Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        },
        Instruction::Add {
            source: Register::R11,
            destination: Register::Rcx,
        },
        Instruction::Jump(header),
        Instruction::Label(complete),
        Instruction::Move {
            source: destination_home,
            destination: Register::Rax.into(),
        },
        Instruction::Leave,
        Instruction::Return,
        Instruction::Label(empty),
        Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rax,
        },
        Instruction::Leave,
        Instruction::Return,
    ]);
    Ok(AssemblyFunction {
        symbol: symbol::array_clone(array),
        exported: false,
        instructions,
    })
}

fn lower_copier(
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
    let source = value::indexed_memory(Register::Rsi, Register::Rdx, scale, displacement);
    let destination = value::indexed_memory(Register::Rdi, Register::Rdx, scale, displacement);
    let mut instructions = Vec::new();
    if matches!(element, MirType::U8 | MirType::Bool) {
        instructions.push(Instruction::LoadZeroExtendByte {
            source,
            destination: Register::Rax,
        });
        instructions.push(Instruction::MoveByte {
            source: ByteRegister::Al,
            destination,
        });
    } else {
        value::load_rax(source, &mut instructions);
        value::store_rax(destination, &mut instructions);
    }
    instructions.push(Instruction::Return);
    Ok(AssemblyFunction {
        symbol: symbol::array_copy_element(array),
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
