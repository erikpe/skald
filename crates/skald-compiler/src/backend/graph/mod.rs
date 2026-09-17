//! Callable- and stage-owned dense storage shared by low-level graph owners.
//! Item-scoped non-test lint allowances expire as native graph consumers land.

mod arena;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use arena::{
    LoweredBlockId, LoweredObjectId, LoweredValueId, OwnedArena, SelectedBlockId, SelectedObjectId,
    SelectedValueId,
};
