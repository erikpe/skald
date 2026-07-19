//! Declaration collection, lexical name resolution, and stable symbol identity.
//!
//! M3 produces a separate resolved representation. Later phases consume typed
//! IDs and never choose declarations by comparing source names.

mod dump;
mod ir;
mod resolver;

pub use dump::dump_resolved;
pub use ir::*;
pub use resolver::{
    resolve, ResolveOutput, DUPLICATE_BINDING, DUPLICATE_FUNCTION, FUNCTION_USED_AS_VALUE,
    INVALID_CALL_TARGET, UNKNOWN_NAME,
};
