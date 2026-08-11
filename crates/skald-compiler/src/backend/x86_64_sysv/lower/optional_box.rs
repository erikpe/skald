//! Finalizers for allocations containing canonical optional wrappers.
//!
//! Primitive wrappers own no nested resources, but each exact box identity
//! still receives its own function and descriptor. Later lifecycle-bearing
//! targets can extend this module without changing the shared release path.

use crate::mir::MirProgram;

use super::super::{
    machine::{AssemblyFunction, Instruction},
    symbol,
};

pub(super) fn lower_primitive_finalizers(program: &MirProgram) -> Vec<AssemblyFunction> {
    program
        .optional_box_types
        .iter()
        .filter(|box_type| {
            box_type
                .exact_optional
                .and_then(|optional| program.optional_type(optional))
                .and_then(crate::mir::MirOptionalType::primitive)
                .is_some()
        })
        .map(|box_type| AssemblyFunction {
            symbol: symbol::optional_box_finalizer(box_type.id),
            exported: false,
            instructions: vec![Instruction::Return],
        })
        .collect()
}
