//! Borrowed lookup boundaries. Equal numeric IDs never substitute for context.

use super::{
    ArtifactCategory, ArtifactDeclaration, ArtifactId, ArtifactPolicy, BodyDisposition,
    CallableDeclaration, CheckedPlan, DispatchSlot, LayoutFact, LayoutId, LirCallableId, PlanError,
    ResourceFacts, SemanticFacts, SignatureFact, SignatureId, TargetProfile,
};

#[derive(Clone, Copy)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct PlanView<'plan> {
    plan: &'plan CheckedPlan,
}

/// A compact pool ID paired with its live lookup authority.
#[derive(Clone, Copy)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct DeclarationId<'plan, I> {
    context: PlanView<'plan>,
    id: I,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct CallableBinding<'plan> {
    context: PlanView<'plan>,
    key: LirCallableId,
    signature: SignatureId,
    scope: Option<&'plan u8>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'plan> PlanView<'plan> {
    pub(super) const fn new(plan: &'plan CheckedPlan) -> Self {
        Self { plan }
    }

    pub(in crate::backend) fn require_same_context(self, other: Self) -> Result<(), PlanError> {
        if self.profile() != other.profile() {
            return Err(PlanError::WrongTarget);
        }
        if !std::ptr::eq(self.plan, other.plan) {
            return Err(PlanError::WrongContext);
        }
        Ok(())
    }

    pub(in crate::backend) const fn profile(self) -> TargetProfile {
        self.plan.profile
    }
    pub(in crate::backend) const fn runtime_trace(self) -> crate::backend::RuntimeTracePolicy {
        self.plan.runtime_trace
    }
    pub(in crate::backend) const fn artifact_policy(self) -> ArtifactPolicy {
        self.plan.artifact_policy
    }

    pub(in crate::backend) fn layout_id(
        self,
        index: usize,
    ) -> Result<DeclarationId<'plan, LayoutId>, PlanError> {
        self.plan
            .layouts
            .get(index)
            .ok_or(PlanError::UnknownDeclaration)?;
        Ok(DeclarationId {
            context: self,
            id: LayoutId::new(index),
        })
    }
    pub(in crate::backend) fn signature_id(
        self,
        index: usize,
    ) -> Result<DeclarationId<'plan, SignatureId>, PlanError> {
        self.plan
            .signatures
            .get(index)
            .ok_or(PlanError::UnknownDeclaration)?;
        Ok(DeclarationId {
            context: self,
            id: SignatureId::new(index),
        })
    }
    pub(in crate::backend) fn artifact_id(
        self,
        key: ArtifactId,
    ) -> Result<DeclarationId<'plan, ArtifactId>, PlanError> {
        self.plan
            .artifacts
            .get(&key)
            .ok_or(PlanError::UnknownDeclaration)?;
        Ok(DeclarationId {
            context: self,
            id: key,
        })
    }

    pub(in crate::backend) fn layout(
        self,
        id: DeclarationId<'plan, LayoutId>,
    ) -> Result<&'plan LayoutFact, PlanError> {
        self.require_same_context(id.context)?;
        self.plan
            .layouts
            .get(id.id.index())
            .ok_or(PlanError::UnknownDeclaration)
    }
    pub(in crate::backend) fn signature(
        self,
        id: DeclarationId<'plan, SignatureId>,
    ) -> Result<&'plan SignatureFact, PlanError> {
        self.require_same_context(id.context)?;
        self.plan
            .signatures
            .get(id.id.index())
            .ok_or(PlanError::UnknownDeclaration)
    }
    pub(in crate::backend) fn artifact(
        self,
        id: DeclarationId<'plan, ArtifactId>,
        category: ArtifactCategory,
    ) -> Result<&'plan ArtifactDeclaration, PlanError> {
        self.require_same_context(id.context)?;
        if id.id.category() != category {
            return Err(PlanError::ArtifactCategoryMismatch);
        }
        self.plan
            .artifacts
            .get(&id.id)
            .ok_or(PlanError::UnknownDeclaration)
    }

    pub(in crate::backend) fn callable(
        self,
        key: LirCallableId,
    ) -> Result<CallableBinding<'plan>, PlanError> {
        let declaration = self
            .plan
            .callables
            .get(&key)
            .ok_or(PlanError::UnknownDeclaration)?;
        if declaration.body != BodyDisposition::Required {
            return Err(PlanError::AbsentBody);
        }
        Ok(CallableBinding {
            context: self,
            key,
            signature: declaration.signature,
            scope: None,
        })
    }

    /// Called only after the frozen extension has resolved a generated thunk.
    pub(in crate::backend) fn thunk_binding(
        self,
        key: LirCallableId,
        signature: SignatureId,
    ) -> Result<CallableBinding<'plan>, PlanError> {
        if !matches!(key, LirCallableId::TargetThunk(_)) {
            return Err(PlanError::UnknownDeclaration);
        }
        self.signature(self.signature_id(signature.index())?)?;
        Ok(CallableBinding {
            context: self,
            key,
            signature,
            scope: None,
        })
    }
    pub(in crate::backend) fn callables(
        self,
    ) -> impl ExactSizeIterator<Item = &'plan CallableDeclaration> {
        self.plan.callables.values()
    }
    pub(in crate::backend) fn artifacts(
        self,
    ) -> impl ExactSizeIterator<Item = &'plan ArtifactDeclaration> {
        self.plan.artifacts.values()
    }
    pub(in crate::backend) fn layouts(self) -> impl ExactSizeIterator<Item = &'plan LayoutFact> {
        self.plan.layouts.iter()
    }
    pub(in crate::backend) fn signatures_with_ids(
        self,
    ) -> impl ExactSizeIterator<Item = (SignatureId, &'plan SignatureFact)> {
        self.plan
            .signatures
            .iter()
            .enumerate()
            .map(|(index, fact)| (SignatureId::new(index), fact))
    }
    pub(in crate::backend) fn signatures(
        self,
    ) -> impl ExactSizeIterator<Item = &'plan SignatureFact> {
        self.plan.signatures.iter()
    }
    pub(in crate::backend) fn dispatch(self) -> &'plan [DispatchSlot] {
        &self.plan.dispatch
    }
    pub(in crate::backend) fn semantic(self) -> &'plan SemanticFacts {
        &self.plan.semantic
    }
    pub(in crate::backend) fn resources(self) -> &'plan ResourceFacts {
        &self.plan.resources
    }
    pub(in crate::backend) fn static_storage_disposition(
        self,
        field: crate::identity::StaticFieldId,
    ) -> Option<super::StaticStorageDisposition> {
        self.plan
            .resources
            .static_storage(field)
            .map(|fact| fact.disposition)
    }
    pub(in crate::backend) fn is_active_static(
        self,
        field: crate::identity::StaticFieldId,
    ) -> bool {
        self.plan.active_statics.contains(&field)
    }

    pub(in crate::backend) fn permits_static_reference(
        self,
        field: crate::identity::StaticFieldId,
    ) -> bool {
        match self.static_storage_disposition(field) {
            Some(super::StaticStorageDisposition::Active) => true,
            Some(super::StaticStorageDisposition::RetainedInactive) => {
                self.artifact_policy() == super::ArtifactPolicy::Complete
            }
            None => false,
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'plan> CallableBinding<'plan> {
    /// Namespace a selected draft without granting completion or publication.
    pub(in crate::backend) fn scoped(mut self, scope: &'plan u8) -> Self {
        self.scope = Some(scope);
        self
    }
    pub(in crate::backend) const fn context(self) -> PlanView<'plan> {
        self.context
    }
    pub(in crate::backend) const fn key(self) -> LirCallableId {
        self.key
    }
    pub(in crate::backend) const fn signature_id(self) -> SignatureId {
        self.signature
    }
    pub(in crate::backend) fn signature(self) -> Result<&'plan SignatureFact, PlanError> {
        self.context.signature(DeclarationId {
            context: self.context,
            id: self.signature,
        })
    }
    pub(in crate::backend) fn require_same_owner(self, other: Self) -> Result<(), PlanError> {
        self.context.require_same_context(other.context)?;
        if !match (self.scope, other.scope) {
            (None, None) => true,
            (Some(a), Some(b)) => std::ptr::eq(a, b),
            _ => false,
        } {
            return Err(PlanError::WrongContext);
        }
        if self.key != other.key {
            return Err(PlanError::WrongOwner);
        }
        Ok(())
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl LayoutFact {
    pub(in crate::backend) fn checked_access(
        self,
        offset: usize,
        bytes: usize,
    ) -> Result<(), PlanError> {
        let end = offset.checked_add(bytes).ok_or(PlanError::SizeOverflow)?;
        if end > self.size {
            return Err(PlanError::OutOfBounds);
        }
        Ok(())
    }
}
