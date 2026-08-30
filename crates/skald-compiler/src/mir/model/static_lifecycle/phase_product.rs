//! Final-MIR lifecycle phase product.

use super::{
    MirStaticLifecycleDefinition, MirStaticLifecycleProof, MirStaticLifecycleTransition,
    StaticLifecyclePlan,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirProgramLifecycle {
    definitions: Vec<MirStaticLifecycleDefinition>,
    activation: Vec<MirStaticLifecycleTransition>,
    shutdown: Vec<MirStaticLifecycleTransition>,
    plan: StaticLifecyclePlan,
    proof: MirStaticLifecycleProof,
}

impl MirProgramLifecycle {
    pub(crate) fn new(
        definitions: Vec<MirStaticLifecycleDefinition>,
        activation: Vec<MirStaticLifecycleTransition>,
        shutdown: Vec<MirStaticLifecycleTransition>,
        plan: StaticLifecyclePlan,
        proof: MirStaticLifecycleProof,
    ) -> Self {
        Self {
            definitions,
            activation,
            shutdown,
            plan,
            proof,
        }
    }

    pub fn definitions(&self) -> &[MirStaticLifecycleDefinition] {
        &self.definitions
    }

    pub fn activation(&self) -> &[MirStaticLifecycleTransition] {
        &self.activation
    }

    pub fn shutdown(&self) -> &[MirStaticLifecycleTransition] {
        &self.shutdown
    }

    pub fn plan(&self) -> &StaticLifecyclePlan {
        &self.plan
    }

    pub fn proof(&self) -> &MirStaticLifecycleProof {
        &self.proof
    }

    #[cfg(test)]
    pub(crate) fn plan_mut_for_test(&mut self) -> &mut StaticLifecyclePlan {
        &mut self.plan
    }

    #[cfg(test)]
    pub(crate) fn definitions_mut_for_test(&mut self) -> &mut Vec<MirStaticLifecycleDefinition> {
        &mut self.definitions
    }

    #[cfg(test)]
    pub(crate) fn activation_mut_for_test(&mut self) -> &mut Vec<MirStaticLifecycleTransition> {
        &mut self.activation
    }

    #[cfg(test)]
    pub(crate) fn shutdown_mut_for_test(&mut self) -> &mut Vec<MirStaticLifecycleTransition> {
        &mut self.shutdown
    }

    #[cfg(test)]
    pub(crate) fn proof_mut_for_test(&mut self) -> &mut MirStaticLifecycleProof {
        &mut self.proof
    }
}
