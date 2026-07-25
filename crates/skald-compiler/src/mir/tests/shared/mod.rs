//! Shared-ownership MIR tests organized by semantic responsibility.

use super::*;
use crate::{
    backend::{emit_assembly, Target},
    passes::run_mir_pipeline,
};

mod anchors;
mod calls_and_results;
mod casts_and_views;
mod copy_allocation;
mod core_owners;
mod fields;

fn main_instructions(program: &MirProgram) -> &[MirInstruction] {
    &program
        .definitions
        .get(program.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .instructions
}

fn main_instructions_mut(program: &mut MirProgram) -> &mut Vec<MirInstruction> {
    &mut program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .instructions
}

fn has_error(program: &MirProgram, needle: &str) -> bool {
    verify_mir(program)
        .unwrap_err()
        .iter()
        .any(|error| error.message.contains(needle))
}
