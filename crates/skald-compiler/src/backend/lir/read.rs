//! Borrowed local schemas shared by checked mutation and independent verification.
use super::{
    AddressProvenance, BlockHandle, BuildError, Call, CallableDraft, DraftBuilder, ObjectHandle,
    Operation, ScalarCheck, ValueHandle,
};
use crate::backend::effects::Effects;
use crate::backend::graph::LoweredObjectId;
use crate::backend::plan::ScalarType;
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct DraftChecks<'a, 'p> {
    pub(super) draft: &'a CallableDraft<'p>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p> DraftBuilder<'p> {
    pub(super) fn checks(&self) -> DraftChecks<'_, 'p> {
        DraftChecks { draft: &self.draft }
    }
    pub(super) fn check_type(&self, ty: ScalarType) -> Result<(), BuildError> {
        self.checks().check_type(ty)
    }
    pub(super) fn value(
        &self,
        value: ValueHandle<'p>,
    ) -> Result<(crate::backend::graph::LoweredValueId, ScalarType), BuildError> {
        self.checks().value(value)
    }
    pub(super) fn normalize(
        &self,
        operation: Operation<ValueHandle<'p>, ObjectHandle<'p>, BlockHandle<'p>>,
    ) -> Result<(Operation, Vec<ScalarType>), BuildError> {
        self.checks().normalize(operation)
    }
    pub(super) fn check_relation(
        &self,
        relation: ScalarCheck<ValueHandle<'p>>,
    ) -> Result<ScalarCheck, BuildError> {
        self.checks().check_relation(relation)
    }
    pub(super) fn normalize_call(
        &self,
        call: Call<ValueHandle<'p>>,
        terminal: bool,
    ) -> Result<(Call, Vec<ScalarType>), BuildError> {
        self.checks().normalize_call(call, terminal)
    }
    pub(super) fn call_effects(&self, call: &Call) -> Result<Effects<LoweredObjectId>, BuildError> {
        self.checks().call_effects(call)
    }
    pub(super) fn operation_effects(
        &self,
        operation: &Operation,
    ) -> Result<Effects<LoweredObjectId>, BuildError> {
        self.checks().operation_effects(operation)
    }
    pub(super) fn result_provenance(
        &self,
        operation: &Operation,
    ) -> Result<AddressProvenance, BuildError> {
        self.checks().result_provenance(operation)
    }
    pub(super) fn check_failure_message(
        &self,
        call: &Call,
        reason: crate::backend::failure::FailureMessage,
    ) -> Result<(), BuildError> {
        self.checks().check_failure_message(call, reason)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl DraftChecks<'_, '_> {
    pub(super) fn check_object(&self, object: &super::Object) -> Result<(), BuildError> {
        let layout = object.layout;
        if layout.alignment == 0 || !layout.alignment.is_power_of_two() {
            return Err(BuildError::InvalidObject);
        }
        layout
            .size
            .checked_add(layout.alignment - 1)
            .ok_or(BuildError::SizeOverflow)?;
        if layout.disposition != crate::backend::plan::LayoutDisposition::Addressable
            && layout.size != 0
        {
            return Err(BuildError::InvalidObject);
        }
        if matches!(object.lifetime, super::LifetimeDisposition::Sites(0)) {
            return Err(BuildError::InvalidLifetime);
        }
        if object.role == super::ObjectRole::TraceRecord
            && self.draft.owner.context().runtime_trace()
                == crate::backend::RuntimeTracePolicy::Omitted
        {
            return Err(BuildError::Plan(
                crate::backend::plan::PlanError::OmittedTrace,
            ));
        }
        Ok(())
    }
}
