use super::super::selected::{discover_requests, select, selection_context};
use super::{super::*, NativePilotError, NativePilotInspection};
use crate::backend::{
    lir::TargetDeclarations,
    lowering::lower_program_with,
    planning::{plan_program, PlannedProgram},
    selected::SelectedProgramBuilder,
    BackendInput,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write};

use super::profile::NativePilotProfile;
#[cfg(test)]
use super::profile::Phase;
#[cfg(test)]
use crate::backend::lowering::lower_program_with_profile;
#[cfg(test)]
use std::time::{Duration, Instant};

/// Compile one wholly planned final-MIR program through every new native phase.
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
    let planned = plan_program(input).map_err(NativePilotError::Planning)?;
    compile_planned(&planned, inspection, out)
}

#[cfg(test)]
pub(super) fn compile_native_pilot_profiled(
    input: BackendInput<'_>,
) -> Result<(String, NativePilotProfile), NativePilotError> {
    let total = Instant::now();
    let mut profile = NativePilotProfile::default();
    let started = Instant::now();
    let planned = plan_program(input).map_err(NativePilotError::Planning)?;
    profile.add(Phase::Planning, started.elapsed());
    let assembly = compile_planned_impl(
        &planned,
        NativePilotInspection::default(),
        None,
        Some(&mut profile),
    )?;
    profile.total = total.elapsed();
    Ok((assembly, profile))
}

/// Continue the private path after whole-program planning.
///
/// Keeping this seam visible only inside the pilot makes post-planning failure
/// tests explicit without creating a second production entry or fallback path.
pub(super) fn compile_planned(
    planned: &PlannedProgram<'_>,
    inspection: NativePilotInspection,
    out: Option<&mut dyn Write>,
) -> Result<String, NativePilotError> {
    compile_planned_impl(planned, inspection, out, None)
}

fn compile_planned_impl<'plan>(
    planned: &'plan PlannedProgram<'_>,
    inspection: NativePilotInspection,
    mut out: Option<&mut dyn Write>,
    #[allow(unused_variables, unused_mut)] mut profile: Option<&mut NativePilotProfile>,
) -> Result<String, NativePilotError> {
    // Discovery receipts never certify the executable pass. Selection repeats
    // the pure request rule after this catalog has been frozen.
    let mut requests = BTreeSet::new();
    let mut discover = |lower| {
        requests.extend(discover_requests(&lower).map_err(NativePilotError::Selection)?);
        Ok::<(), NativePilotError>(())
    };
    #[cfg(test)]
    let discovery_program = if profile.is_some() {
        let mut elapsed = Duration::ZERO;
        let result = lower_program_with_profile(planned, &mut discover, &mut elapsed)?;
        profile
            .as_deref_mut()
            .expect("profile requested")
            .add(Phase::DiscoveryLowering, elapsed);
        result
    } else {
        lower_program_with(planned, &mut discover)?
    };
    #[cfg(not(test))]
    let discovery_program = lower_program_with(planned, &mut discover)?;
    drop(discovery_program);
    let plan = planned.plan().view();
    for request in requests {
        let request_id = plan
            .artifact_id(request)
            .map_err(|error| NativePilotError::Discovery(error.into()))?;
        plan.artifact(request_id, request.category())
            .map_err(|error| NativePilotError::Discovery(error.into()))?;
    }
    // The planned program needs no target-local constants or thunks yet.
    let catalog = TargetDeclarations::new(plan)
        .freeze()
        .map_err(NativePilotError::Discovery)?;
    let context = selection_context(&catalog).map_err(NativePilotError::Abi)?;
    let symbols = planned
        .program()
        .external_links
        .iter()
        .map(|link| (link.id, link.symbol.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut selected_program = SelectedProgramBuilder::new(&context);
    let mut physical_program =
        PhysicalProgramBuilder::temporary_with_external_symbols(&context, symbols)
            .map_err(NativePilotError::PhysicalProgram)?;

    #[cfg(test)]
    let profiled = profile.is_some();
    let lower_program = {
        let mut consume = |lower: crate::backend::lir::VerifiedCallable<'plan>| {
            observe(&mut out, inspection.lowered, |out| lower.dump(out))?;
            #[cfg(test)]
            let selected = measured(&mut profile, Phase::Selection, || {
                select(&context, &lower).map_err(NativePilotError::Selection)
            })?;
            #[cfg(not(test))]
            let selected = select(&context, &lower).map_err(NativePilotError::Selection)?;
            observe(&mut out, inspection.selected, |out| selected.dump(out))?;
            #[cfg(test)]
            measured(&mut profile, Phase::Publication, || {
                selected_program
                    .complete(&selected, &selected.receipt())
                    .map_err(NativePilotError::SelectedProgram)
            })?;
            #[cfg(not(test))]
            selected_program
                .complete(&selected, &selected.receipt())
                .map_err(NativePilotError::SelectedProgram)?;
            #[cfg(test)]
            let placement = if profile.is_some() {
                let (placement, measurement) = place_native_baseline_profiled(&selected)
                    .map_err(NativePilotError::Placement)?;
                profile
                    .as_deref_mut()
                    .expect("profile requested")
                    .placements
                    .push(measurement);
                placement
            } else {
                place_native_baseline(&selected).map_err(NativePilotError::Placement)?
            };
            #[cfg(not(test))]
            let placement =
                place_native_baseline(&selected).map_err(NativePilotError::Placement)?;
            #[cfg(test)]
            let frame = measured(&mut profile, Phase::FramePlanning, || {
                plan_native_frame(&placement).map_err(NativePilotError::Frame)
            })?;
            #[cfg(not(test))]
            let frame = plan_native_frame(&placement).map_err(NativePilotError::Frame)?;
            #[cfg(test)]
            let physical = measured(&mut profile, Phase::RealizationChecking, || {
                let draft = realize_native(&selected, &placement, &frame)
                    .map_err(NativePilotError::Realization)?;
                check_native_physical(draft, &selected, &placement, &frame)
                    .map_err(NativePilotError::Physical)
            })?;
            #[cfg(not(test))]
            let physical = {
                let draft = realize_native(&selected, &placement, &frame)
                    .map_err(NativePilotError::Realization)?;
                check_native_physical(draft, &selected, &placement, &frame)
                    .map_err(NativePilotError::Physical)?
            };
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
            #[cfg(test)]
            return measured(&mut profile, Phase::Publication, || {
                physical_program
                    .complete(&physical)
                    .map_err(NativePilotError::PhysicalProgram)
            });
            #[cfg(not(test))]
            physical_program
                .complete(&physical)
                .map_err(NativePilotError::PhysicalProgram)
        };
        #[cfg(test)]
        let mut executable_lowering = Duration::ZERO;
        #[cfg(test)]
        let lower_program = if profiled {
            lower_program_with_profile(planned, &mut consume, &mut executable_lowering)?
        } else {
            lower_program_with(planned, &mut consume)?
        };
        #[cfg(not(test))]
        let lower_program = lower_program_with(planned, &mut consume)?;
        #[cfg(test)]
        if profiled {
            profile
                .as_deref_mut()
                .expect("profile requested")
                .add(Phase::ExecutableLowering, executable_lowering);
        }
        lower_program
    };
    observe(&mut out, inspection.lowered, |out| {
        lower_program.dump_inventory(out)
    })?;
    #[cfg(test)]
    let selected_program = measured(&mut profile, Phase::Publication, || {
        selected_program
            .finish(&lower_program)
            .map_err(NativePilotError::SelectedProgram)
    })?;
    #[cfg(not(test))]
    let selected_program = selected_program
        .finish(&lower_program)
        .map_err(NativePilotError::SelectedProgram)?;
    observe(&mut out, inspection.selected, |out| {
        selected_program.dump_inventory(out)
    })?;
    #[cfg(test)]
    return measured(&mut profile, Phase::Publication, || {
        physical_program
            .finish(&selected_program)
            .map(|assembly| assembly.into_string())
            .map_err(NativePilotError::PhysicalProgram)
    });
    #[cfg(not(test))]
    physical_program
        .finish(&selected_program)
        .map(|assembly| assembly.into_string())
        .map_err(NativePilotError::PhysicalProgram)
}

#[cfg(test)]
fn measured<T>(
    profile: &mut Option<&mut NativePilotProfile>,
    phase: Phase,
    operation: impl FnOnce() -> T,
) -> T {
    let Some(profile) = profile.as_deref_mut() else {
        return operation();
    };
    let started = Instant::now();
    let result = operation();
    profile.add(phase, started.elapsed());
    result
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
