//! Explicit two-phase rehoming of selected callable-local MIR.
//!
//! The supported API owns immutable source snapshots, explicit selections and
//! boundary substitutions, and complete source-to-destination maps. Request
//! preparation, allocation/cloning, and exhaustive identity mapping remain
//! private implementation responsibilities.

mod execute;
mod mapper;
mod model;
mod prepare;

pub(crate) use model::{
    MirImportMap, MirImportMaps, MirImportRequest, MirImportResult, MirImportSource,
};

#[cfg(test)]
mod tests;
