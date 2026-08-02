//! Target-independent typed standard-I/O operations.

use super::{HirArrayAliasArgument, HirExpression};

/// A compiler-known I/O operation after source calls have been fully checked.
///
/// The operation carries semantic inputs only: no runtime symbol, target
/// pointer, descriptor layout, or array representation is present in HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirIoOperation {
    StandardHandle {
        stream: HirExpression,
    },
    Open {
        path: HirArrayAliasArgument,
        mode: HirExpression,
    },
    Read {
        handle: HirExpression,
        destination: HirArrayAliasArgument,
        offset: HirExpression,
    },
    Write {
        handle: HirExpression,
        source: HirArrayAliasArgument,
        offset: HirExpression,
    },
    Close {
        handle: HirExpression,
    },
}
