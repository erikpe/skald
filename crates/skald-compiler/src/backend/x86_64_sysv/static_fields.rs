//! Deterministic writable storage planning for executable static fields.

use crate::{backend::BackendError, mir::MirProgram};

use super::{layout::DataLayout, machine::AssemblyStaticSlot, symbol};

pub(super) fn plan(
    program: &MirProgram,
    data_layout: &DataLayout,
) -> Result<Vec<AssemblyStaticSlot>, BackendError> {
    program
        .classes
        .iter()
        .flat_map(|class| &class.static_fields)
        .map(|field| {
            let layout = data_layout.ty(field.ty)?;
            Ok(AssemblyStaticSlot {
                symbol: symbol::static_field(program, field.id),
                size: layout.size(),
                alignment_power: u8::try_from(layout.alignment().trailing_zeros())
                    .expect("target alignment power must fit u8"),
            })
        })
        .collect()
}
