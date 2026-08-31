//! Exhaustive static-place extraction within the shared MIR traversal.
//!
//! Instruction, terminator, rvalue, argument, and projected-place coverage is
//! intentionally kept here rather than in a lifecycle consumer. Every record
//! is direct evidence; target closure and static-effect propagation remain
//! separate responsibilities.

mod control;
mod instruction;
mod place;

use crate::{
    identity::CallableId,
    mir::{
        MirAliasAccess, MirArgument, MirArrayInstruction, MirCallReceiver, MirClassOptionalSource,
        MirDefinitionRef, MirInstruction, MirIoOperation, MirObjectOrigin, MirObjectView,
        MirOptionalSharedSource, MirOptionalSource, MirPlace, MirPlaceBase, MirRvalue,
        MirRvalueKind, MirSharedCastSource, MirTerminator, StaticAccessKind,
    },
    source::Span,
};

use super::{
    extract::MirDependencyExtractor, MirDependencyExtractionError, MirDependencyRegion,
    MirStaticAccess, MirStaticAccessOrigin,
};
