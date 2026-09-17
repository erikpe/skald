//! Immutable execution declarations for future low-level phase consumers.
//!
//! Checking supplied facts does not project or certify final MIR. Production
//! target planning remains with the current backend until the lowering migration.
//! Item-scoped non-test lint allowances cover APIs without native consumers yet;
//! remove them as production lowering adopts the corresponding declarations.

mod check;
mod facts;
mod identities;
mod services;
mod view;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use check::{CheckedPlan, PlanError};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use facts::{
    Abi, Architecture, ArtifactDeclaration, ArtifactPolicy, BodyDisposition, CallableDeclaration,
    Capabilities, Component, ComponentRole, Convention, DataLayout, DispatchSlot, Endianness,
    LayoutDisposition, LayoutFact, PlanFacts, ReturnShape, ScalarType, SignatureFact,
    TargetProfile,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use identities::{
    ArtifactCategory, ArtifactId, Coordinator, DataKey, HelperFamily, HelperKey, LayoutId,
    LirCallableId, RuntimeService, SignatureId, TargetThunkKey,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use view::{CallableBinding, DeclarationId, PlanView};

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(in crate::backend) mod test_fixtures;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use services::service_effects;
