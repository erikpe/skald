//! Optional lifecycle, storage, and checked-access lowering.

use crate::backend::BackendError;

use super::{super::machine::Operand, value};

mod access;
mod aggregate;
mod inline_class;
mod scalar;
mod shared_owner;

fn offset_operand(
    operand: Operand,
    offset: i32,
    callable: crate::identity::CallableId,
) -> Result<Operand, BackendError> {
    let displacement = match operand {
        Operand::Memory { base, displacement } => {
            return displacement
                .checked_add(offset)
                .map(|displacement| value::memory(base, displacement))
                .ok_or_else(|| {
                    BackendError::new(
                        crate::backend::Target::X86_64SysV,
                        Some(callable),
                        "optional payload displacement exceeds x86-64 limits",
                    )
                });
        }
        Operand::IndexedMemory {
            base,
            index,
            scale,
            displacement,
        } => displacement
            .checked_add(offset)
            .map(|displacement| value::indexed_memory(base, index, scale, displacement)),
        Operand::Register(_) => None,
    };
    displacement.ok_or_else(|| {
        BackendError::new(
            crate::backend::Target::X86_64SysV,
            Some(callable),
            "optional payload displacement exceeds x86-64 limits",
        )
    })
}
