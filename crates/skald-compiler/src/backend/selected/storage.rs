//! Stage-owned selected storage, without a selected publication constructor.
use super::{AbiArea, AbiBindings, Representation, SelectionContext};
use crate::{
    backend::{
        graph::{
            DefinitionSite, LoweredBlockId, LoweredObjectId, LoweredValueId, OwnedArena,
            SelectedBlockId, SelectedObjectId, SelectedValueId,
        },
        lir::CompletionReceipt,
        plan::{CallableBinding, LayoutFact},
    },
    source::Span,
};
use std::collections::BTreeMap;
#[derive(Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct Value {
    pub(super) ty: Representation,
    pub(super) definition: Option<DefinitionSite>,
    pub(super) origin: Option<Span>,
}
#[derive(Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct Block<P> {
    pub(super) parameters: Vec<SelectedValueId>,
    pub(super) instructions: Vec<P>,
    pub(super) terminal: Option<Terminal<P>>,
    pub(super) origin: Option<Span>,
}
#[derive(Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct Terminal<P> {
    pub(super) payload: P,
    pub(super) edges: Vec<(SelectedBlockId, Vec<SelectedValueId>)>,
}
#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ObjectRole {
    Semantic,
    Trace,
    Abi(AbiArea),
}
#[derive(Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct Object {
    pub(super) layout: LayoutFact,
    pub(super) role: ObjectRole,
    pub(super) origin: Option<Span>,
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct SelectedDraft<'p, P> {
    pub(super) context: &'p SelectionContext<'p>,
    pub(super) owner: CallableBinding<'p>,
    pub(super) input: Option<CompletionReceipt<'p>>,
    pub(super) entry: Option<SelectedBlockId>,
    pub(super) inputs: Vec<SelectedValueId>,
    pub(super) abi: Option<AbiBindings>,
    pub(super) values: OwnedArena<'p, SelectedValueId, Value>,
    pub(super) blocks: OwnedArena<'p, SelectedBlockId, Block<P>>,
    pub(super) objects: OwnedArena<'p, SelectedObjectId, Object>,
    pub(super) origins: BTreeMap<LoweredValueId, SelectedValueId>,
    pub(super) block_origins: BTreeMap<LoweredBlockId, SelectedBlockId>,
    pub(super) object_origins: BTreeMap<LoweredObjectId, SelectedObjectId>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl<P> SelectedDraft<'_, P> {
    pub(in crate::backend) fn abi(&self) -> Option<&AbiBindings> {
        self.abi.as_ref()
    }
}
