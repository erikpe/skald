//! Path-sensitive allocation, owner, checked-view, and anchor verification.

mod analysis;
mod state;
mod transitions;
mod uses;

use std::collections::HashSet;

use crate::mir::{BlockId, MirDefinitionRef};

use super::super::context::Verifier;

struct SharedOwnershipAnalysis<'mir, 'verifier> {
    function: MirDefinitionRef<'mir>,
    verifier: &'verifier mut Verifier<'mir>,
    reported_joins: HashSet<BlockId>,
}

impl<'mir, 'verifier> SharedOwnershipAnalysis<'mir, 'verifier> {
    fn new(function: MirDefinitionRef<'mir>, verifier: &'verifier mut Verifier<'mir>) -> Self {
        Self {
            function,
            verifier,
            reported_joins: HashSet::new(),
        }
    }

    fn error(&mut self, block: BlockId, message: impl Into<String>) {
        self.verifier
            .block_error(self.function.callable(), block, message);
    }
}

impl<'mir> Verifier<'mir> {
    pub(in crate::mir::verify) fn verify_shared_ownership(
        &mut self,
        function: MirDefinitionRef<'mir>,
    ) {
        SharedOwnershipAnalysis::new(function, self).analyze();
    }
}
