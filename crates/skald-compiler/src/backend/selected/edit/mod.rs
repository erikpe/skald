//! Target-owned rewriting over consuming selected snapshots; no opcode knowledge here.
mod editor;
mod rebuild;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use editor::{EditablePayload, SelectedEditFailure, SelectedEditor};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use rebuild::SelectedRemap;
