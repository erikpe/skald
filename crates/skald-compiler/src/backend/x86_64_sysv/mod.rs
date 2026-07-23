//! Linux x86-64 backend using the System V AMD64 ABI.
//!
//! The first implementation intentionally gives every MIR storage location
//! and value a stack home. Instruction selection uses only caller-saved
//! scratch registers, keeping register allocation an internal optimization
//! that can be replaced later.
//! Target legality, layout, ABI, and emission are documented in
//! `docs/compiler/BACKEND.md`.

mod abi;
mod dispatch;
mod emit;
mod frame;
mod layout;
mod legality;
mod lower;
mod machine;
mod symbol;

use crate::mir::MirProgram;

use super::BackendError;

pub fn emit_assembly(program: &MirProgram) -> Result<String, BackendError> {
    let (data_layout, dispatch) = legality::check(program)?;
    let assembly = lower::lower(program, &data_layout, &dispatch)?;
    Ok(emit::emit(&assembly))
}

#[cfg(test)]
mod tests;
