//! Ownership-moving publication of a validated definition-retention plan.

use super::{
    super::{MirFunctionDefinitionTable, MirMemberDefinitionTable, MirProgram},
    MirDefinitionRetentionChange, MirPreparedDefinitionRetention,
};

impl MirPreparedDefinitionRetention {
    /// Consumes the exact program used to prepare this opaque plan and
    /// publishes both rebuilt definition containers together.
    pub(crate) fn apply(self, mut program: MirProgram) -> MirDefinitionRetentionChange {
        let removed = self.summary.removed_callables();

        let function_slots = std::mem::take(&mut program.definitions)
            .into_rewrite_slots()
            .into_iter()
            .map(|slot| {
                slot.filter(|definition| removed.binary_search(&definition.callable()).is_err())
            })
            .collect();
        let members = std::mem::take(&mut program.member_definitions)
            .into_rewrite_entries()
            .into_iter()
            .filter(|definition| removed.binary_search(&definition.callable).is_err())
            .collect();

        program.definitions = MirFunctionDefinitionTable::new(function_slots);
        program.member_definitions = MirMemberDefinitionTable::new(members);
        MirDefinitionRetentionChange {
            program,
            summary: self.summary,
        }
    }
}
