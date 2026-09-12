//! Checked source facts, relations, and plans for non-owning object views.

mod planning;
mod relation;
mod shared;
mod source;

#[cfg(test)]
mod tests;

pub(super) use planning::project_place_to_ancestor;
pub(super) use planning::{
    plan_object_view, ObjectViewProblem, ObjectViewRequest, ObjectViewRetention,
};
pub(in crate::typeck) use relation::{
    class_provides_view, classify_object_view_relation, ObjectViewRelation,
    ObjectViewRelationSource,
};
pub(super) use shared::CheckedSharedPointee;
pub(super) use source::{
    ObjectViewSource, ObjectViewSourceAdmission, ObjectViewSourceDiagnosticContext,
};
