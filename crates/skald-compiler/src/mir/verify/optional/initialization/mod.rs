//! Path-sensitive definite initialization for optional storage.

mod checks;
mod flow;
mod state;

use crate::mir::MirDefinitionRef;

use super::super::context::Verifier;

impl Verifier<'_> {
    pub(in crate::mir::verify) fn verify_optional_initialization(
        &mut self,
        function: MirDefinitionRef<'_>,
    ) {
        let analysis = flow::analyze(self.program, function);
        checks::verify(self, function, &analysis);
    }
}
