//! Callable- and stage-owned dense storage shared by low-level graph owners.
//! Item-scoped non-test lint allowances expire as native graph consumers land.

mod arena;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use arena::{
    LocalHandle, LoweredBlockId, LoweredObjectId, LoweredValueId, OwnedArena, SelectedBlockId,
    SelectedObjectId, SelectedValueId,
};

mod verify;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use verify::{
    check_graph, BlockDescription, DefinitionSite, EdgeDescription, GraphDescription, GraphFailure,
    GraphIdentity, GraphLocation, GraphReason, GraphSession, GraphStage, GraphView,
    InstructionDescription, TypedUse, ValueDescription,
};
