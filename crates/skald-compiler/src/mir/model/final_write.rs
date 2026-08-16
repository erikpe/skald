//! Exact lifecycle evidence for direct final-field replacement.

use crate::identity::{CopyAssignmentId, FieldId};

/// Evidence that one replacement instruction belongs to the selected user
/// copy assignment of the final field's exact declaring class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirFinalWriteAuthorization {
    pub field: FieldId,
    pub operation: CopyAssignmentId,
}
