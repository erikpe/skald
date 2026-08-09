//! Mechanical lowering of verified eager static activation.

use crate::{
    identity::CallableId,
    mir::{MirProgram, MirStaticActivationWork},
};

use super::super::{
    machine::{AssemblyFunction, Instruction, Register},
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
