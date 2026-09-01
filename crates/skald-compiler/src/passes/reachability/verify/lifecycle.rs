//! Exact lifecycle-root and runtime-entity obligations in final reachability.

use crate::mir::{MirProgram, MirVerificationError, MirVerificationErrors};

use super::super::roots::collect_reachability_roots;
use super::super::{MirReachabilityAnalysis, MirReachabilityRootTarget, MirRuntimeEntity};

pub(in crate::passes) fn verify_active_lifecycle_reachability(
    program: &MirProgram,
    analysis: &MirReachabilityAnalysis,
) -> Result<(), MirVerificationErrors> {
    let expected = collect_reachability_roots(program).map_err(|error| {
        MirVerificationErrors::program(format!(
            "cannot independently derive final reachability roots: {error}"
        ))
    })?;
    let mut errors = Vec::new();

    if analysis.roots() != expected.roots {
        errors.push(program_error(
            "sealed final reachability roots disagree with the exact program roots",
        ));
    }
    for entity in expected.runtime_entities {
        if !analysis.runtime_entities().contains(&entity) {
            errors.push(program_error(format!(
                "sealed final reachability omits required lifecycle runtime entity {entity:?}"
            )));
        }
    }
    for root in analysis.roots() {
        match root.target() {
            MirReachabilityRootTarget::Execution(node) if !analysis.is_reachable(node) => {
                errors.push(program_error(format!(
                    "sealed final reachability omits executable lifecycle root {node:?} selected by {:?}",
                    root.reason()
                )));
            }
            MirReachabilityRootTarget::RuntimeEntity(entity)
                if !analysis.runtime_entities().contains(&entity) =>
            {
                errors.push(program_error(format!(
                    "sealed final reachability omits runtime lifecycle root {entity:?} selected by {:?}",
                    root.reason()
                )));
            }
            MirReachabilityRootTarget::Execution(_)
            | MirReachabilityRootTarget::RuntimeEntity(_) => {}
        }
    }

    if let Some(coordinator) = &program.static_lifecycle {
        for field in coordinator.lifecycle().proof().activation().fields() {
            if !analysis
                .runtime_entities()
                .contains(&MirRuntimeEntity::StaticStorage(*field))
            {
                errors.push(program_error(format!(
                    "sealed final reachability omits storage for active static field {field}"
                )));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(MirVerificationErrors::new(errors))
    }
}

fn program_error(message: impl Into<String>) -> MirVerificationError {
    MirVerificationError {
        callable: None,
        block: None,
        message: message.into(),
    }
}
