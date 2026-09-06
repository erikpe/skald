//! Array verification facade split by invariant family.

mod anchor;
mod ownership;
mod projection;
mod storage;
mod structural;

use super::super::model::{BlockId, StorageId};

pub(super) struct IndexedArrayLoopShape {
    pub header: BlockId,
    pub backing: StorageId,
    pub prefix: StorageId,
    pub length: StorageId,
    pub binding: StorageId,
    pub body_target: BlockId,
    pub complete_target: BlockId,
}
