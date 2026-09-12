//! Uncached proof-snapshot analysis boundary used to establish baseline demand.

use std::collections::BTreeSet;

use crate::{
    identity::CallableId,
    mir::{MirDefinitionRef, MirProgram},
};

use super::super::optimizations::{
    solve_local_constants, LocalConstantAnalysisError, LocalConstantSolution,
};
use super::MirSnapshotAnalysisUsage;

#[derive(Clone, Copy)]
pub(in crate::passes::pipeline) struct MirSnapshotAnalysisCheckpoint(MirSnapshotAnalysisUsage);

#[derive(Default)]
pub(in crate::passes::pipeline) struct MirProofSnapshotAnalysis {
    requested_callables: BTreeSet<CallableId>,
    usage: MirSnapshotAnalysisUsage,
}

impl MirProofSnapshotAnalysis {
    pub(in crate::passes::pipeline) const fn checkpoint(&self) -> MirSnapshotAnalysisCheckpoint {
        MirSnapshotAnalysisCheckpoint(self.usage)
    }

    pub(in crate::passes::pipeline) fn usage_since(
        &self,
        checkpoint: MirSnapshotAnalysisCheckpoint,
    ) -> MirSnapshotAnalysisUsage {
        MirSnapshotAnalysisUsage::between(checkpoint.0, self.usage)
    }

    pub(super) fn local_constants(
        &mut self,
        program: &MirProgram,
        callable: CallableId,
    ) -> Result<LocalConstantSolution, LocalConstantAnalysisError> {
        let definition = executable_definition(program, callable)
            .expect("a proof pass may request only an executable callable from its exact input");
        let repeated = !self.requested_callables.insert(callable);
        self.usage.record_uncached_request(repeated);
        solve_local_constants(definition)
    }

    pub(in crate::passes::pipeline) fn reset(&mut self) {
        self.requested_callables.clear();
    }
}

fn executable_definition(
    program: &MirProgram,
    callable: CallableId,
) -> Option<MirDefinitionRef<'_>> {
    program
        .executable_definitions()
        .find(|definition| definition.callable() == callable)
}
