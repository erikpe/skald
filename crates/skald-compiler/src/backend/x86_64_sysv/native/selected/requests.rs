//! Pure discovery over an authenticated lower body; no MIR or emission discovery.
use super::SelectionError;
use crate::backend::{
    lir::VerifiedCallable,
    plan::{ArtifactId, LirCallableId},
};
use std::collections::BTreeSet;
/// Immediate constants need no data artifact. Failure/source bytes already have
/// canonical shared keys. The admitted pilot is ABI compatible and needs no thunk.
pub(in crate::backend) fn requests(
    body: &VerifiedCallable<'_>,
) -> Result<BTreeSet<ArtifactId>, SelectionError> {
    let refs = body.receipt().references().clone();
    if matches!(body.receipt().owner().key(), LirCallableId::TargetThunk(_)) {
        return Err(SelectionError::Unsupported("native ABI thunk form"));
    }
    for key in &refs {
        require_key(*key)?;
    }
    Ok(refs)
}
fn require_key(key: ArtifactId) -> Result<(), SelectionError> {
    if matches!(key, ArtifactId::Callable(LirCallableId::TargetThunk(_))) {
        return Err(SelectionError::Unsupported("native ABI thunk form"));
    }
    Ok(())
}
/// Selection and the later discovery/executable-pass orchestrator use the same
/// rule: a concrete reference cannot enlarge the frozen request inventory.
pub(in crate::backend) fn check_references(
    requests: &BTreeSet<ArtifactId>,
    references: &BTreeSet<ArtifactId>,
) -> Result<(), SelectionError> {
    for key in references {
        require_key(*key)?;
        if !requests.contains(key) {
            return Err(SelectionError::Undiscovered(*key));
        }
    }
    Ok(())
}
