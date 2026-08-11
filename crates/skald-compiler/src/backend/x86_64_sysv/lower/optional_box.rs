//! Finalizers for allocations containing canonical optional wrappers.
//!
//! Each exact box identity receives its own function and descriptor. Cleanup
//! follows the canonical optional metadata recursively, so nested owners,
//! inline classes, and arrays retain their ordinary lifecycle semantics.

use crate::{backend::BackendError, mir::MirProgram};

use super::super::{dispatch::DispatchMetadata, layout::DataLayout, machine::AssemblyFunction};
use super::finalize;

pub(super) fn lower_finalizers(
    program: &MirProgram,
    data_layout: &DataLayout,
    dispatch: &DispatchMetadata,
) -> Result<Vec<AssemblyFunction>, BackendError> {
    program
        .optional_box_types
        .iter()
        .filter_map(|box_type| {
            box_type
                .exact_optional
                .map(|optional| (box_type.id, optional))
        })
        .map(|(target, optional)| {
            finalize::lower_optional_box(program, data_layout, dispatch, target, optional)
        })
        .collect()
}
