//! Only complete verification can allocate a snapshot and its completion receipt.
use super::super::CallableDraft;
use crate::backend::plan::{ArtifactId, CallableBinding};
use std::{collections::BTreeSet, sync::Arc};
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct VerifiedCallable<'p> {
    draft: CallableDraft<'p>,
    receipt: CompletionReceipt<'p>,
}
#[derive(Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct CompletionReceipt<'p> {
    owner: CallableBinding<'p>,
    snapshot: Arc<()>,
    references: BTreeSet<ArtifactId>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p> VerifiedCallable<'p> {
    pub(in crate::backend) fn draft(&self) -> &CallableDraft<'p> {
        &self.draft
    }
    pub(in crate::backend) fn receipt(&self) -> CompletionReceipt<'p> {
        self.receipt.clone()
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p> CompletionReceipt<'p> {
    pub(in crate::backend) fn owner(&self) -> CallableBinding<'p> {
        self.owner
    }
    pub(in crate::backend) fn references(&self) -> &BTreeSet<ArtifactId> {
        &self.references
    }
    pub(in crate::backend) fn same_snapshot(&self, other: &Self) -> bool {
        self.owner.require_same_owner(other.owner).is_ok()
            && Arc::ptr_eq(&self.snapshot, &other.snapshot)
    }
    pub(in crate::backend) fn matches(&self, body: &VerifiedCallable<'p>) -> bool {
        self.same_snapshot(&body.receipt)
    }
}
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn publish(
    draft: CallableDraft<'_>,
    references: BTreeSet<ArtifactId>,
) -> VerifiedCallable<'_> {
    let receipt = CompletionReceipt {
        owner: draft.owner,
        snapshot: Arc::new(()),
        references,
    };
    VerifiedCallable { draft, receipt }
}
