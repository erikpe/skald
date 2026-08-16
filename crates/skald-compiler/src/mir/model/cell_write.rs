//! Explicit authorization for whole-field interior replacement.

use crate::identity::FieldId;

/// Evidence that type checking authorized one exact declaring-class cell
/// field as the complete destination of an assignment instruction.
///
/// The destination place remains read-only. MIR verification independently
/// proves this field identity against the place, declaration, and enclosing
/// callable before permitting the ordinary assignment machinery to use it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirCellWriteAuthorization {
    pub field: FieldId,
}
