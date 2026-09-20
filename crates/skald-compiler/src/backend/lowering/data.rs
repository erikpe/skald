//! Materialize the frozen owned data catalog, without source or layout queries.
use super::LowerError;
use crate::backend::{
    lir::{DataDefinition, DataInitializer, ProgramBuilder},
    plan::DataInitializerFact,
    planning::PlannedProgram,
};

pub(super) fn define(
    planned: &PlannedProgram<'_>,
    worklist: &mut ProgramBuilder<'_>,
) -> Result<(), LowerError> {
    for fact in &planned.plan().view().resources().data {
        let initializers = fact
            .initializers
            .iter()
            .map(|initializer| match initializer {
                DataInitializerFact::Bytes(value) => DataInitializer::Bytes(value.clone()),
                DataInitializerFact::Zero(bytes) => DataInitializer::Zero(*bytes),
                DataInitializerFact::Address {
                    target,
                    category,
                    addend,
                } => DataInitializer::Address {
                    target: *target,
                    category: *category,
                    addend: *addend,
                },
            })
            .collect();
        worklist.define_data(DataDefinition {
            key: fact.key,
            initializers,
        })?;
    }
    Ok(())
}
