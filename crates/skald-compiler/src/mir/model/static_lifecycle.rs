//! Static-lifecycle MIR schema facade.

mod coordinator;
mod phase_product;
mod plan;
mod proof;

pub use coordinator::{
    MirStaticActivationRegion, MirStaticActivationWork, MirStaticDestructionRegion,
    MirStaticLifecycleCoordinator, MirStaticSharedCleanup, MirStaticValueCleanup,
};
pub use phase_product::{MirPlannedLifecycle, MirProgramLifecycle};
pub use plan::{
    MirStaticFieldInitialization, MirStaticLifecycleDefinition, MirStaticLifecycleTransition,
    MirStaticLifecycleTransitionKind, StaticLifecyclePlan,
};
pub use proof::{
    MirStaticLifecycleProof, StaticAccessKind, StaticArrayLifecycleOperation,
    StaticClassLifecycleOperation, StaticEffectNode, StaticEffectPhase, StaticLifecycleAuthority,
    StaticLifecycleEffectFact, StaticLifecycleRootAuthority,
};
