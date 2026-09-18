//! Logical components stay intact when assigning target ABI locations.
use super::{AbiBinding, AbiLocation, Representation, ResourceCatalog};
use crate::backend::plan::{PlanError, SignatureFact};
#[derive(Clone)]
pub(in crate::backend) struct AbiBindings {
    inputs: Vec<AbiBinding>,
    results: Vec<AbiBinding>,
}
impl AbiBindings {
    pub(in crate::backend) fn new(
        signature: &SignatureFact,
        pointer_bits: u16,
        resources: &ResourceCatalog,
        inputs: Vec<AbiBinding>,
        results: Vec<AbiBinding>,
    ) -> Result<Self, PlanError> {
        for (components, bindings) in [(&signature.inputs, &inputs), (&signature.results, &results)]
        {
            if components.len() != bindings.len() {
                return Err(PlanError::InvalidSignature);
            }
            for (component, binding) in components.iter().zip(bindings) {
                let representation = Representation::from_scalar(component.ty, pointer_bits)
                    .ok_or(PlanError::InvalidSignature)?;
                if *component != binding.component || representation != binding.representation {
                    return Err(PlanError::InvalidSignature);
                }
                if let AbiLocation::Fixed(view) = binding.location {
                    resources
                        .require_view(view, representation.bits(), representation.bank(), false)
                        .map_err(|_| PlanError::InvalidSignature)?;
                }
            }
        }
        Ok(Self { inputs, results })
    }
    pub(in crate::backend) fn inputs(&self) -> &[AbiBinding] {
        &self.inputs
    }
    pub(in crate::backend) fn results(&self) -> &[AbiBinding] {
        &self.results
    }
}

/// Areas contain component-sized symbolic slots, never frame byte offsets.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::backend) struct AbiAreas {
    pub incoming: Vec<Representation>,
    pub outgoing: Vec<Representation>,
    pub results: Vec<Representation>,
}
impl AbiAreas {
    pub(in crate::backend) fn slots(&self, area: super::AbiArea) -> &[Representation] {
        match area {
            super::AbiArea::Incoming => &self.incoming,
            super::AbiArea::Outgoing => &self.outgoing,
            super::AbiArea::Results => &self.results,
        }
    }
    pub(in crate::backend) fn require_slot(
        &self,
        area: super::AbiArea,
        index: usize,
        representation: Representation,
    ) -> Result<(), PlanError> {
        let slots = match area {
            super::AbiArea::Incoming => &self.incoming,
            super::AbiArea::Outgoing => &self.outgoing,
            super::AbiArea::Results => &self.results,
        };
        match slots.get(index) {
            Some(actual) if *actual == representation => Ok(()),
            Some(_) => Err(PlanError::InvalidSignature),
            None => Err(PlanError::OutOfBounds),
        }
    }
}
