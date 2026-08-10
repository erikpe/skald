//! Array element initialization and whole-array cloning helpers.

use crate::{
    backend::{
        x86_64_sysv::{
            layout::{DataLayout, ARRAY_LENGTH_OFFSET, ARRAY_OWNER_COUNT_OFFSET},
            machine::{AssemblyFunction, ByteRegister, Instruction, Label, Register},
            symbol,
        },
        BackendError, Target,
    },
    mir::MirType,
};

use super::materialize_destroy_element_address;
use crate::backend::x86_64_sysv::lower::{call, value};

pub(super) fn lower_initializer(
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
    let displacement = i32::try_from(layout.element_offset()).map_err(|_| {
        BackendError::new(
            Target::X86_64SysV,
            None,
            format!("array {array} element offset cannot be encoded"),
        )
    })?;
    let (destination, mut address_setup) = if matches!(layout.stride(), 1 | 2 | 4 | 8) {
        (
            value::indexed_memory(
                Register::Rdi,
                Register::Rsi,
                u8::try_from(layout.stride()).expect("encodable array stride"),
                displacement,
            ),
            Vec::new(),
        )
    } else {
        (
            value::memory(Register::Rdi, 0),
            materialize_destroy_element_address(layout.stride(), displacement),
        )
    };
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
    address_setup.extend(instructions);
    Ok(AssemblyFunction {
        symbol: symbol::array_initialize_element(array),
        exported: false,
        instructions: address_setup,
    })
}

pub(super) fn lower_clone(
    array: crate::identity::ArrayTypeId,
    data_layout: &DataLayout,
) -> Result<AssemblyFunction, BackendError> {
    let layout = data_layout.array(array).ok_or_else(|| {
        BackendError::new(
            Target::X86_64SysV,
            None,
            format!("array {array} has no clone layout"),
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
    let index_home = value::memory(Register::Rbp, -32);
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
        call::direct_instruction(
            "ska_rt_alloc",
            call::TraceAttribution::InheritedSourceOperation,
        ),
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
        Instruction::Move {
            source: Register::Rcx.into(),
            destination: Register::Rdx.into(),
        },
        Instruction::Move {
            source: Register::Rcx.into(),
            destination: index_home,
        },
        call::direct_instruction(
            symbol::array_copy_element(array),
            call::TraceAttribution::InheritedSourceOperation,
        ),
        Instruction::Move {
            source: index_home,
            destination: Register::Rcx.into(),
        },
    ];
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
