use super::identity::MirPassIdentity;
use super::stage::MirPassStage;
use crate::passes::pipeline::execution::{MirFinalPassTransform, MirProofPassTransform};

/// Stable selection and inspection metadata for one pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirPassDescriptor {
    identity: MirPassIdentity,
    stage: MirPassStage,
    name: &'static str,
    description: &'static str,
}

impl MirPassDescriptor {
    pub(in crate::passes::pipeline) const fn new(
        identity: MirPassIdentity,
        stage: MirPassStage,
        name: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            identity,
            stage,
            name,
            description,
        }
    }

    pub const fn identity(self) -> MirPassIdentity {
        self.identity
    }

    pub const fn stage(self) -> MirPassStage {
        self.stage
    }

    pub const fn name(self) -> &'static str {
        self.name
    }

    pub const fn description(self) -> &'static str {
        self.description
    }
}

/// Identity and transformation entry point declared by a pass implementation.
#[derive(Clone, Copy, Debug)]
pub(in crate::passes::pipeline) enum MirPassImplementation {
    ProofRich {
        identity: MirPassIdentity,
        transform: MirProofPassTransform,
    },
    Final {
        identity: MirPassIdentity,
        transform: MirFinalPassTransform,
    },
}

impl MirPassImplementation {
    pub(in crate::passes::pipeline) const fn proof_rich(
        identity: MirPassIdentity,
        transform: MirProofPassTransform,
    ) -> Self {
        Self::ProofRich {
            identity,
            transform,
        }
    }

    pub(in crate::passes::pipeline) const fn final_stage(
        identity: MirPassIdentity,
        transform: MirFinalPassTransform,
    ) -> Self {
        Self::Final {
            identity,
            transform,
        }
    }

    pub(in crate::passes::pipeline) const fn identity(self) -> MirPassIdentity {
        match self {
            Self::ProofRich { identity, .. } | Self::Final { identity, .. } => identity,
        }
    }

    pub(in crate::passes::pipeline) const fn stage(self) -> MirPassStage {
        match self {
            Self::ProofRich { .. } => MirPassStage::ProofRich,
            Self::Final { .. } => MirPassStage::Final,
        }
    }

    pub(in crate::passes::pipeline) const fn proof_transform(
        self,
    ) -> Option<MirProofPassTransform> {
        match self {
            Self::ProofRich { transform, .. } => Some(transform),
            Self::Final { .. } => None,
        }
    }

    pub(in crate::passes::pipeline) const fn final_transform(
        self,
    ) -> Option<MirFinalPassTransform> {
        match self {
            Self::ProofRich { .. } => None,
            Self::Final { transform, .. } => Some(transform),
        }
    }
}

impl PartialEq for MirPassImplementation {
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity() && self.stage() == other.stage()
    }
}

impl Eq for MirPassImplementation {}

/// One immutable compiler-owned registry entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline) struct MirPassRegistration {
    descriptor: MirPassDescriptor,
    implementation: MirPassImplementation,
}

impl MirPassRegistration {
    pub(in crate::passes::pipeline) const fn new(
        descriptor: MirPassDescriptor,
        implementation: MirPassImplementation,
    ) -> Self {
        Self {
            descriptor,
            implementation,
        }
    }

    pub(in crate::passes::pipeline) const fn descriptor(self) -> MirPassDescriptor {
        self.descriptor
    }

    pub(in crate::passes::pipeline) const fn implementation(self) -> MirPassImplementation {
        self.implementation
    }
}
