//! Array verification facade split by invariant family.

mod anchor;
mod ownership;
mod projection;
mod storage;
mod structural;

use super::super::model::{BlockId, MirType, StorageId};

pub(super) fn indexed_element_is_executable(element: MirType) -> bool {
    element.is_scalar_value() || indexed_element_requires_advance(element)
}

pub(super) fn indexed_element_requires_advance(element: MirType) -> bool {
    matches!(
        element,
        MirType::Class(_) | MirType::Optional(_) | MirType::Array(_)
    )
}

pub(super) struct IndexedArrayLoopShape {
    pub header: BlockId,
    pub backing: StorageId,
    pub prefix: StorageId,
    pub length: StorageId,
    pub binding: StorageId,
    pub body_target: BlockId,
    pub complete_target: BlockId,
}
