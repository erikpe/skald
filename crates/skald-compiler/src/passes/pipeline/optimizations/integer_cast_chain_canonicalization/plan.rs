//! Immutable guarded plans for integer cast-chain canonicalization.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    identity::CallableId,
    mir::{
        rewrite::{
            value_use_site_index_for_definition, MirCallableEdit, MirCallableEditSnapshot,
            MirLocalIdentitySite, MirRewriteError, MirValueUseSiteIndex, MirValueUseSites,
        },
        MirAssignment, MirDefinitionRef, MirInstruction, MirPrimitiveCast, MirProgram, MirRvalue,
        MirRvalueKind, MirType, ValueId,
    },
    passes::integer_cast::{
        analyze_integer_cast_chains, IntegerCastChain, IntegerCastChainBoundary, IntegerCastRecipe,
        IntegerCastSite,
    },
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct IntegerCastCanonicalizationCounts {
    pub(super) retargeted_endpoints: usize,
    pub(super) forwarded_endpoints: usize,
    pub(super) eliminated_steps: usize,
    pub(super) protected_rejections: usize,
    pub(super) maximum_depth: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum EndpointReplacement {
    Direct(MirPrimitiveCast),
    NarrowThenWiden {
        narrowing: Box<IntegerCastSite>,
        narrow: MirPrimitiveCast,
        widen: MirPrimitiveCast,
    },
    Forward {
        expected_uses: MirValueUseSites,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EndpointPlan {
    endpoint: IntegerCastSite,
    root: ValueId,
    root_definition: MirLocalIdentitySite,
    replacement: EndpointReplacement,
    original_depth: usize,
    eliminated_steps: usize,
}

impl EndpointPlan {
    fn prepare(
        definition: MirDefinitionRef<'_>,
        chain: &IntegerCastChain,
        value_uses: &MirValueUseSiteIndex,
    ) -> Result<Option<Self>, MirRewriteError> {
        if matches!(
            chain.boundary(),
            Some(IntegerCastChainBoundary::ControlFlow)
        ) {
            return Ok(None);
        }

        let endpoint = chain.endpoint().clone();
        let root = chain.root();
        let root_definition = indexed_uses(
            value_uses,
            root,
            definition.callable(),
            "integer cast-chain root uses",
        )?
        .definition();
        if !definition_precedes(root_definition, endpoint.definition_site()) {
            return Ok(None);
        }

        let replacement = match chain.recipe() {
            IntegerCastRecipe::Identity => {
                let expected_uses = indexed_uses(
                    value_uses,
                    endpoint.result(),
                    definition.callable(),
                    "integer cast-chain endpoint uses",
                )?
                .clone();
                if definition.value(root).map(|value| value.ty) != Some(endpoint.result_type())
                    || !expected_uses.is_forwarding_safe()
                {
                    return Ok(None);
                }
                EndpointReplacement::Forward { expected_uses }
            }
            IntegerCastRecipe::Direct(operation) => EndpointReplacement::Direct(operation),
            IntegerCastRecipe::NarrowThenWiden { narrow, widen } => {
                let Some(reusable) = chain.reusable_narrowing() else {
                    return Err(stale(
                        definition.callable(),
                        "integer cast-chain reusable narrowing value",
                    ));
                };
                let Some(narrowing) = chain.reusable_narrowing_site().cloned() else {
                    return Err(stale(
                        definition.callable(),
                        "integer cast-chain reusable narrowing",
                    ));
                };
                if narrowing.result() != reusable
                    || narrowing.operation().result_type() != MirType::U8
                    || !definition_precedes(root_definition, narrowing.definition_site())
                    || !definition_precedes(narrowing.definition_site(), endpoint.definition_site())
                {
                    return Err(stale(
                        definition.callable(),
                        "integer cast-chain narrowing order",
                    ));
                }
                EndpointReplacement::NarrowThenWiden {
                    narrowing: Box::new(narrowing),
                    narrow,
                    widen,
                }
            }
        };
        Ok(Some(Self {
            endpoint,
            root,
            root_definition,
            replacement,
            original_depth: chain.original_length(),
            eliminated_steps: chain.eliminated_steps(),
        }))
    }

    fn validate(
        &self,
        edit: &MirCallableEdit,
        value_uses: &MirValueUseSiteIndex,
    ) -> Result<(), MirRewriteError> {
        validate_site(edit, &self.endpoint)?;
        let root_type = match self.replacement {
            EndpointReplacement::Direct(operation) => operation.source_type(),
            EndpointReplacement::NarrowThenWiden { narrow, .. } => narrow.source_type(),
            EndpointReplacement::Forward { .. } => self.endpoint.result_type(),
        };
        require_value_type(edit, self.root, root_type, "integer cast-chain root type")?;
        require_value_type(
            edit,
            self.endpoint.result(),
            self.endpoint.result_type(),
            "integer cast-chain endpoint type",
        )?;
        let root_sites = indexed_uses(
            value_uses,
            self.root,
            edit.callable(),
            "integer cast-chain root uses",
        )?;
        if root_sites.definition() != self.root_definition
            || !definition_precedes(self.root_definition, self.endpoint.definition_site())
        {
            return Err(stale(edit.callable(), "integer cast-chain root definition"));
        }

        match &self.replacement {
            EndpointReplacement::Direct(operation) => {
                if operation.result_type() != self.endpoint.result_type() {
                    return Err(stale(edit.callable(), "integer cast-chain direct recipe"));
                }
            }
            EndpointReplacement::NarrowThenWiden {
                narrowing,
                narrow,
                widen,
            } => {
                validate_site(edit, narrowing)?;
                require_value_type(
                    edit,
                    narrowing.result(),
                    MirType::U8,
                    "integer cast-chain narrowing type",
                )?;
                if narrow.source_type() != root_type
                    || narrow.result_type() != MirType::U8
                    || narrowing.operation().result_type() != MirType::U8
                    || widen.source_type() != MirType::U8
                    || widen.result_type() != self.endpoint.result_type()
                    || !definition_precedes(self.root_definition, narrowing.definition_site())
                    || !definition_precedes(
                        narrowing.definition_site(),
                        self.endpoint.definition_site(),
                    )
                {
                    return Err(stale(edit.callable(), "integer cast-chain widening recipe"));
                }
            }
            EndpointReplacement::Forward { expected_uses } => {
                let actual_uses = indexed_uses(
                    value_uses,
                    self.endpoint.result(),
                    edit.callable(),
                    "integer cast-chain endpoint uses",
                )?;
                if actual_uses != expected_uses || !actual_uses.is_forwarding_safe() {
                    return Err(stale(edit.callable(), "integer cast-chain forwarding uses"));
                }
            }
        }
        Ok(())
    }

    fn replacement_instruction(&self) -> Option<MirInstruction> {
        let (operation, operand) = match &self.replacement {
            EndpointReplacement::Direct(operation) => (*operation, self.root),
            EndpointReplacement::NarrowThenWiden {
                narrowing, widen, ..
            } => (*widen, narrowing.result()),
            EndpointReplacement::Forward { .. } => return None,
        };
        Some(MirInstruction::Assign(MirAssignment {
            result: self.endpoint.result(),
            rvalue: MirRvalue {
                kind: MirRvalueKind::PrimitiveCast { operation, operand },
                ty: self.endpoint.result_type(),
            },
            span: self.endpoint.span(),
        }))
    }

    const fn is_forwarding(&self) -> bool {
        matches!(self.replacement, EndpointReplacement::Forward { .. })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CallablePlan {
    snapshot: MirCallableEditSnapshot,
    endpoints: Vec<EndpointPlan>,
}

/// All safe shortenings selected from one immutable verified program.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct IntegerCastCanonicalizationPlan {
    callables: BTreeMap<CallableId, CallablePlan>,
    processed_callables: usize,
    counts: IntegerCastCanonicalizationCounts,
}

impl IntegerCastCanonicalizationPlan {
    pub(super) fn prepare(program: &MirProgram) -> Result<Self, MirRewriteError> {
        let mut plan = Self::default();
        for definition in program.executable_definitions() {
            plan.processed_callables = plan.processed_callables.saturating_add(1);
            let analysis = analyze_integer_cast_chains(definition)?;
            let value_uses = value_use_site_index_for_definition(definition)?;
            let mut endpoints = Vec::new();
            for chain in analysis.candidates() {
                let Some(endpoint) = EndpointPlan::prepare(definition, chain, &value_uses)? else {
                    plan.counts.protected_rejections =
                        plan.counts.protected_rejections.saturating_add(1);
                    continue;
                };
                plan.counts.retargeted_endpoints = plan
                    .counts
                    .retargeted_endpoints
                    .saturating_add(usize::from(!endpoint.is_forwarding()));
                plan.counts.forwarded_endpoints = plan
                    .counts
                    .forwarded_endpoints
                    .saturating_add(usize::from(endpoint.is_forwarding()));
                plan.counts.eliminated_steps = plan
                    .counts
                    .eliminated_steps
                    .saturating_add(endpoint.eliminated_steps);
                plan.counts.maximum_depth = plan.counts.maximum_depth.max(endpoint.original_depth);
                endpoints.push(endpoint);
            }
            if !endpoints.is_empty() {
                plan.callables.insert(
                    definition.callable(),
                    CallablePlan {
                        snapshot: MirCallableEditSnapshot::capture(definition),
                        endpoints,
                    },
                );
            }
        }
        Ok(plan)
    }

    pub(super) fn validate_program(&self, program: &MirProgram) -> Result<(), MirRewriteError> {
        let mut validated = BTreeSet::new();
        for definition in program.executable_definitions() {
            if let Some(plan) = self.callables.get(&definition.callable()) {
                plan.snapshot
                    .validate_definition(definition, "integer cast-chain plan")?;
                validated.insert(definition.callable());
            }
        }
        if let Some(missing) = self
            .callables
            .keys()
            .find(|callable| !validated.contains(callable))
        {
            return Err(stale(*missing, "integer cast-chain callable"));
        }
        Ok(())
    }

    /// Validates the whole callable plan before applying its first edit.
    pub(super) fn rewrite_callable(
        &self,
        callable: CallableId,
        edit: &mut MirCallableEdit,
    ) -> Result<usize, MirRewriteError> {
        let Some(plan) = self.callables.get(&callable) else {
            return Ok(0);
        };
        plan.snapshot
            .validate(edit, "integer cast-chain canonicalization plan")?;
        let value_uses = edit.value_use_site_index()?;
        for endpoint in &plan.endpoints {
            endpoint.validate(edit, &value_uses)?;
        }

        // Retargeting preserves instruction positions. Perform every such edit
        // before deletion so all analyzed positions still name the snapshot.
        for endpoint in &plan.endpoints {
            let Some(replacement) = endpoint.replacement_instruction() else {
                continue;
            };
            edit.replace_instruction(
                endpoint.endpoint.block_id(),
                endpoint.endpoint.instruction(),
                endpoint.endpoint.expected(),
                replacement,
            )?;
        }

        // Remove identity definitions in one block-local batch before mapping
        // their remaining uses. Uses inside another removed endpoint are not
        // observable work and therefore do not inflate forwarding metrics.
        let forwarding = plan
            .endpoints
            .iter()
            .filter(|plan| plan.is_forwarding())
            .map(|endpoint| (endpoint.endpoint.result(), endpoint.root))
            .collect::<BTreeMap<_, _>>();
        if forwarding
            .values()
            .any(|target| forwarding.contains_key(target))
        {
            return Err(stale(
                callable,
                "integer cast-chain retained forwarding target",
            ));
        }
        remove_assignments(
            edit,
            plan.endpoints
                .iter()
                .filter(|endpoint| endpoint.is_forwarding())
                .map(|endpoint| &endpoint.endpoint),
        )?;
        let forwarded_uses = edit.replace_value_uses_many(&forwarding)?;
        for endpoint in plan.endpoints.iter().filter(|plan| plan.is_forwarding()) {
            edit.remove_value(endpoint.endpoint.result())?;
        }
        Ok(forwarded_uses)
    }

    pub(super) const fn processed_callables(&self) -> usize {
        self.processed_callables
    }

    pub(super) fn changed_callables(&self) -> usize {
        self.callables.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.callables.is_empty()
    }

    pub(super) const fn counts(&self) -> IntegerCastCanonicalizationCounts {
        self.counts
    }
}

fn validate_site(edit: &MirCallableEdit, site: &IntegerCastSite) -> Result<(), MirRewriteError> {
    if site.callable() != edit.callable() {
        return Err(stale(edit.callable(), "integer cast-chain callable"));
    }
    let Some(actual) = edit
        .block(site.block_id())?
        .instructions
        .get(site.instruction())
    else {
        return Err(stale(
            edit.callable(),
            "integer cast-chain instruction position",
        ));
    };
    if actual != site.expected() {
        return Err(stale(edit.callable(), "integer cast-chain instruction"));
    }
    let MirInstruction::Assign(assignment) = actual else {
        return Err(stale(edit.callable(), "integer cast-chain assignment"));
    };
    let MirRvalueKind::PrimitiveCast { operation, operand } = assignment.rvalue.kind else {
        return Err(stale(edit.callable(), "integer cast-chain operation"));
    };
    if assignment.result != site.result()
        || assignment.rvalue.ty != site.result_type()
        || assignment.span != site.span()
        || operation != site.operation()
        || operand != site.operand()
    {
        return Err(stale(edit.callable(), "integer cast-chain site fields"));
    }
    Ok(())
}

fn require_value_type(
    edit: &MirCallableEdit,
    value: ValueId,
    expected: MirType,
    subject: &'static str,
) -> Result<(), MirRewriteError> {
    if edit.value(value)?.ty != expected {
        return Err(stale(edit.callable(), subject));
    }
    Ok(())
}

const fn definition_precedes(
    definition: MirLocalIdentitySite,
    use_site: MirLocalIdentitySite,
) -> bool {
    match (definition, use_site) {
        (
            MirLocalIdentitySite::Instruction {
                block: definition_block,
                instruction: definition_instruction,
            },
            MirLocalIdentitySite::Instruction {
                block: use_block,
                instruction: use_instruction,
            },
        ) => definition_block == use_block && definition_instruction < use_instruction,
        _ => false,
    }
}

fn indexed_uses<'a>(
    index: &'a MirValueUseSiteIndex,
    value: ValueId,
    callable: CallableId,
    subject: &'static str,
) -> Result<&'a MirValueUseSites, MirRewriteError> {
    index.get(value).ok_or_else(|| stale(callable, subject))
}

fn remove_assignments<'a>(
    edit: &mut MirCallableEdit,
    endpoints: impl IntoIterator<Item = &'a IntegerCastSite>,
) -> Result<(), MirRewriteError> {
    let mut removals = BTreeMap::<_, BTreeSet<_>>::new();
    for endpoint in endpoints {
        removals
            .entry(endpoint.block_id())
            .or_default()
            .insert(endpoint.result());
    }
    for (block, results) in removals {
        edit.rewrite_block_instructions(block, |instructions| {
            instructions
                .iter()
                .filter(|instruction| {
                    !matches!(instruction, MirInstruction::Assign(assignment) if results.contains(&assignment.result))
                })
                .cloned()
                .collect()
        })?;
    }
    Ok(())
}

const fn stale(callable: CallableId, subject: &'static str) -> MirRewriteError {
    MirRewriteError::StaleCallableSnapshot { callable, subject }
}
