//! Compiler-generated complete-object finalizers.
//!
//! A finalizer receives the complete payload address in `rdi`. It deliberately
//! knows nothing about the allocation header: last-owner lowering retains
//! header identity and frees it only after this function returns.

use crate::{
    backend::{BackendError, Target},
    identity::ClassId,
    mir::{MirDestructionStep, MirProgram, MirType},
};

use super::super::{
    layout::DataLayout,
    machine::{AssemblyFunction, Instruction, Operand, Register},
    symbol,
};

const COMPLETE_HOME: i32 = -8;
const FINALIZER_FRAME_SIZE: u32 = 16;

pub(super) fn lower_all(
    program: &MirProgram,
    data_layout: &DataLayout,
) -> Result<Vec<AssemblyFunction>, BackendError> {
    program
        .classes
        .iter()
        .map(|class| lower_class(program, data_layout, class.id))
        .collect()
}

fn lower_class(
    program: &MirProgram,
    data_layout: &DataLayout,
    class: ClassId,
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
    select_plan(program, data_layout, class, 0, &mut instructions)?;
    instructions.extend([Instruction::Leave, Instruction::Return]);
    Ok(AssemblyFunction {
        symbol: symbol::complete_finalizer(class),
        exported: false,
        instructions,
    })
}

fn select_plan(
    program: &MirProgram,
    data_layout: &DataLayout,
    class: ClassId,
    complete_offset: i32,
    output: &mut Vec<Instruction>,
) -> Result<(), BackendError> {
    let declaration = program
        .class(class)
        .ok_or_else(|| finalizer_error(format!("unknown finalizer class {class}")))?;
    for step in declaration.destruction.steps.iter().copied() {
        match step {
            MirDestructionStep::UserBody(destructor) => {
                load_complete_address(complete_offset, Register::Rdi, output);
                load_complete_address(complete_offset, Register::Rsi, output);
                output.push(Instruction::LoadSymbolAddress {
                    symbol: symbol::dispatch_table(class),
                    destination: Register::Rdx,
                });
                output.push(Instruction::Call(symbol::callable(
                    program,
                    destructor.into(),
                )));
            }
            MirDestructionStep::Field(field) => {
                let field_declaration = program
                    .field(field)
                    .ok_or_else(|| finalizer_error(format!("unknown finalizer field {field}")))?;
                let MirType::Class(field_class) = field_declaration.ty else {
                    return Err(finalizer_error(format!(
                        "finalizer for {class} contains non-inline field {field}"
                    )));
                };
                let field_offset = data_layout
                    .field(field)
                    .ok_or_else(|| finalizer_error(format!("field {field} has no target layout")))?
                    .offset;
                let field_offset = i32::try_from(field_offset)
                    .ok()
                    .and_then(|offset| complete_offset.checked_add(offset))
                    .ok_or_else(|| {
                        finalizer_error("finalizer field address exceeds target limits")
                    })?;
                select_plan(program, data_layout, field_class, field_offset, output)?;
            }
            MirDestructionStep::Base(base) => {
                let base_offset = data_layout
                    .class(class)
                    .and_then(|layout| layout.base())
                    .filter(|layout| layout.class == base)
                    .ok_or_else(|| {
                        finalizer_error(format!("class {class} has no direct base {base}"))
                    })?
                    .offset;
                let base_offset = i32::try_from(base_offset)
                    .ok()
                    .and_then(|offset| complete_offset.checked_add(offset))
                    .ok_or_else(|| {
                        finalizer_error("finalizer base address exceeds target limits")
                    })?;
                select_plan(program, data_layout, base, base_offset, output)?;
            }
        }
    }
    Ok(())
}

fn load_complete_address(offset: i32, destination: Register, output: &mut Vec<Instruction>) {
    output.push(Instruction::Move {
        source: memory(Register::Rbp, COMPLETE_HOME),
        destination: destination.into(),
    });
    if offset != 0 {
        output.push(Instruction::LoadEffectiveAddress {
            source: memory(destination, offset),
            destination,
        });
    }
}

const fn memory(base: Register, displacement: i32) -> Operand {
    Operand::Memory { base, displacement }
}

fn finalizer_error(message: impl Into<String>) -> BackendError {
    BackendError::new(Target::X86_64SysV, None, message)
}
