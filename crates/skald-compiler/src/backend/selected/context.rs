//! Frozen plan-bound catalog/resource authority and a fresh arena namespace per selection.
use super::{
    AbiAreas, AbiBinding, AbiBindings, AbiLocation, Representation, RepresentationKind,
    ResourceCatalog,
};
use crate::backend::{
    lir::{ProgramError, TargetCatalog},
    plan::{CallableBinding, LirCallableId, PlanError, SignatureId},
};
pub(in crate::backend) struct SelectionContext<'p> {
    pub(super) catalog: &'p TargetCatalog<'p>,
    pub resources: ResourceCatalog,
    pub(super) abi_areas: std::collections::BTreeMap<SignatureId, AbiAreas>,
    abi_layouts: std::collections::BTreeMap<(SignatureId, super::AbiArea), super::AbiAreaLayout>,
    scope: Box<u8>,
}
impl<'p> SelectionContext<'p> {
    pub(in crate::backend) fn new(
        catalog: &'p TargetCatalog<'p>,
        resources: ResourceCatalog,
    ) -> Self {
        Self {
            catalog,
            resources,
            abi_areas: std::collections::BTreeMap::new(),
            abi_layouts: std::collections::BTreeMap::new(),
            scope: Box::new(0),
        }
    }
    pub(in crate::backend) fn with_abi_areas(
        mut self,
        signature: SignatureId,
        areas: AbiAreas,
    ) -> Result<Self, PlanError> {
        self.catalog
            .plan()
            .signature(self.catalog.plan().signature_id(signature.index())?)?;
        if self.abi_areas.contains_key(&signature) {
            return Err(PlanError::DuplicateDeclaration);
        }
        for ty in areas
            .incoming
            .iter()
            .chain(&areas.outgoing)
            .chain(&areas.results)
        {
            self.representation(*ty)?;
        }
        self.abi_areas.insert(signature, areas);
        Ok(self)
    }
    pub(in crate::backend) fn with_abi_layout(
        mut self,
        signature: SignatureId,
        area: super::AbiArea,
        layout: super::AbiAreaLayout,
    ) -> Result<Self, PlanError> {
        let shape = self
            .areas(signature)
            .ok_or(PlanError::OutOfBounds)?
            .slots(area);
        layout.validate(shape)?;
        if self.abi_layouts.insert((signature, area), layout).is_some() {
            return Err(PlanError::DuplicateDeclaration);
        }
        Ok(self)
    }
    pub(in crate::backend) fn abi_layout(
        &self,
        signature: SignatureId,
        area: super::AbiArea,
    ) -> Option<&super::AbiAreaLayout> {
        self.abi_layouts.get(&(signature, area))
    }
    pub(in crate::backend) fn abi_bindings(
        &self,
        signature: SignatureId,
        inputs: Vec<AbiBinding>,
        results: Vec<AbiBinding>,
    ) -> Result<AbiBindings, PlanError> {
        let view = self.catalog.plan();
        let signature_id = signature;
        let signature = view.signature(view.signature_id(signature.index())?)?;
        let pointer_bits = u16::try_from(view.profile().data_layout.pointer_bytes * 8)
            .map_err(|_| PlanError::InvalidProfile)?;
        let bindings = AbiBindings::new(signature, pointer_bits, &self.resources, inputs, results)?;
        for binding in bindings.inputs().iter().chain(bindings.results()) {
            if let AbiLocation::Slot { area, index } = binding.location {
                self.require_slot(signature_id, area, index, binding.representation)?;
            }
        }
        Ok(bindings)
    }
    pub(super) fn require_abi_binding(
        &self,
        signature: SignatureId,
        binding: &AbiBinding,
    ) -> Result<(), PlanError> {
        self.representation(binding.representation)?;
        match binding.location {
            AbiLocation::Fixed(view) => self
                .resources
                .require_view(
                    view,
                    binding.representation.bits(),
                    binding.representation.bank(),
                    false,
                )
                .map_err(|_| PlanError::InvalidSignature),
            AbiLocation::Slot { area, index } => {
                self.require_slot(signature, area, index, binding.representation)
            }
        }
    }
    pub(in crate::backend) fn catalog(&self) -> &TargetCatalog<'p> {
        self.catalog
    }
    pub(in crate::backend) fn areas(&self, signature: SignatureId) -> Option<&AbiAreas> {
        self.abi_areas.get(&signature)
    }
    pub(super) fn require_slot(
        &self,
        signature: SignatureId,
        area: super::AbiArea,
        index: usize,
        representation: Representation,
    ) -> Result<(), PlanError> {
        self.areas(signature)
            .ok_or(PlanError::OutOfBounds)?
            .require_slot(area, index, representation)
    }
    pub(in crate::backend) fn binding(
        &'p self,
        key: LirCallableId,
    ) -> Result<CallableBinding<'p>, ProgramError> {
        Ok(self.catalog.selection_binding(key)?.scoped(&self.scope))
    }
    pub(super) fn representation(&self, ty: Representation) -> Result<(), PlanError> {
        let view = self.catalog.plan();
        match ty.kind {
            RepresentationKind::CodeAddress(signature) => {
                view.signature(view.signature_id(signature.index())?)?;
            }
            RepresentationKind::Float if !view.profile().capabilities.binary64 => {
                return Err(PlanError::WrongTarget)
            }
            _ => {}
        }
        if matches!(
            ty.kind,
            RepresentationKind::DataAddress | RepresentationKind::CodeAddress(_)
        ) && usize::from(ty.bits()) != view.profile().data_layout.pointer_bytes * 8
        {
            return Err(PlanError::WrongTarget);
        }
        Ok(())
    }
}
