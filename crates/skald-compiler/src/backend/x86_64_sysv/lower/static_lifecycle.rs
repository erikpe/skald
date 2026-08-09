//! Mechanical lowering of verified eager static activation and shutdown.

use crate::{
    backend::{BackendError, Target},
    identity::CallableId,
    mir::{MirArrayInstruction, MirProgram, MirStaticActivationWork, MirStaticValueCleanup},
};

use super::super::{
    layout::DataLayout,
    machine::{AssemblyFunction, Instruction, Label, Operand, Register},
    symbol,
};

pub(super) fn has_program_initializer(program: &MirProgram) -> bool {
    program
        .static_lifecycle
        .as_ref()
        .is_some_and(|coordinator| !coordinator.initializers().is_empty())
}

/// Lowers one private coordinator that invokes the already-lowered initializer
/// bodies in the verified activation order. Lifecycle transitions need no
/// target state: zero-default work is already represented by `.bss`, while
/// explicit bodies publish directly into their private value slots.
pub(super) fn lower_program_initializer(program: &MirProgram) -> Option<AssemblyFunction> {
    let coordinator = program.static_lifecycle.as_ref()?;
    if coordinator.initializers().is_empty() {
        return None;
    }

    let mut instructions = vec![
        Instruction::Push(Register::Rbp),
        Instruction::Move {
            source: Register::Rsp.into(),
            destination: Register::Rbp.into(),
        },
    ];
    instructions.extend(coordinator.activation().iter().filter_map(|region| {
        let MirStaticActivationWork::Explicit(initializer) = region.work else {
            return None;
        };
        Some(Instruction::Call(symbol::callable(
            program,
            CallableId::StaticInitializer(initializer),
        )))
    }));
    instructions.extend([Instruction::Leave, Instruction::Return]);

    Some(AssemblyFunction {
        symbol: symbol::program_initializer(),
        exported: false,
        instructions,
    })
}

pub(super) fn has_program_finalizer(program: &MirProgram) -> bool {
    program
        .static_lifecycle
        .as_ref()
        .is_some_and(|coordinator| !coordinator.shutdown().is_empty())
}

/// Lowers the verified destruction regions exactly as ordered. The begin and
/// finish transitions are semantic lifetime boundaries checked in MIR; they
/// require no target state because ordinary code cannot observe a destroying
/// or dead static field in a verified program.
pub(super) fn lower_program_finalizer(
    program: &MirProgram,
    data_layout: &DataLayout,
) -> Result<Option<AssemblyFunction>, BackendError> {
    let Some(coordinator) = program.static_lifecycle.as_ref() else {
        return Ok(None);
    };
    if coordinator.shutdown().is_empty() {
        return Ok(None);
    }

    let mut instructions = vec![
        Instruction::Push(Register::Rbp),
        Instruction::Move {
            source: Register::Rsp.into(),
            destination: Register::Rbp.into(),
        },
    ];
    for region in coordinator.shutdown() {
        lower_cleanup(
            program,
            data_layout,
            region.field,
            &region.cleanup,
            &mut instructions,
        )?;
    }
    instructions.extend([Instruction::Leave, Instruction::Return]);

    Ok(Some(AssemblyFunction {
        symbol: symbol::program_finalizer(),
        exported: false,
        instructions,
    }))
}

fn lower_cleanup(
    program: &MirProgram,
    data_layout: &DataLayout,
    field: crate::identity::StaticFieldId,
    cleanup: &MirStaticValueCleanup,
    output: &mut Vec<Instruction>,
) -> Result<(), BackendError> {
    match cleanup {
        MirStaticValueCleanup::None => {}
        MirStaticValueCleanup::CompleteObject(cleanup) => {
            load_static_address(program, field, Register::Rdi, output);
            output.push(Instruction::Call(symbol::complete_finalizer(
                program,
                cleanup.target,
            )));
        }
        MirStaticValueCleanup::OptionalClass(cleanup) => {
            let layout = data_layout.optional_class(cleanup.class)?;
            let state_offset = displacement(layout.state_offset(), "optional state")?;
            let payload_offset = displacement(layout.payload_offset(), "optional payload")?;
            let complete = Label::new(format!(
                ".Lska.static.s{}_{}_finalize_optional_complete",
                field.class().index(),
                field.index()
            ));

            load_static_address(program, field, Register::R11, output);
            output.push(Instruction::Move {
                source: memory(Register::R11, state_offset),
                destination: Register::Rax.into(),
            });
            output.push(Instruction::Test(Register::Rax));
            output.push(Instruction::JumpIfEqual(complete.clone()));
            load_static_address(program, field, Register::R11, output);
            output.push(Instruction::LoadEffectiveAddress {
                source: memory(Register::R11, payload_offset),
                destination: Register::Rdi,
            });
            output.push(Instruction::Call(symbol::complete_finalizer(
                program,
                cleanup.class,
            )));
            load_static_address(program, field, Register::R11, output);
            output.push(Instruction::MoveImmediate64 {
                bits: 0,
                destination: Register::Rax,
            });
            output.push(Instruction::Move {
                source: Register::Rax.into(),
                destination: memory(Register::R11, state_offset),
            });
            output.push(Instruction::Label(complete));
        }
        MirStaticValueCleanup::Shared(_) => {
            load_static_word(program, field, output);
            output.push(Instruction::Call(symbol::shared_handle_release()));
        }
        MirStaticValueCleanup::OptionalShared(_) => {
            let complete = Label::new(format!(
                ".Lska.static.s{}_{}_finalize_optional_shared_complete",
                field.class().index(),
                field.index()
            ));
            load_static_word(program, field, output);
            output.push(Instruction::Test(Register::Rdi));
            output.push(Instruction::JumpIfEqual(complete.clone()));
            output.push(Instruction::Call(symbol::shared_handle_release()));
            output.push(Instruction::Label(complete));
        }
        MirStaticValueCleanup::Array(MirArrayInstruction::Release {
            owner: _, array, ..
        }) => {
            load_static_word(program, field, output);
            output.push(Instruction::Call(symbol::array_release(*array)));
        }
        MirStaticValueCleanup::Array(_) => {
            unreachable!("verified static array cleanup is always a release")
        }
    }
    Ok(())
}

fn load_static_address(
    program: &MirProgram,
    field: crate::identity::StaticFieldId,
    destination: Register,
    output: &mut Vec<Instruction>,
) {
    output.push(Instruction::LoadSymbolAddress {
        symbol: symbol::static_field(program, field),
        destination,
    });
}

fn load_static_word(
    program: &MirProgram,
    field: crate::identity::StaticFieldId,
    output: &mut Vec<Instruction>,
) {
    output.push(Instruction::LoadSymbolAddress {
        symbol: symbol::static_field(program, field),
        destination: Register::R11,
    });
    output.push(Instruction::Move {
        source: memory(Register::R11, 0),
        destination: Register::Rdi.into(),
    });
}

fn displacement(offset: usize, description: &str) -> Result<i32, BackendError> {
    i32::try_from(offset).map_err(|_| {
        BackendError::new(
            Target::X86_64SysV,
            None,
            format!("static {description} offset exceeds target limits"),
        )
    })
}

const fn memory(base: Register, displacement: i32) -> Operand {
    Operand::Memory { base, displacement }
}
