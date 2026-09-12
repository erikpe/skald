//! Checked source facts, relations, and plans for non-owning object views.
//!
//! Consumers keep syntax-specific selection and diagnostics. This facade owns
//! direct compatibility, target projections, and the point at which a shared
//! owner is retained. Immediate receivers borrow stable owners; loop-body
//! requests anchor them before an `HirObjectView` is constructed.

mod planning;
mod relation;
mod shared;
mod source;

#[cfg(test)]
mod tests;

pub(in crate::typeck) use planning::{
    plan_checked_object_view, plan_object_view, plan_resolved_object_view,
    CheckedObjectViewPlanKind, CheckedObjectViewProblem, ObjectViewProblem, ObjectViewRequest,
    ObjectViewRetention,
};
pub(in crate::typeck) use relation::{
    class_provides_view, classify_object_view_relation, ObjectViewRelation,
    ObjectViewRelationSource,
};
pub(in crate::typeck) use shared::CheckedSharedPointee;
pub(in crate::typeck) use source::{
    ObjectViewSource, ObjectViewSourceAdmission, ObjectViewSourceDiagnosticContext,
};
