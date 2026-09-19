//! Materialize the frozen owned data catalog, without source or layout queries.
use super::LowerError;
use crate::backend::{
    lir::{DataDefinition, DataInitializer, ProgramBuilder},
    plan::{ArtifactCategory, ArtifactId, DataKey, PlanError},
    planning::AdmittedProgram,
};

pub(super) fn define(
    admitted: &AdmittedProgram<'_>,
    worklist: &mut ProgramBuilder<'_>,
) -> Result<(), LowerError> {
    let trace = admitted.trace();
    let address = |key| DataInitializer::Address {
        target: ArtifactId::Data(key),
        category: ArtifactCategory::Data,
        addend: 0,
    };
    let word = |value: u64| DataInitializer::Bytes(value.to_le_bytes().to_vec());
    for artifact in admitted.plan().view().artifacts() {
        let ArtifactId::Data(key) = artifact.key else {
            continue;
        };
        let initializers = match key {
            DataKey::FailureMessage(reason) => {
                vec![DataInitializer::Bytes(reason.bytes().to_vec())]
            }
            DataKey::TraceBytes(index) => {
                vec![DataInitializer::Bytes(trace.strings[index].clone())]
            }
            DataKey::TraceContext(index) => {
                let context = &trace.contexts[index];
                let length = |key| match key {
                    DataKey::TraceBytes(i) => Ok(trace.strings[i].len() as u64),
                    _ => Err(PlanError::InvalidDomain),
                };
                vec![
                    address(context.name),
                    word(length(context.name)?),
                    address(context.path),
                    word(length(context.path)?),
                ]
            }
            DataKey::TraceLocation(index) => {
                let location = &trace.locations[index];
                vec![
                    address(location.context),
                    word(location.line),
                    word(location.column),
                ]
            }
            _ => return Err(PlanError::InvalidDomain.into()),
        };
        worklist.define_data(DataDefinition { key, initializers })?;
    }
    Ok(())
}
