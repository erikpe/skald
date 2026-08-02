//! Target-independent standard-I/O operations.
//!
//! These operations describe checked access to semantic byte-array places.
//! Runtime symbols, target pointers, descriptor layouts, and host error
//! mechanisms deliberately remain outside MIR.

use crate::{identity::ArrayTypeId, source::Span};

use super::{MirAliasAccess, MirPlace, StorageId, ValueId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirIoInstruction {
    pub result: ValueId,
    pub operation: MirIoOperation,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirIoOperation {
    StandardHandle {
        stream: ValueId,
    },
    Open {
        path: MirIoBuffer,
        mode: ValueId,
    },
    Read {
        handle: ValueId,
        destination: MirIoBuffer,
        offset: StorageId,
    },
    Write {
        handle: ValueId,
        source: MirIoBuffer,
        offset: StorageId,
    },
    Close {
        handle: ValueId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirIoBuffer {
    pub place: MirPlace,
    pub anchor: StorageId,
    pub array: ArrayTypeId,
    pub access: MirAliasAccess,
}
