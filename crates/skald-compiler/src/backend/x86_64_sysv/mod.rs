//! Linux x86-64 backend using the System V AMD64 ABI.
//!
//! The first implementation intentionally gives every MIR storage location
//! and value a stack home. Instruction selection uses only caller-saved
//! scratch registers, keeping register allocation an internal optimization
//! that can be replaced later.
//! Target legality, layout, ABI, and emission are documented in
//! `docs/compiler/BACKEND.md`.

mod abi;
mod array_legality;
mod dispatch;
mod emit;
mod frame;
mod layout;
mod legality;
mod literal_data;
mod lower;
mod machine;
// TRACE1 establishes the request API before TRACE2's instruction selection
// begins consuming locations in production lowering.
#[allow(dead_code)]
mod runtime_trace;
mod static_fields;
mod symbol;

use super::{BackendError, BackendInput};

pub fn emit_assembly(input: BackendInput<'_>) -> Result<String, BackendError> {
    let program = input.program();
    let (data_layout, dispatch) = legality::check(program)?;
    let mut assembly = lower::lower(program, &data_layout, &dispatch)?;
    assembly.runtime_trace = runtime_trace::Metadata::new(input).finish();
    Ok(emit::emit(&assembly))
}

#[cfg(test)]
mod tests;
