//! Immutable callable-local control-flow facts.
//!
//! This analysis deliberately owns only a short-lived snapshot. It does not
//! cache facts across rewrites or infer higher-level dominance, liveness, or
//! loop structure.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{
    identity::CallableId,
    mir::{classify_local_identity_site, BlockId, MirDefinitionRef, MirIdentitySiteRole, ValueId},
};

use super::{
    census::{value_use_census_for_definition, MirValueUseCensus},
    edit::MirCallableEdit,
    map::observe_definition_local_identities,
    MirLocalIdentity, MirLocalIdentityObserver, MirLocalIdentitySite, MirReferenceFailure,
    MirRewriteError,
};

/// One non-executable reference which protects a block from local deletion.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MirProtectedBlockRoot {
    site: MirLocalIdentitySite,
    block: BlockId,
}

impl MirProtectedBlockRoot {
    pub(crate) const fn site(self) -> MirLocalIdentitySite {
        self.site
    }

    pub(crate) const fn block(self) -> BlockId {
        self.block
    }
}

/// Successors and transient definitions owned by one block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirLocalCfgBlockFacts {
    block: BlockId,
    successors: Vec<BlockId>,
    defined_values: Vec<ValueId>,
}

impl MirLocalCfgBlockFacts {
    pub(crate) const fn block(&self) -> BlockId {
        self.block
    }

    pub(crate) fn successors(&self) -> &[BlockId] {
        &self.successors
    }

    pub(crate) fn defined_values(&self) -> &[ValueId] {
        &self.defined_values
    }
}

/// Deterministic roots, adjacency, closure, and block-owned value facts.
///
/// All block lists use the callable's current explicit block order. Protected
/// roots retain exhaustive identity-observation order and may therefore name
/// the same block in more than one semantic role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirLocalCfgFacts {
    callable: CallableId,
    entry: BlockId,
    protected_roots: Vec<MirProtectedBlockRoot>,
    blocks: Vec<MirLocalCfgBlockFacts>,
    entry_reachable: Vec<BlockId>,
    reachable: Vec<BlockId>,
    protected_but_entry_unreachable: Vec<BlockId>,
    unreachable: Vec<BlockId>,
}

impl MirLocalCfgFacts {
    pub(crate) const fn callable(&self) -> CallableId {
        self.callable
    }

    pub(crate) const fn entry(&self) -> BlockId {
        self.entry
    }

    pub(crate) fn protected_roots(&self) -> &[MirProtectedBlockRoot] {
        &self.protected_roots
    }

    pub(crate) fn blocks(&self) -> &[MirLocalCfgBlockFacts] {
        &self.blocks
    }

    pub(crate) fn block(&self, block: BlockId) -> Option<&MirLocalCfgBlockFacts> {
        (block.callable() == self.callable)
            .then(|| self.blocks.iter().find(|facts| facts.block == block))
            .flatten()
    }

    pub(crate) fn entry_reachable(&self) -> &[BlockId] {
        &self.entry_reachable
    }

    pub(crate) fn reachable(&self) -> &[BlockId] {
        &self.reachable
    }

    pub(crate) fn protected_but_entry_unreachable(&self) -> &[BlockId] {
        &self.protected_but_entry_unreachable
    }

    pub(crate) fn unreachable(&self) -> &[BlockId] {
        &self.unreachable
    }
}

/// Computes CFG facts directly from a borrowed dense definition.
pub(crate) fn local_cfg_facts_for_definition(
    definition: MirDefinitionRef<'_>,
) -> Result<MirLocalCfgFacts, MirRewriteError> {
    let callable = definition.callable();
    let mut blocks = Vec::with_capacity(definition.body().blocks.len());
    for (index, block) in definition.body().blocks.iter().enumerate() {
        validate_dense_declaration(callable, index, block.id)?;
        let terminator = block
            .terminator
            .as_ref()
            .ok_or(MirRewriteError::MissingBlockTerminator { block: block.id })?;
        let successors = terminator.successors().collect::<Vec<_>>();
        blocks.push((block.id, successors));
    }
    for (block, successors) in &blocks {
        for successor in successors {
            validate_dense_block_reference(
                definition,
                *successor,
                MirLocalIdentitySite::Terminator(block.index()),
            )?;
        }
    }

    let mut roots = RootCollector::default();
    observe_definition_local_identities(definition, &mut roots)?;
    let entry = roots.entry.ok_or(MirRewriteError::InvalidReference {
        expected: callable,
        identity: MirLocalIdentity::Block(definition.body().entry),
        site: MirLocalIdentitySite::BodyEntry,
        failure: MirReferenceFailure::Unknown,
    })?;
    for root in &roots.protected {
        validate_dense_block_reference(definition, root.block, root.site)?;
    }
    validate_dense_block_reference(definition, entry, MirLocalIdentitySite::BodyEntry)?;

    build_facts(
        callable,
        entry,
        roots.protected,
        blocks,
        value_use_census_for_definition(definition)?,
    )
}

impl MirCallableEdit {
    /// Computes CFG facts from the current sparse edit snapshot.
    pub(crate) fn local_cfg_facts(&self) -> Result<MirLocalCfgFacts, MirRewriteError> {
        let callable = self.callable();
        let mut seen = BTreeSet::new();
        let mut blocks = Vec::with_capacity(self.block_order().len());
        for block_id in self.block_order() {
            if !seen.insert(*block_id) {
                return Err(MirRewriteError::DuplicateOrderIdentity {
                    identity: MirLocalIdentity::Block(*block_id),
                });
            }
            let block = self.block(*block_id)?;
            let terminator = block
                .terminator
                .as_ref()
                .ok_or(MirRewriteError::MissingBlockTerminator { block: *block_id })?;
            blocks.push((*block_id, terminator.successors().collect::<Vec<_>>()));
        }
        for live in self.block_ids() {
            if !seen.contains(&live) {
                return Err(MirRewriteError::MissingOrderIdentity {
                    identity: MirLocalIdentity::Block(live),
                });
            }
        }
        for (block, successors) in &blocks {
            for successor in successors {
                validate_edit_block_reference(
                    self,
                    *successor,
                    MirLocalIdentitySite::Terminator(block.index()),
                )?;
            }
        }

        let mut roots = RootCollector::default();
        self.observe_cfg_roots(&mut roots)?;
        let entry = roots.entry.ok_or(MirRewriteError::InvalidReference {
            expected: callable,
            identity: MirLocalIdentity::Block(self.entry()),
            site: MirLocalIdentitySite::BodyEntry,
            failure: MirReferenceFailure::Unknown,
        })?;
        validate_edit_block_reference(self, entry, MirLocalIdentitySite::BodyEntry)?;
        for root in &roots.protected {
            validate_edit_block_reference(self, root.block, root.site)?;
        }

        build_facts(
            callable,
            entry,
            roots.protected,
            blocks,
            self.value_use_census()?,
        )
    }
}

fn build_facts(
    callable: CallableId,
    entry: BlockId,
    protected_roots: Vec<MirProtectedBlockRoot>,
    adjacency: Vec<(BlockId, Vec<BlockId>)>,
    census: MirValueUseCensus,
) -> Result<MirLocalCfgFacts, MirRewriteError> {
    let block_order = adjacency
        .iter()
        .map(|(block, _)| *block)
        .collect::<Vec<_>>();
    let adjacency_by_block = adjacency.iter().cloned().collect::<BTreeMap<_, _>>();
    let entry_closure = closure([entry], &adjacency_by_block);
    let all_closure = closure(
        std::iter::once(entry).chain(protected_roots.iter().map(|root| root.block)),
        &adjacency_by_block,
    );

    let mut definitions = BTreeMap::<BlockId, Vec<ValueId>>::new();
    for value in census.iter() {
        let Some(site) = value.definition() else {
            // Some intermediate and test MIR snapshots can retain an unused
            // declaration. This query inventories definitions; it does not
            // turn declaration liveness into a new verifier responsibility.
            continue;
        };
        let MirLocalIdentitySite::Instruction { block, .. } = site else {
            return Err(MirRewriteError::InvalidValueDefinitionSite {
                value: value.value(),
                site,
            });
        };
        let block = BlockId::new(callable, block);
        if !adjacency_by_block.contains_key(&block) {
            return Err(MirRewriteError::InvalidReference {
                expected: callable,
                identity: MirLocalIdentity::Block(block),
                site,
                failure: MirReferenceFailure::Unknown,
            });
        }
        definitions.entry(block).or_default().push(value.value());
    }

    let blocks = adjacency
        .into_iter()
        .map(|(block, successors)| MirLocalCfgBlockFacts {
            block,
            successors,
            defined_values: definitions.remove(&block).unwrap_or_default(),
        })
        .collect();
    let select = |members: &BTreeSet<BlockId>| {
        block_order
            .iter()
            .copied()
            .filter(|block| members.contains(block))
            .collect::<Vec<_>>()
    };
    let protected_only = all_closure
        .difference(&entry_closure)
        .copied()
        .collect::<BTreeSet<_>>();
    let unreachable_set = block_order
        .iter()
        .copied()
        .filter(|block| !all_closure.contains(block))
        .collect::<BTreeSet<_>>();

    Ok(MirLocalCfgFacts {
        callable,
        entry,
        protected_roots,
        blocks,
        entry_reachable: select(&entry_closure),
        reachable: select(&all_closure),
        protected_but_entry_unreachable: select(&protected_only),
        unreachable: select(&unreachable_set),
    })
}

fn closure(
    roots: impl IntoIterator<Item = BlockId>,
    adjacency: &BTreeMap<BlockId, Vec<BlockId>>,
) -> BTreeSet<BlockId> {
    let mut reachable = BTreeSet::new();
    let mut pending = VecDeque::from_iter(roots);
    while let Some(block) = pending.pop_front() {
        if !reachable.insert(block) {
            continue;
        }
        if let Some(successors) = adjacency.get(&block) {
            pending.extend(successors.iter().copied());
        }
    }
    reachable
}

#[derive(Default)]
struct RootCollector {
    entry: Option<BlockId>,
    protected: Vec<MirProtectedBlockRoot>,
}

impl MirLocalIdentityObserver for RootCollector {
    type Error = MirRewriteError;

    fn observe_block(
        &mut self,
        site: MirLocalIdentitySite,
        block: BlockId,
    ) -> Result<(), Self::Error> {
        match classify_local_identity_site(site) {
            MirIdentitySiteRole::BodyEntry => self.entry = Some(block),
            MirIdentitySiteRole::PermanentAttachment | MirIdentitySiteRole::ConsumableProof => {
                self.protected.push(MirProtectedBlockRoot { site, block });
            }
            MirIdentitySiteRole::Ordinary => {}
        }
        Ok(())
    }
}

fn validate_dense_declaration(
    callable: CallableId,
    index: usize,
    actual: BlockId,
) -> Result<(), MirRewriteError> {
    if actual.callable() != callable {
        return Err(MirRewriteError::ForeignIdentity {
            expected: callable,
            identity: MirLocalIdentity::Block(actual),
        });
    }
    let expected = BlockId::new(callable, index);
    if actual != expected {
        return Err(MirRewriteError::DeclarationIdentityMismatch {
            expected: MirLocalIdentity::Block(expected),
            actual: MirLocalIdentity::Block(actual),
        });
    }
    Ok(())
}

fn validate_dense_block_reference(
    definition: MirDefinitionRef<'_>,
    block: BlockId,
    site: MirLocalIdentitySite,
) -> Result<(), MirRewriteError> {
    let expected = definition.callable();
    if block.callable() != expected {
        return Err(invalid_block_reference(
            expected,
            block,
            site,
            MirReferenceFailure::Foreign,
        ));
    }
    if definition.block(block).is_none() {
        return Err(invalid_block_reference(
            expected,
            block,
            site,
            MirReferenceFailure::Unknown,
        ));
    }
    Ok(())
}

fn validate_edit_block_reference(
    edit: &MirCallableEdit,
    block: BlockId,
    site: MirLocalIdentitySite,
) -> Result<(), MirRewriteError> {
    match edit.block(block) {
        Ok(_) => Ok(()),
        Err(MirRewriteError::ForeignIdentity { .. }) => Err(invalid_block_reference(
            edit.callable(),
            block,
            site,
            MirReferenceFailure::Foreign,
        )),
        Err(MirRewriteError::UnknownIdentity { .. }) => Err(invalid_block_reference(
            edit.callable(),
            block,
            site,
            MirReferenceFailure::Unknown,
        )),
        Err(MirRewriteError::DeletedIdentity { .. }) => Err(invalid_block_reference(
            edit.callable(),
            block,
            site,
            MirReferenceFailure::Deleted,
        )),
        Err(error) => Err(error),
    }
}

fn invalid_block_reference(
    expected: CallableId,
    block: BlockId,
    site: MirLocalIdentitySite,
    failure: MirReferenceFailure,
) -> MirRewriteError {
    MirRewriteError::InvalidReference {
        expected,
        identity: MirLocalIdentity::Block(block),
        site,
        failure,
    }
}

#[cfg(test)]
mod tests;
