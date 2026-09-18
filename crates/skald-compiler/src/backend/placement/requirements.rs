//! Reconstruct selected requirements and immutable target footprints once per check.
use super::{model::*, state::State, target::*};
use crate::{
    backend::{
        graph::{SelectedBlockId, SelectedValueId},
        selected::{AbiBinding, Payload, Representation, SelectedFact, UnitId, ViewId},
    },
    source::Span,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum CheckReason {
    WrongTarget,
    Capacity,
    Convergence,
    Constraint,
    Reserved,
    Abi,
    Tie,
    Overlap,
    Scratch,
    Lifetime,
    Preservation,
    Transfer,
    MissingValue,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum CheckLocation {
    Entry,
    Assignment(Assignment),
    Storage(usize),
    Site(Site),
    Transfer { point: TransferPoint, index: usize },
    Edge { block: SelectedBlockId, slot: usize },
}
impl CheckLocation {
    pub(super) fn site(self) -> Option<Site> {
        match self {
            CheckLocation::Site(site)
            | CheckLocation::Assignment(
                Assignment::Operand { site, .. } | Assignment::Scratch { site, .. },
            ) => Some(site),
            CheckLocation::Transfer {
                point: TransferPoint::Before(site) | TransferPoint::After(site),
                ..
            } => Some(site),
            CheckLocation::Edge { block, .. }
            | CheckLocation::Transfer {
                point: TransferPoint::Edge { block, .. },
                ..
            } => Some(Site::Terminal(block)),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::backend) struct CheckFailure {
    pub location: CheckLocation,
    pub reason: CheckReason,
    pub origin: Option<Span>,
    pub structure: Option<Box<PlacementError>>,
}
impl CheckFailure {
    pub(super) fn new(location: CheckLocation, reason: CheckReason) -> Self {
        Self {
            location,
            reason,
            origin: None,
            structure: None,
        }
    }
}
pub(super) struct Block<'s, P> {
    pub parameters: Vec<SelectedValueId>,
    pub instructions: Vec<&'s P>,
    pub terminal: Option<&'s P>,
    pub edges: Vec<(SelectedBlockId, Vec<SelectedValueId>)>,
    pub origin: Option<Span>,
}
pub(super) struct Requirements<'s, P> {
    pub entry: SelectedBlockId,
    pub inputs: Vec<SelectedValueId>,
    pub abi: Vec<AbiBinding>,
    pub values: BTreeMap<SelectedValueId, Representation>,
    pub blocks: BTreeMap<SelectedBlockId, Block<'s, P>>,
    pub origins: BTreeMap<Site, Option<Span>>,
    pub locations: Vec<Location>,
    pub tokens: Vec<TransferValue>,
    pub footprints: BTreeMap<Location, SlotFootprint>,
    pub preserved: Vec<ViewId>,
    pub move_kills: BTreeMap<TransferPoint, Vec<Vec<UnitId>>>,
    pub seed: State,
}
impl<'s, P: Payload> Requirements<'s, P> {
    pub(super) fn collect<'p>(
        draft: &PlacementDraft<'s, 'p, P>,
        target: &impl PlacementTarget,
    ) -> Result<Self, CheckFailure> {
        let mut entry = None;
        let mut inputs = vec![];
        let mut abi = vec![];
        let mut values = BTreeMap::new();
        let mut blocks = BTreeMap::new();
        let mut origins = BTreeMap::new();
        draft
            .selected
            .visit::<()>(|fact| {
                match fact {
                    SelectedFact::Entry {
                        entry: id,
                        inputs: ids,
                        abi: bindings,
                    } => {
                        entry = id;
                        inputs.extend_from_slice(ids);
                        if let Some(bindings) = bindings {
                            abi.extend_from_slice(bindings.inputs());
                        }
                    }
                    SelectedFact::Value {
                        id, representation, ..
                    } => {
                        values.insert(id, representation);
                    }
                    SelectedFact::Block {
                        id,
                        parameters,
                        origin,
                    } => {
                        blocks.insert(
                            id,
                            Block {
                                parameters: parameters.to_vec(),
                                instructions: vec![],
                                terminal: None,
                                edges: vec![],
                                origin,
                            },
                        );
                    }
                    SelectedFact::Instruction {
                        block,
                        ordinal,
                        payload,
                    } => {
                        let facts = blocks.get_mut(&block).expect("verified block");
                        facts.instructions.push(payload);
                        origins.insert(
                            Site::Instruction { block, ordinal },
                            payload.span().or(facts.origin),
                        );
                    }
                    SelectedFact::Terminal {
                        block,
                        payload,
                        edges,
                    } => {
                        let facts = blocks.get_mut(&block).expect("verified block");
                        facts.terminal = payload;
                        facts.edges = edges.to_vec();
                        origins.insert(
                            Site::Terminal(block),
                            payload.and_then(Payload::span).or(facts.origin),
                        );
                    }
                    _ => {}
                }
                Ok(())
            })
            .expect("infallible visitor");
        let context = draft.selected.draft().context();
        if context.catalog().plan().profile() != target.profile() {
            return Err(CheckFailure::new(
                CheckLocation::Entry,
                CheckReason::WrongTarget,
            ));
        }
        let mut locations: Vec<_> = context
            .resources
            .views()
            .map(|(view, _)| Location::Resource(view))
            .collect();
        locations.extend((0..draft.storage.len()).map(|i| Location::Storage(StorageId(i))));
        let mut footprints = BTreeMap::new();
        draft.selected.visit(|fact| {
            if let SelectedFact::Resources { abi_areas, .. } = fact {
                for (&signature, areas) in abi_areas {
                    for area in [
                        crate::backend::selected::AbiArea::Incoming,
                        crate::backend::selected::AbiArea::Outgoing,
                        crate::backend::selected::AbiArea::Results,
                    ] {
                        for (index, &representation) in areas.slots(area).iter().enumerate() {
                            let footprint = target
                                .slot_footprint(signature, area, index, representation)
                                .map_err(|reason| {
                                    CheckFailure::new(CheckLocation::Entry, reason)
                                })?;
                            if footprint.bytes < usize::from(representation.bits()).div_ceil(8)
                                || footprint.offset.checked_add(footprint.bytes).is_none()
                            {
                                return Err(CheckFailure::new(
                                    CheckLocation::Entry,
                                    CheckReason::Capacity,
                                ));
                            }
                            let location = Location::Abi {
                                signature,
                                area,
                                index,
                            };
                            locations.push(location);
                            footprints.insert(location, footprint);
                        }
                    }
                }
            }
            Ok(())
        })?;
        locations.sort();
        let preserved = target.preserved_views();
        let mut tokens: Vec<_> = values
            .keys()
            .copied()
            .map(TransferValue::Selected)
            .collect();
        tokens.extend(preserved.iter().copied().map(TransferValue::Preserved));
        let seed = State::empty(locations.len());
        Ok(Self {
            entry: entry.expect("verified entry"),
            inputs,
            abi,
            values,
            blocks,
            origins,
            locations,
            tokens,
            footprints,
            preserved,
            move_kills: BTreeMap::new(),
            seed,
        })
    }
    pub(super) fn failure(&self, location: CheckLocation, reason: CheckReason) -> CheckFailure {
        let mut failure = CheckFailure::new(location, reason);
        failure.origin = location
            .site()
            .and_then(|site| self.origins.get(&site).copied().flatten());
        failure
    }
}
