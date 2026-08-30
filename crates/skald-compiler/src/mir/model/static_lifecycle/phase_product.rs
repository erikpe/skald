//! Planned and final lifecycle phase products.

use crate::identity::StaticFieldId;

use super::{
    plan::MirStaticLifecycleDefinitions, MirStaticLifecycleDefinition, MirStaticLifecycleProof,
    StaticLifecyclePlan,
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

/// Canonical plan and proof retained by the final structured coordinator.
///
/// Executable transitions belong only to their activation or destruction
/// regions and are deliberately absent from this phase product.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirProgramLifecycle {
    planned: MirPlannedLifecycle,
}

impl MirProgramLifecycle {
    pub(crate) const fn new(planned: MirPlannedLifecycle) -> Self {
        Self { planned }
    }

    pub fn definitions(&self) -> &[MirStaticLifecycleDefinition] {
        self.planned.definitions()
    }

    pub fn definition(&self, field: StaticFieldId) -> Option<&MirStaticLifecycleDefinition> {
        self.planned.definition(field)
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
    pub(crate) fn proof_mut_for_test(&mut self) -> &mut MirStaticLifecycleProof {
        self.planned.proof_mut_for_test()
    }
}
