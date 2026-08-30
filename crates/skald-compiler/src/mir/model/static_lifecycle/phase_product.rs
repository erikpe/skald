//! Planned and final lifecycle phase products.

use crate::identity::StaticFieldId;

use super::{
    plan::MirStaticLifecycleDefinitions, MirStaticLifecycleDefinition, MirStaticLifecycleProof,
    MirStaticLifecycleTransition, StaticLifecyclePlan,
};

/// Canonical planned lifecycle data shared unchanged with final MIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirPlannedLifecycle {
    definitions: MirStaticLifecycleDefinitions,
    plan: StaticLifecyclePlan,
    proof: MirStaticLifecycleProof,
}

impl MirPlannedLifecycle {
    pub(crate) fn new(
        definitions: Vec<MirStaticLifecycleDefinition>,
        plan: StaticLifecyclePlan,
        proof: MirStaticLifecycleProof,
    ) -> Self {
        Self {
            definitions: MirStaticLifecycleDefinitions::new(definitions),
            plan,
            proof,
        }
    }

    pub fn definitions(&self) -> &[MirStaticLifecycleDefinition] {
        self.definitions.entries()
    }

    pub fn definition(&self, field: StaticFieldId) -> Option<&MirStaticLifecycleDefinition> {
        self.definitions.get(field)
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
        self.definitions.entries_mut_for_test()
    }

    #[cfg(test)]
    pub(crate) fn proof_mut_for_test(&mut self) -> &mut MirStaticLifecycleProof {
        &mut self.proof
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirProgramLifecycle {
    planned: MirPlannedLifecycle,
    activation: Vec<MirStaticLifecycleTransition>,
    shutdown: Vec<MirStaticLifecycleTransition>,
}

impl MirProgramLifecycle {
    pub(crate) fn new(
        planned: MirPlannedLifecycle,
        activation: Vec<MirStaticLifecycleTransition>,
        shutdown: Vec<MirStaticLifecycleTransition>,
    ) -> Self {
        Self {
            planned,
            activation,
            shutdown,
        }
    }

    pub fn definitions(&self) -> &[MirStaticLifecycleDefinition] {
        self.planned.definitions()
    }

    pub fn definition(&self, field: StaticFieldId) -> Option<&MirStaticLifecycleDefinition> {
        self.planned.definition(field)
    }

    pub fn activation(&self) -> &[MirStaticLifecycleTransition] {
        &self.activation
    }

    pub fn shutdown(&self) -> &[MirStaticLifecycleTransition] {
        &self.shutdown
    }

    pub fn plan(&self) -> &StaticLifecyclePlan {
        self.planned.plan()
    }

    pub fn proof(&self) -> &MirStaticLifecycleProof {
        self.planned.proof()
    }

    #[cfg(test)]
    pub(crate) fn plan_mut_for_test(&mut self) -> &mut StaticLifecyclePlan {
        self.planned.plan_mut_for_test()
    }

    #[cfg(test)]
    pub(crate) fn definitions_mut_for_test(&mut self) -> &mut Vec<MirStaticLifecycleDefinition> {
        self.planned.definitions_mut_for_test()
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
        self.planned.proof_mut_for_test()
    }
}
