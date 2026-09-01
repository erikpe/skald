//! Linux x86-64 backend using the System V AMD64 ABI.
//!
//! The first implementation intentionally gives every MIR storage location
//! and value a stack home. Instruction selection uses only caller-saved
//! scratch registers, keeping register allocation an internal optimization
//! that can be replaced later.
//! Target legality, layout, ABI, and emission are documented in
//! `docs/compiler/BACKEND.md`.

mod abi;
mod array_legality;
mod artifacts;
mod dispatch;
mod emit;
mod frame;
mod layout;
mod legality;
mod literal_data;
mod lower;
mod machine;
mod planning;
mod runtime_trace;
mod static_fields;
mod symbol;

use super::{BackendError, BackendInput};

pub fn emit_assembly(input: BackendInput<'_>) -> Result<String, BackendError> {
    emit_assembly_observed(input, &mut planning::Unobserved)
}

fn emit_assembly_observed(
    input: BackendInput<'_>,
    observer: &mut impl planning::PlanningObserver,
) -> Result<String, BackendError> {
    let program = input.program();
    planning::validate_required_runtime_entities(input)?;
    let (data_layout, dispatch) = legality::check(input, observer)?;
    let metadata = runtime_trace::Metadata::new(input);
    let activations = runtime_trace::Activations::plan(program, &metadata, observer)?;
    let mut assembly = lower::lower(
        program,
        input.active_static_fields(),
        &data_layout,
        &dispatch,
        &activations,
        &metadata,
        observer,
    )?;
    assembly.runtime_trace = metadata.finish();
    if input.reachable_artifacts_only() {
        artifacts::retain_reachable(&mut assembly);
    }
    for slot in &assembly.static_slots {
        observer.visits_static_field(planning::StaticPlanningPhase::Retained, slot.field);
    }
    for slot in &assembly.static_slots {
        observer.visits_static_field(planning::StaticPlanningPhase::Emitted, slot.field);
    }
    Ok(emit::emit(&assembly))
}

#[cfg(test)]
mod tests;
