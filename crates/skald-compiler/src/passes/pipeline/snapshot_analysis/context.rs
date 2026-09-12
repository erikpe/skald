//! Stage-specific capabilities that add proof-snapshot analysis queries.

use crate::{
    identity::CallableId,
    mir::{
        rewrite::{MirCallableEdit, MirRewriteError},
        MirDefinitionRef, MirProgram,
    },
};

use super::{
    super::{
        execution::{
            MirPassData, MirPassFailure, MirProofPassCapability, MirProofPassOutcome,
            MirProofTransitionCapability, MirProofTransitionFailure, MirProofTransitionOutcome,
        },
        optimizations::{LocalConstantAnalysisError, LocalConstantSolution},
    },
    MirProofSnapshotAnalysis,
};
use crate::passes::pipeline::normalization::MirProofTransitionPlan;

pub(in crate::passes::pipeline) struct MirProofPassContext<'session> {
    capability: MirProofPassCapability,
    analyses: &'session mut MirProofSnapshotAnalysis,
}

impl<'session> MirProofPassContext<'session> {
    pub(in crate::passes::pipeline) fn new(
        capability: MirProofPassCapability,
        analyses: &'session mut MirProofSnapshotAnalysis,
    ) -> Self {
        Self {
            capability,
            analyses,
        }
    }

    pub(in crate::passes::pipeline) const fn verified(
        &self,
    ) -> &super::super::VerifiedProofMirProgram {
        self.capability.verified()
    }

    pub(in crate::passes::pipeline) fn executable_callables(&self) -> Vec<CallableId> {
        executable_callables(self.verified().program())
    }

    pub(in crate::passes::pipeline) fn local_constants(
        &mut self,
        callable: CallableId,
    ) -> Result<LocalConstantSolution, LocalConstantAnalysisError> {
        self.analyses
            .local_constants(self.capability.verified().program(), callable)
    }

    pub(in crate::passes::pipeline) fn executable_definition(
        &self,
        callable: CallableId,
    ) -> MirDefinitionRef<'_> {
        executable_definition(self.verified().program(), callable)
    }

    #[cfg(test)]
    pub(in crate::passes::pipeline) fn unchanged(self) -> MirProofPassOutcome {
        self.capability.unchanged()
    }

    pub(in crate::passes::pipeline) fn unchanged_with(
        self,
        data: MirPassData,
    ) -> Result<MirProofPassOutcome, MirPassFailure> {
        self.capability.unchanged_with(data)
    }

    pub(in crate::passes::pipeline) fn rewrite(
        self,
        rewrite: impl FnMut(CallableId, &mut MirCallableEdit) -> Result<(), MirRewriteError>,
    ) -> Result<super::super::execution::MirProofChangedProgram, MirPassFailure> {
        self.capability.rewrite(rewrite)
    }
}

pub(in crate::passes::pipeline) struct MirProofTransitionContext<'session> {
    capability: MirProofTransitionCapability,
    analyses: &'session mut MirProofSnapshotAnalysis,
}

impl<'session> MirProofTransitionContext<'session> {
    pub(in crate::passes::pipeline) fn new(
        capability: MirProofTransitionCapability,
        analyses: &'session mut MirProofSnapshotAnalysis,
    ) -> Self {
        Self {
            capability,
            analyses,
        }
    }

    pub(in crate::passes::pipeline) const fn verified(
        &self,
    ) -> &super::super::VerifiedProofMirProgram {
        self.capability.verified()
    }

    pub(in crate::passes::pipeline) fn executable_callables(&self) -> Vec<CallableId> {
        executable_callables(self.verified().program())
    }

    pub(in crate::passes::pipeline) fn local_constants(
        &mut self,
        callable: CallableId,
    ) -> Result<LocalConstantSolution, LocalConstantAnalysisError> {
        self.analyses
            .local_constants(self.capability.verified().program(), callable)
    }

    pub(in crate::passes::pipeline) fn executable_definition(
        &self,
        callable: CallableId,
    ) -> MirDefinitionRef<'_> {
        executable_definition(self.verified().program(), callable)
    }

    pub(in crate::passes::pipeline) fn normalize(
        self,
        optional_plan: Option<MirProofTransitionPlan>,
        data: MirPassData,
    ) -> Result<MirProofTransitionOutcome, MirProofTransitionFailure> {
        self.capability.normalize(optional_plan, data)
    }
}

fn executable_callables(program: &MirProgram) -> Vec<CallableId> {
    program
        .executable_definitions()
        .map(|definition| definition.callable())
        .collect()
}

fn executable_definition(program: &MirProgram, callable: CallableId) -> MirDefinitionRef<'_> {
    program
        .executable_definitions()
        .find(|definition| definition.callable() == callable)
        .expect("an executable callable collected from this context must still exist")
}
