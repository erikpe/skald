use super::*;
use crate::typeck::{capabilities::CopyCapabilities, function::MemberBodyKind};
use crate::{
    diagnostics::Diagnostics,
    hir::{
        HirAccess, HirCallArgument, HirCopyCapability, HirLocalInitializer,
        HirSelectedCopyOperation, HirSynthesizedFieldCopy,
    },
    identity::{
        BindingId, ClassId, CopyAssignmentId, FieldId, FunctionId, InitializerId, MethodId,
    },
    mir::{lower_hir, verify_mir, MirInstruction, MirPlaceProjection},
    typeck::function::{CallableChecker, MemberCheckContext, ReceiverContext},
};

mod construction;
mod dumps;
mod lifecycle_copy;
mod object_places;
mod receiver_access;
