//! Immutable arbitrary-depth discovery of same-block integer cast chains.

use std::collections::BTreeMap;

use crate::{
    identity::CallableId,
    mir::{
        rewrite::{MirLocalIdentitySite, MirRewriteError},
        BlockId, MirDefinitionRef, MirInstruction, MirPrimitiveCast, MirRvalueKind, MirType,
        ValueId,
    },
    source::Span,
};

use super::{IntegerCastRecipe, IntegerCastTransform};

/// One ordinary primitive-cast assignment in deterministic body order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::passes) struct IntegerCastSite {
    callable: CallableId,
    block: usize,
    block_id: BlockId,
    instruction: usize,
    result: ValueId,
    operation: MirPrimitiveCast,
    operand: ValueId,
    result_type: MirType,
    span: Span,
    expected: MirInstruction,
}

impl IntegerCastSite {
    pub(in crate::passes) const fn callable(&self) -> CallableId {
        self.callable
    }

    pub(in crate::passes) const fn block(&self) -> usize {
        self.block
    }

    pub(in crate::passes) const fn block_id(&self) -> BlockId {
        self.block_id
    }

    pub(in crate::passes) const fn instruction(&self) -> usize {
        self.instruction
    }

    pub(in crate::passes) const fn result(&self) -> ValueId {
        self.result
    }

    pub(in crate::passes) const fn operation(&self) -> MirPrimitiveCast {
        self.operation
    }

    pub(in crate::passes) const fn operand(&self) -> ValueId {
        self.operand
    }

    pub(in crate::passes) const fn result_type(&self) -> MirType {
        self.result_type
    }

    pub(in crate::passes) const fn span(&self) -> Span {
        self.span
    }

    pub(in crate::passes) fn expected(&self) -> &MirInstruction {
        &self.expected
    }

    pub(in crate::passes) const fn definition_site(&self) -> MirLocalIdentitySite {
        MirLocalIdentitySite::Instruction {
            block: self.block,
            instruction: self.instruction,
        }
    }
}

/// Invalid site properties which cannot occur in verified MIR.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::passes) enum IntegerCastSiteInvalidity {
    MalformedIdentity,
    UnsupportedTypeOrOperation,
}

/// Why discovery stopped at another cast definition rather than a non-cast
/// root. Valid boundaries still permit canonicalization of the same-block
/// integer suffix which precedes them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::passes) enum IntegerCastChainBoundary {
    ControlFlow,
    UnsupportedCastFamily,
    InvalidPredecessor,
    NonPrecedingDefinition,
    RepeatedIdentity,
}

/// One valid same-block integer-only cast chain ending at a selected site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::passes) struct IntegerCastChain {
    root: ValueId,
    sites: Vec<IntegerCastSite>,
    transform: IntegerCastTransform,
    recipe: IntegerCastRecipe,
    boundary: Option<IntegerCastChainBoundary>,
}

impl IntegerCastChain {
    fn start(site: IntegerCastSite, boundary: Option<IntegerCastChainBoundary>) -> Self {
        let transform = IntegerCastTransform::from_operation(site.operation)
            .expect("a validated integer cast starts an integer transform");
        let recipe = transform.canonical_recipe();
        Self {
            root: site.operand,
            sites: vec![site],
            transform,
            recipe,
            boundary,
        }
    }

    fn extend(&self, site: IntegerCastSite) -> Option<Self> {
        if self.root == site.result || self.sites.iter().any(|known| known.result == site.result) {
            return None;
        }
        let transform = self.transform.then(site.operation)?;
        let recipe = transform.canonical_recipe();
        let mut sites = self.sites.clone();
        sites.push(site);
        Some(Self {
            root: self.root,
            sites,
            transform,
            recipe,
            boundary: self.boundary,
        })
    }

    pub(in crate::passes) const fn root(&self) -> ValueId {
        self.root
    }

    pub(in crate::passes) fn sites(&self) -> &[IntegerCastSite] {
        &self.sites
    }

    pub(in crate::passes) fn endpoint(&self) -> &IntegerCastSite {
        self.sites
            .last()
            .expect("an integer cast chain contains its endpoint")
    }

    pub(in crate::passes) const fn recipe(&self) -> IntegerCastRecipe {
        self.recipe
    }

    pub(in crate::passes) const fn boundary(&self) -> Option<IntegerCastChainBoundary> {
        self.boundary
    }

    pub(in crate::passes) fn original_length(&self) -> usize {
        self.sites.len()
    }

    pub(in crate::passes) const fn canonical_length(&self) -> usize {
        self.recipe.length()
    }

    pub(in crate::passes) fn eliminated_steps(&self) -> usize {
        self.original_length()
            .saturating_sub(self.canonical_length())
    }

    pub(in crate::passes) fn is_candidate(&self) -> bool {
        self.eliminated_steps() != 0
    }

    /// Existing result of the first information-losing `64-bit -> u8` cast.
    /// It is present exactly when the canonical recipe needs two casts.
    pub(in crate::passes) fn reusable_narrowing(&self) -> Option<ValueId> {
        self.reusable_narrowing_site().map(IntegerCastSite::result)
    }

    /// Exact existing assignment which owns the reusable low-byte value.
    pub(in crate::passes) fn reusable_narrowing_site(&self) -> Option<&IntegerCastSite> {
        if !matches!(self.recipe, IntegerCastRecipe::NarrowThenWiden { .. }) {
            return None;
        }
        self.sites.iter().find(|site| {
            site.operation.target == crate::mir::MirPrimitiveType::U8
                && site.operation.source != crate::mir::MirPrimitiveType::U8
        })
    }
}

/// Analysis of one ordinary primitive-cast assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::passes) struct IntegerCastChainEntry {
    site: IntegerCastSite,
    invalidities: Vec<IntegerCastSiteInvalidity>,
    chain: Option<IntegerCastChain>,
    boundary: Option<IntegerCastChainBoundary>,
}

impl IntegerCastChainEntry {
    pub(in crate::passes) const fn site(&self) -> &IntegerCastSite {
        &self.site
    }

    pub(in crate::passes) fn invalidities(&self) -> &[IntegerCastSiteInvalidity] {
        &self.invalidities
    }

    pub(in crate::passes) const fn chain(&self) -> Option<&IntegerCastChain> {
        self.chain.as_ref()
    }

    pub(in crate::passes) const fn boundary(&self) -> Option<IntegerCastChainBoundary> {
        self.boundary
    }
}

/// Deterministically ordered cast sites and integer-chain summaries for one
/// immutable callable snapshot.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::passes) struct IntegerCastChainAnalysis {
    entries: Vec<IntegerCastChainEntry>,
    by_result: BTreeMap<ValueId, usize>,
}

impl IntegerCastChainAnalysis {
    pub(in crate::passes) fn entries(&self) -> &[IntegerCastChainEntry] {
        &self.entries
    }

    pub(in crate::passes) fn entry_for_result(
        &self,
        result: ValueId,
    ) -> Option<&IntegerCastChainEntry> {
        self.by_result
            .get(&result)
            .and_then(|index| self.entries.get(*index))
    }

    /// Shorter canonical recipes in the same deterministic order as their
    /// endpoint instructions. Boundary-bearing candidates remain visible so
    /// a consumer can report why it cannot rewrite them.
    pub(in crate::passes) fn candidates(&self) -> impl Iterator<Item = &IntegerCastChain> {
        self.entries
            .iter()
            .filter_map(IntegerCastChainEntry::chain)
            .filter(|chain| chain.is_candidate())
    }
}

/// Builds arbitrary-depth same-block chain summaries without cloning or
/// mutating the definition itself.
pub(in crate::passes) fn analyze_integer_cast_chains(
    definition: MirDefinitionRef<'_>,
) -> Result<IntegerCastChainAnalysis, MirRewriteError> {
    let sites = cast_sites(definition);
    let mut indexed_sites = BTreeMap::new();
    for (index, site) in sites.iter().enumerate() {
        if let Some(first_index) = indexed_sites.insert(site.result, index) {
            return Err(MirRewriteError::DuplicateValueDefinition {
                value: site.result,
                first: sites[first_index].definition_site(),
                duplicate: site.definition_site(),
            });
        }
    }

    let mut analysis = IntegerCastChainAnalysis::default();
    for site in sites.iter().cloned() {
        let invalidities = site_invalidities(definition, &site);
        let is_supported_integer = site.operation.source.is_integer()
            && site.operation.target.is_integer()
            && invalidities.is_empty();
        let (chain, boundary) = if !is_supported_integer {
            (None, None)
        } else {
            analyze_integer_site(&analysis, &sites, &indexed_sites, &site)
        };
        let entry_index = analysis.entries.len();
        analysis.by_result.insert(site.result, entry_index);
        analysis.entries.push(IntegerCastChainEntry {
            site,
            invalidities,
            chain,
            boundary,
        });
    }
    Ok(analysis)
}

fn analyze_integer_site(
    analysis: &IntegerCastChainAnalysis,
    sites: &[IntegerCastSite],
    indexed_sites: &BTreeMap<ValueId, usize>,
    site: &IntegerCastSite,
) -> (Option<IntegerCastChain>, Option<IntegerCastChainBoundary>) {
    let Some(predecessor_index) = indexed_sites.get(&site.operand).copied() else {
        return (Some(IntegerCastChain::start(site.clone(), None)), None);
    };
    let Some(predecessor) = analysis.entries.get(predecessor_index) else {
        let boundary = if reaches_result(indexed_sites, sites, site.operand, site.result) {
            IntegerCastChainBoundary::RepeatedIdentity
        } else {
            IntegerCastChainBoundary::NonPrecedingDefinition
        };
        return (None, Some(boundary));
    };

    if predecessor.site.block != site.block {
        let boundary = IntegerCastChainBoundary::ControlFlow;
        return (
            Some(IntegerCastChain::start(site.clone(), Some(boundary))),
            Some(boundary),
        );
    }
    if predecessor.site.instruction >= site.instruction {
        return (None, Some(IntegerCastChainBoundary::NonPrecedingDefinition));
    }
    if !predecessor.invalidities.is_empty() {
        return (None, Some(IntegerCastChainBoundary::InvalidPredecessor));
    }
    let Some(predecessor_chain) = &predecessor.chain else {
        if predecessor.site.operand == site.result
            || matches!(
                predecessor.boundary,
                Some(IntegerCastChainBoundary::RepeatedIdentity)
            )
        {
            return (None, Some(IntegerCastChainBoundary::RepeatedIdentity));
        }
        if predecessor.boundary.is_some() {
            return (None, Some(IntegerCastChainBoundary::InvalidPredecessor));
        }
        let boundary = IntegerCastChainBoundary::UnsupportedCastFamily;
        return (
            Some(IntegerCastChain::start(site.clone(), Some(boundary))),
            Some(boundary),
        );
    };
    match predecessor_chain.extend(site.clone()) {
        Some(chain) => {
            let boundary = chain.boundary;
            (Some(chain), boundary)
        }
        None => (None, Some(IntegerCastChainBoundary::RepeatedIdentity)),
    }
}

fn reaches_result(
    indexed_sites: &BTreeMap<ValueId, usize>,
    sites: &[IntegerCastSite],
    mut value: ValueId,
    selected: ValueId,
) -> bool {
    let mut steps = 0usize;
    while let Some(index) = indexed_sites.get(&value).copied() {
        if value == selected {
            return true;
        }
        value = sites[index].operand;
        steps = steps.saturating_add(1);
        if value == selected {
            return true;
        }
        if steps > indexed_sites.len() {
            return true;
        }
    }
    false
}

fn cast_sites(definition: MirDefinitionRef<'_>) -> Vec<IntegerCastSite> {
    definition
        .body()
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(block, body)| {
            body.instructions
                .iter()
                .enumerate()
                .filter_map(move |(instruction, item)| {
                    let MirInstruction::Assign(assignment) = item else {
                        return None;
                    };
                    let MirRvalueKind::PrimitiveCast { operation, operand } =
                        assignment.rvalue.kind
                    else {
                        return None;
                    };
                    Some(IntegerCastSite {
                        callable: definition.callable(),
                        block,
                        block_id: body.id,
                        instruction,
                        result: assignment.result,
                        operation,
                        operand,
                        result_type: assignment.rvalue.ty,
                        span: assignment.span,
                        expected: item.clone(),
                    })
                })
        })
        .collect()
}

fn site_invalidities(
    definition: MirDefinitionRef<'_>,
    site: &IntegerCastSite,
) -> Vec<IntegerCastSiteInvalidity> {
    let mut invalidities = Vec::new();
    if site.result.callable() != definition.callable()
        || site.operand.callable() != definition.callable()
        || definition.value(site.result).is_none()
        || definition.value(site.operand).is_none()
    {
        invalidities.push(IntegerCastSiteInvalidity::MalformedIdentity);
    }
    if !site.operation.is_semantically_consistent()
        || definition.value(site.operand).map(|value| value.ty)
            != Some(site.operation.source_type())
        || definition.value(site.result).map(|value| value.ty) != Some(site.operation.result_type())
        || site.result_type != site.operation.result_type()
    {
        invalidities.push(IntegerCastSiteInvalidity::UnsupportedTypeOrOperation);
    }
    invalidities
}

#[cfg(test)]
#[path = "chain/tests.rs"]
mod tests;
