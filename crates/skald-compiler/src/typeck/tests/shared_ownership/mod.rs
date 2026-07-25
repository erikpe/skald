//! Shared-ownership type-check tests organized by semantic responsibility.

use super::*;
use crate::{
    hir::{
        dump_hir, HirCallArgument, HirLocalInitializer, HirOwnerTransfer, HirReturnValue,
        HirSharedFieldWriteKind, HirSharedProducer, HirSharedSource, HirSharedTarget, HirStatement,
        Type,
    },
    identity::{ClassId, FunctionId, InitializerId, InterfaceId},
    mir::{dump_mir, lower_hir, verify_mir},
    typeck::INVALID_SHARED_CONVERSION,
};

mod anchors;
mod calls_and_results;
mod casts_and_views;
mod copy_allocation;
mod core_owners;
mod fields;

fn assert_diagnostics(diagnostics: &crate::diagnostics::Diagnostics, expected: &[&'static str]) {
    let actual: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(actual, expected);
}
