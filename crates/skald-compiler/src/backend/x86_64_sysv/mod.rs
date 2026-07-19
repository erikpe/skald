//! Linux x86-64 backend using the System V AMD64 ABI.
//!
//! The first implementation intentionally gives every MIR storage location
//! and value a stack home. Instruction selection uses only caller-saved
//! scratch registers, keeping register allocation an internal optimization
//! that can be replaced later.

mod abi;
mod emit;
mod frame;
mod legality;
mod lower;
mod machine;

use crate::mir::MirProgram;

use super::BackendError;

pub fn emit_assembly(program: &MirProgram) -> Result<String, BackendError> {
    legality::check(program)?;
    let assembly = lower::lower(program)?;
    Ok(emit::emit(&assembly))
}

#[cfg(test)]
mod tests;
