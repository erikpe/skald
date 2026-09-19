//! Materialize the frozen owned data catalog, without source or layout queries.
use super::LowerError;
use crate::backend::{
    lir::{DataDefinition, DataInitializer, ProgramBuilder},
    plan::{DataInitializerFact, StaticStorageDisposition},
    planning::AdmittedProgram,
};

pub(super) fn define(
    admitted: &AdmittedProgram<'_>,
    worklist: &mut ProgramBuilder<'_>,
) -> Result<(), LowerError> {
    for fact in &admitted.plan().view().resources().data {
        if matches!(
            fact.purpose,
            crate::backend::plan::DataPurpose::StaticStorage(
                StaticStorageDisposition::RetainedInactive
            )
        ) {
            continue;
        }
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
