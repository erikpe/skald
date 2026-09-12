//! Checked source facts and relations for non-owning object views.

mod relation;
mod shared;
mod source;

#[cfg(test)]
mod tests;

pub(in crate::typeck) use relation::{
    class_provides_view, classify_object_view_relation, ObjectViewRelation,
    ObjectViewRelationSource,
};
pub(super) use shared::{view_shared_target, CheckedSharedPointee};
pub(super) use source::{
    ObjectViewSource, ObjectViewSourceAdmission, ObjectViewSourceDiagnosticContext,
};
