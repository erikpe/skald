//! Deterministic inline-array helpers specialized by canonical array ID.

use crate::{
    backend::{
        x86_64_sysv::{layout::DataLayout, machine::AssemblyFunction},
        BackendError, Target,
    },
    mir::MirProgram,
};

mod address;
mod copy;
mod destruction;
mod initialization;

use address::{
    materialize_destroy_element_address, materialize_helper_element_addresses, offset_operand,
};

const RUNTIME_FREE: &str = "ska_rt_free";

pub(super) fn lower_all(
    program: &MirProgram,
    data_layout: &DataLayout,
) -> Result<Vec<AssemblyFunction>, BackendError> {
    program
        .array_types
        .iter()
        .flat_map(|array| {
            [
                initialization::lower_initializer(array.id, array.element, data_layout),
                copy::lower_copier(program, array.id, array.element, data_layout),
                initialization::lower_clone(array.id, data_layout),
                destruction::lower_destroyer(program, array.id, array.element, data_layout),
                destruction::lower_release(array.id),
                destruction::lower_shared_finalizer(array.id, data_layout),
            ]
        })
        .collect()
}

fn helper_error(message: impl Into<String>) -> BackendError {
    BackendError::new(Target::X86_64SysV, None, message)
}
