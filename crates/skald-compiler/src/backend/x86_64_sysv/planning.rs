//! Backend planning boundaries shared by x86-64 orchestration and tests.

use crate::{
    backend::{BackendError, BackendInput, BackendRequiredRuntimeEntity, Target},
    identity::{CallableId, StaticFieldId},
};

/// Callable-oriented backend work whose input domain must be physically
/// retained definitions, never the dense declaration inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DefinitionPlanningPhase {
    ArrayLegality,
    Legality,
    RuntimeTraceActivation,
    Frame,
    InstructionSelection,
}

/// Static-storage and lifecycle work observed at the backend boundary.
///
/// These stages are target diagnostics for tests and measurements only. They
/// never participate in semantic activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StaticPlanningPhase {
    Declared,
    Active,
    Initializer,
    Finalizer,
    ConservativeFallback,
    Retained,
    Emitted,
}

pub(super) trait PlanningObserver {
    fn visits_definition(&mut self, _phase: DefinitionPlanningPhase, _callable: CallableId) {}

    fn visits_static_field(&mut self, _phase: StaticPlanningPhase, _field: StaticFieldId) {}
}

pub(super) struct Unobserved;

impl PlanningObserver for Unobserved {}

/// Defensively checks the target-independent runtime obligations before any
/// target planning. Final-MIR verification normally makes every branch
/// succeed; keeping this boundary fallible gives backend callers a structured
/// error if a future entity kind or sparse-product rewrite becomes corrupt.
pub(super) fn validate_required_runtime_entities(
    input: BackendInput<'_>,
) -> Result<(), BackendError> {
    let program = input.program();
    for entity in input.required_runtime_entities() {
        let present = match entity {
            BackendRequiredRuntimeEntity::ClassDispatch(class) => program.class(class).is_some(),
            BackendRequiredRuntimeEntity::VirtualFamily(family) => {
                program.virtual_family(family).is_some()
            }
            BackendRequiredRuntimeEntity::InterfaceRequirement(requirement) => {
                program.interface_requirement(requirement).is_some()
            }
            BackendRequiredRuntimeEntity::FunctionType(function_type) => {
                program.function_type(function_type).is_some()
            }
            BackendRequiredRuntimeEntity::ArrayLifecycle(array) => {
                program.array_type(array).is_some()
            }
            BackendRequiredRuntimeEntity::OptionalLifecycle(optional) => {
                program.optional_type(optional).is_some()
            }
            BackendRequiredRuntimeEntity::OptionalBoxLayout(box_type) => {
                program.optional_box_type(box_type).is_some()
            }
            BackendRequiredRuntimeEntity::StaticStorage(field) => {
                program.static_field(field).is_some()
            }
            BackendRequiredRuntimeEntity::LiteralBacking(data) => {
                program.literal_data.get(data).is_some()
            }
        };
        if !present {
            return Err(missing_runtime_entity(entity));
        }
    }
    Ok(())
}

fn missing_runtime_entity(entity: BackendRequiredRuntimeEntity) -> BackendError {
    BackendError::new(
        Target::X86_64SysV,
        None,
        format!("verified reachability requires missing runtime entity {entity:?}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::ClassId;

    #[test]
    fn missing_required_runtime_entity_is_a_structured_target_error() {
        let error =
            missing_runtime_entity(BackendRequiredRuntimeEntity::ClassDispatch(ClassId::new(9)));

        assert_eq!(error.target(), Target::X86_64SysV);
        assert_eq!(error.callable(), None);
        assert!(error
            .message()
            .contains("missing runtime entity ClassDispatch(ClassId(9))"));
    }
}
