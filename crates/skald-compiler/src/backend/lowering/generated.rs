//! Generated bodies admitted by the current migration stage.

use super::LowerError;
use crate::backend::{
    lir::{verify_callable, DraftBuilder, Terminator, VerifiedCallable},
    plan::{CallableBinding, LayoutId, PlanError},
    planning::{trivial_cleanup, AdmittedProgram},
};

/// Emit the empty finalizer shared by layouts whose complete destruction plan
/// contains only recursively empty bases and fields. LM07 extends this owner to
/// nontrivial class finalizers without changing dispatch-table publication.
pub(super) fn lower_trivial_class_finalizer<'plan>(
    admitted: &'plan AdmittedProgram<'_>,
    owner: CallableBinding<'plan>,
    layout: LayoutId,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let classes = admitted
        .plan()
        .view()
        .semantic()
        .classes
        .iter()
        .filter(|class| class.complete_layout == layout)
        .map(|class| class.class)
        .collect::<Vec<_>>();
    if classes.is_empty()
        || classes
            .iter()
            .any(|class| !trivial_cleanup(admitted.program(), *class))
    {
        return Err(PlanError::InvalidDomain.into());
    }
    let mut builder = DraftBuilder::new(owner)?;
    let entry = builder.reserve_block()?;
    builder.define_block(entry, &[])?;
    builder.set_entry(entry)?;
    builder.terminate(entry, Terminator::Return(vec![]))?;
    verify_callable(builder.finish()).map_err(LowerError::Verification)
}
