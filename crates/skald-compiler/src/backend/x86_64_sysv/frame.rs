//! Deterministic fixed-stack-frame layout.

use crate::{
    backend::{BackendError, Target},
    mir::{MirFunction, StorageId, ValueId},
};

use super::abi;

const SLOT_SIZE: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FrameLayout {
    size: u32,
    storage_offsets: Vec<i32>,
    value_offsets: Vec<i32>,
}

impl FrameLayout {
    pub(super) fn plan(function: &MirFunction) -> Result<Self, BackendError> {
        let slot_count = function
            .storage
            .len()
            .checked_add(function.values.len())
            .ok_or_else(|| frame_too_large(function))?;
        let unaligned_size = slot_count
            .checked_mul(SLOT_SIZE)
            .ok_or_else(|| frame_too_large(function))?;
        let aligned_size = abi::align_up(unaligned_size, abi::STACK_ALIGNMENT)
            .ok_or_else(|| frame_too_large(function))?;
        let size = u32::try_from(aligned_size).map_err(|_| frame_too_large(function))?;

        // `%rbp`-relative displacements are signed 32-bit values in the
        // selected instruction encodings.
        if aligned_size > i32::MAX as usize {
            return Err(frame_too_large(function));
        }

        let storage_offsets = (0..function.storage.len())
            .map(slot_offset)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| frame_too_large(function))?;
        let value_offsets = (0..function.values.len())
            .map(|index| slot_offset(function.storage.len() + index))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| frame_too_large(function))?;

        Ok(Self {
            size,
            storage_offsets,
            value_offsets,
        })
    }

    pub(super) const fn size(&self) -> u32 {
        self.size
    }

    pub(super) fn storage(&self, id: StorageId) -> i32 {
        self.storage_offsets[id.index()]
    }

    pub(super) fn value(&self, id: ValueId) -> i32 {
        self.value_offsets[id.index()]
    }
}

fn slot_offset(index: usize) -> Option<i32> {
    let magnitude = index.checked_add(1)?.checked_mul(SLOT_SIZE)?;
    let magnitude = i32::try_from(magnitude).ok()?;
    magnitude.checked_neg()
}

fn frame_too_large(function: &MirFunction) -> BackendError {
    BackendError::new(
        Target::X86_64SysV,
        Some(function.id),
        "stack frame is too large for x86-64 frame-relative addressing",
    )
}
