//! Closed runtime service contracts, independent of linker spellings and target ABI.

use super::{
    ComponentRole, Convention, PlanError, ReturnShape, RuntimeService, ScalarType, SignatureFact,
};
use crate::backend::effects::{Effect, Effects, MemoryRegion};

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn check_service(
    service: RuntimeService,
    signature: &SignatureFact,
) -> Result<(), PlanError> {
    if signature != &service_signature(service) {
        return Err(PlanError::InvalidSignature);
    }
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn service_signature(service: RuntimeService) -> SignatureFact {
    use ScalarType::*;
    let (inputs, returns): (&[ScalarType], ReturnShape) = match service {
        RuntimeService::Allocate => (&[U64], ReturnShape::Scalar(DataAddress)),
        RuntimeService::Free => (&[DataAddress], ReturnShape::Unit),
        RuntimeService::Panic => (&[DataAddress, U64], ReturnShape::Never),
        RuntimeService::IoStandardHandle => (&[U8], ReturnShape::Scalar(I64)),
        RuntimeService::IoOpen => (&[DataAddress, U64, U8], ReturnShape::Scalar(I64)),
        RuntimeService::IoRead | RuntimeService::IoWrite => {
            (&[I64, DataAddress, U64], ReturnShape::Scalar(I64))
        }
        RuntimeService::IoClose => (&[I64], ReturnShape::Scalar(I64)),
        RuntimeService::AbiMarker => (&[], ReturnShape::Unit),
    };
    SignatureFact {
        convention: Convention::Runtime,
        inputs: inputs
            .iter()
            .enumerate()
            .map(|(index, ty)| super::Component {
                ty: *ty,
                role: ComponentRole::RuntimeParameter(index),
            })
            .collect(),
        results: match returns {
            ReturnShape::Scalar(ty) => vec![super::Component {
                ty,
                role: ComponentRole::Result,
            }],
            _ => vec![],
        },
        returns,
    }
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn service_effects<O: Copy + Ord>(service: RuntimeService) -> Effects<O> {
    use Effect::*;
    use MemoryRegion::Unknown;
    match service {
        RuntimeService::Allocate => Effects::new([
            Call,
            Read(Unknown),
            Write(Unknown),
            Allocate,
            Report,
            HardTrap,
            TraceState,
        ]),
        RuntimeService::Free => Effects::new([Call, Read(Unknown), Write(Unknown), Free, HardTrap]),
        RuntimeService::Panic => Effects::new([Call, Read(Unknown), Report, TraceState, HardTrap]),
        RuntimeService::IoRead => Effects::new([Call, Write(Unknown), HardTrap]),
        RuntimeService::IoWrite | RuntimeService::IoOpen => {
            Effects::new([Call, Read(Unknown), HardTrap])
        }
        RuntimeService::IoStandardHandle | RuntimeService::IoClose => {
            Effects::new([Call, HardTrap])
        }
        RuntimeService::AbiMarker => Effects::new([Call]),
    }
}
