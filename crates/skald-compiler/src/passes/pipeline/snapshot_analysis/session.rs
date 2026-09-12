//! Proof-snapshot analysis lifetime, lookup, and optional memoization.

use std::{collections::BTreeMap, sync::Arc};

#[cfg(test)]
use std::collections::BTreeSet;

use crate::{
    identity::CallableId,
    mir::{MirDefinitionRef, MirProgram},
};

use super::super::optimizations::{
    solve_local_constants, LocalConstantAnalysisError, LocalConstantSolution,
};
use super::MirSnapshotAnalysisUsage;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::passes::pipeline) enum MirSnapshotAnalysisPolicy {
    #[default]
    Memoized,
    #[cfg(test)]
    MeasureOnly,
}

#[derive(Clone, Copy)]
pub(in crate::passes::pipeline) struct MirSnapshotAnalysisCheckpoint(MirSnapshotAnalysisUsage);

#[derive(Default)]
pub(in crate::passes::pipeline) struct MirProofSnapshotAnalysis {
    policy: MirSnapshotAnalysisPolicy,
    #[cfg(test)]
    requested_callables: BTreeSet<CallableId>,
    local_constants: BTreeMap<CallableId, Arc<LocalConstantSolution>>,
    usage: MirSnapshotAnalysisUsage,
}

impl MirProofSnapshotAnalysis {
    pub(in crate::passes::pipeline) fn new(policy: MirSnapshotAnalysisPolicy) -> Self {
        Self {
            policy,
            #[cfg(test)]
            requested_callables: BTreeSet::new(),
            local_constants: BTreeMap::new(),
            usage: MirSnapshotAnalysisUsage::default(),
        }
    }

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
    ) -> Result<Arc<LocalConstantSolution>, LocalConstantAnalysisError> {
        let definition = executable_definition(program, callable)
            .ok_or(LocalConstantAnalysisError::UnknownExecutableCallable { callable })?;
        match self.policy {
            #[cfg(test)]
            MirSnapshotAnalysisPolicy::MeasureOnly => {
                let repeated = !self.requested_callables.insert(callable);
                self.usage.record_uncached_request(repeated);
                solve_local_constants(definition).map(Arc::new)
            }
            MirSnapshotAnalysisPolicy::Memoized => {
                if let Some(solution) = self.local_constants.get(&callable) {
                    self.usage.record_cache_hit();
                    return Ok(Arc::clone(solution));
                }
                self.usage.record_cache_miss();
                let solution = Arc::new(solve_local_constants(definition)?);
                self.local_constants.insert(callable, Arc::clone(&solution));
                self.usage.record_insertion();
                Ok(solution)
            }
        }
    }

    pub(in crate::passes::pipeline) fn reset(&mut self) {
        #[cfg(test)]
        self.requested_callables.clear();
        self.usage.record_discarded(self.local_constants.len());
        self.local_constants.clear();
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
