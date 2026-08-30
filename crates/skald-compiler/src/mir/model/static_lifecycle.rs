//! Static-lifecycle MIR schema facade.

mod coordinator;
mod phase_product;
mod plan;
mod proof;

// These compatibility names preserve the public static-lifecycle certificate
// API while the identity itself is owned by the neutral MIR execution model.
pub use super::execution::{
    MirArrayLifecycleOperation as StaticArrayLifecycleOperation,
    MirClassLifecycleOperation as StaticClassLifecycleOperation,
    MirExecutionNode as StaticEffectNode,
};

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
    MirStaticLifecycleProof, StaticAccessKind, StaticEffectPhase, StaticLifecycleAuthority,
    StaticLifecycleEffectFact, StaticLifecycleRootAuthority,
};
