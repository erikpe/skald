//! The only selected snapshot constructor runs after both verification layers.
use crate::backend::selected::{SelectedDraft, SelectionContext};
use crate::backend::{
    lir::CompletionReceipt,
    plan::{ArtifactId, LirCallableId, PlanError},
};
use std::{collections::BTreeSet, sync::Arc};
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct VerifiedSelectedCallable<'p, P> {
    draft: SelectedDraft<'p, P>,
    receipt: SelectedReceipt<'p>,
}
#[derive(Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct SelectedReceipt<'p> {
    context: &'p SelectionContext<'p>,
    key: LirCallableId,
    snapshot: Arc<()>,
    input: Option<CompletionReceipt<'p>>,
    references: BTreeSet<ArtifactId>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p, P> VerifiedSelectedCallable<'p, P> {
    pub(in crate::backend) fn draft(&self) -> &SelectedDraft<'p, P> {
        &self.draft
    }
    pub(in crate::backend) fn receipt(&self) -> SelectedReceipt<'p> {
        self.receipt.clone()
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p> SelectedReceipt<'p> {
    pub(in crate::backend) fn key(&self) -> LirCallableId {
        self.key
    }
    pub(in crate::backend) fn input(&self) -> Option<&CompletionReceipt<'p>> {
        self.input.as_ref()
    }
    pub(in crate::backend) fn references(&self) -> &BTreeSet<ArtifactId> {
        &self.references
    }
    pub(in crate::backend) fn require_context(
        &self,
        context: &SelectionContext<'p>,
    ) -> Result<(), PlanError> {
        self.context
            .extension
            .parent()
            .parent()
            .require_same_context(context.extension.parent().parent())?;
        if !std::ptr::eq(self.context, context) {
            return Err(PlanError::WrongContext);
        }
        Ok(())
    }
    pub(in crate::backend) fn matches<P>(&self, product: &VerifiedSelectedCallable<'p, P>) -> bool {
        self.require_context(product.draft.context).is_ok()
            && self.key == product.receipt.key
            && Arc::ptr_eq(&self.snapshot, &product.receipt.snapshot)
    }
}
pub(super) fn publish<'p, P>(
    draft: SelectedDraft<'p, P>,
    references: BTreeSet<ArtifactId>,
) -> VerifiedSelectedCallable<'p, P> {
    let receipt = SelectedReceipt {
        context: draft.context,
        key: draft.owner.key(),
        snapshot: Arc::new(()),
        input: draft.input.clone(),
        references,
    };
    VerifiedSelectedCallable { draft, receipt }
}
