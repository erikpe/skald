use super::*;
use crate::typeck::{capabilities::CopyCapabilities, function::MemberBodyKind};
use crate::{
    diagnostics::Diagnostics,
    hir::{
        HirAccess, HirCallArgument, HirCopyCapability, HirLocalInitializer,
        HirSelectedCopyOperation, HirSynthesizedFieldCopy,
    },
    identity::{
        BindingId, ClassId, CopyAssignmentId, CopyConstructorId, FieldId, FunctionId,
        InitializerId, MethodId,
    },
    mir::{lower_hir, verify_mir, MirInstruction, MirPlaceProjection},
    object_path::ObjectProjection,
    typeck::function::{CallableChecker, MemberCheckContext, ReceiverContext},
};

mod construction;
mod dumps;
mod explicit_copy;
mod lifecycle_copy;
mod lifecycle_inheritance;
mod object_places;
mod private_initializers;
mod receiver_access;
mod static_inheritance;
mod virtual_methods;
