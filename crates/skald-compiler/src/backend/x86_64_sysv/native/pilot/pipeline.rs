use super::super::selected::{discover_requests, select, selection_context};
use super::{super::*, NativePilotError, NativePilotInspection};
use crate::backend::{
    lir::TargetDeclarations,
    lowering::lower_program_with,
    planning::{admit, AdmittedProgram},
    selected::SelectedProgramBuilder,
    BackendInput,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write};

/// Compile one wholly admitted final-MIR program through every new native phase.
///
/// This entry is deliberately compiler-private and has no legacy fallback. The
/// production target registry continues to use the established backend.
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn compile_native_pilot(
    input: BackendInput<'_>,
) -> Result<String, NativePilotError> {
    compile(input, NativePilotInspection::default(), None)
}

/// Compile through the private pilot while rendering only requested immutable
/// checkpoints. Observation failure is terminal and cannot publish a program.
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn compile_native_pilot_inspected(
    input: BackendInput<'_>,
    inspection: NativePilotInspection,
    out: &mut dyn Write,
) -> Result<String, NativePilotError> {
    compile(input, inspection, Some(out))
}

fn compile(
    input: BackendInput<'_>,
    inspection: NativePilotInspection,
    out: Option<&mut dyn Write>,
) -> Result<String, NativePilotError> {
    let admitted = admit(input).map_err(NativePilotError::Admission)?;
    compile_admitted(&admitted, inspection, out)
}

/// Continue the private path after whole-program admission.
///
/// Keeping this seam visible only inside the pilot makes post-admission failure
/// tests explicit without creating a second production entry or fallback path.
pub(super) fn compile_admitted(
    admitted: &AdmittedProgram<'_>,
    inspection: NativePilotInspection,
    mut out: Option<&mut dyn Write>,
) -> Result<String, NativePilotError> {
    // Exploratory receipts never certify the executable pass. The pure request
    // rule is repeated by selection after this catalog has been frozen.
    let mut requests = BTreeSet::new();
    let exploratory = lower_program_with(admitted, |lower| {
        requests.extend(discover_requests(&lower).map_err(NativePilotError::Selection)?);
        Ok::<(), NativePilotError>(())
    })?;
    drop(exploratory);
    let plan = admitted.plan().view();
    for request in requests {
        let request_id = plan
            .artifact_id(request)
            .map_err(|error| NativePilotError::Discovery(error.into()))?;
        plan.artifact(request_id, request.category())
            .map_err(|error| NativePilotError::Discovery(error.into()))?;
    }
    // The admitted program needs no target-local constants or thunks yet.
    let catalog = TargetDeclarations::new(plan)
        .freeze()
        .map_err(NativePilotError::Discovery)?;
    let context = selection_context(&catalog).map_err(NativePilotError::Abi)?;
    let symbols = admitted
        .program()
        .external_links
        .iter()
        .map(|link| (link.id, link.symbol.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut selected_program = SelectedProgramBuilder::new(&context);
    let mut physical_program =
        PhysicalProgramBuilder::temporary_with_external_symbols(&context, symbols)
            .map_err(NativePilotError::PhysicalProgram)?;

    let lower_program = lower_program_with(admitted, |lower| {
        observe(&mut out, inspection.lowered, |out| lower.dump(out))?;
        let selected = select(&context, &lower).map_err(NativePilotError::Selection)?;
        observe(&mut out, inspection.selected, |out| selected.dump(out))?;
        selected_program
            .complete(&selected, &selected.receipt())
            .map_err(NativePilotError::SelectedProgram)?;
        let placement = place_native_baseline(&selected).map_err(NativePilotError::Placement)?;
        let frame = plan_native_frame(&placement).map_err(NativePilotError::Frame)?;
        let draft =
            realize_native(&selected, &placement, &frame).map_err(NativePilotError::Realization)?;
        let physical = check_native_physical(draft, &selected, &placement, &frame)
            .map_err(NativePilotError::Physical)?;
        observe(&mut out, inspection.physical_checkpoint(), |out| {
            physical.inspect(
                out,
                Inspection {
                    physical: inspection.physical,
                    placement: inspection.placement,
                    frame: inspection.frame,
                },
            )
        })?;
        physical_program
            .complete(&physical)
            .map_err(NativePilotError::PhysicalProgram)
    })?;
    observe(&mut out, inspection.lowered, |out| {
        lower_program.dump_inventory(out)
    })?;
    let selected_program = selected_program
        .finish(&lower_program)
        .map_err(NativePilotError::SelectedProgram)?;
    observe(&mut out, inspection.selected, |out| {
        selected_program.dump_inventory(out)
    })?;
    physical_program
        .finish(&selected_program)
        .map(|assembly| assembly.into_string())
        .map_err(NativePilotError::PhysicalProgram)
}

fn observe(
    out: &mut Option<&mut dyn Write>,
    requested: bool,
    render: impl FnOnce(&mut dyn Write) -> fmt::Result,
) -> Result<(), NativePilotError> {
    if requested {
        render(out.as_deref_mut().expect("inspection writer supplied"))
            .map_err(NativePilotError::Observation)?;
    }
    Ok(())
}
