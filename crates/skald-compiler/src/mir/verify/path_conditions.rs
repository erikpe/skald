//! Structural and path-sensitive verification for MIR path conditions.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::mir::{
    BlockId, MirDefinitionRef, MirInstruction, MirPathCondition, MirPlaceBase, MirRvalueKind,
    MirStorageKind, MirTerminator, PathConditionId, StorageId, ValueId,
};

use super::{
    context::Verifier,
    dataflow::ForwardDataflow,
    path_state::{condition_reads, PathEdgeError, PathStates},
};

impl Verifier<'_> {
    pub(super) fn verify_path_conditions(&mut self, function: MirDefinitionRef<'_>) {
        self.verify_path_condition_declarations(function);
        self.verify_path_condition_flow(function);
    }

    fn verify_path_condition_declarations(&mut self, function: MirDefinitionRef<'_>) {
        let mut ids = HashSet::new();
        let mut activations = HashSet::new();
        let mut merges = HashSet::new();
        let predecessors = predecessors(function);

        for (index, condition) in function.path_conditions().iter().enumerate() {
            self.verify_path_condition_identity(function, condition, index, &mut ids);

            if !activations.insert(condition.activation) {
                self.function_error(
                    function.callable(),
                    format!(
                        "path-condition activation storage {} is declared more than once",
                        condition.activation
                    ),
                );
            }
            match function.storage(condition.activation) {
                Some(storage)
                    if storage.kind == MirStorageKind::PathCondition
                        && storage.ty == crate::mir::MirType::Bool => {}
                Some(_) => self.function_error(
                    function.callable(),
                    format!(
                        "path condition {} requires matching `bool` path-condition storage",
                        condition.id
                    ),
                ),
                None => self.function_error(
                    function.callable(),
                    format!(
                        "path condition {} references undeclared activation storage {}",
                        condition.id, condition.activation
                    ),
                ),
            }

            if !merges.insert(condition.merge) {
                self.function_error(
                    function.callable(),
                    format!(
                        "control-flow merge {} is shared by multiple path conditions",
                        condition.merge
                    ),
                );
            }
            self.verify_path_condition_blocks(function, condition, &predecessors);
            self.verify_path_condition_selection(function, condition, true);
            self.verify_path_condition_selection(function, condition, false);
        }

        for storage in function
            .storage_entries()
            .iter()
            .filter(|storage| storage.kind == MirStorageKind::PathCondition)
        {
            if !activations.contains(&storage.id) {
                self.function_error(
                    function.callable(),
                    format!(
                        "path-condition storage {} has no path condition declaration",
                        storage.id
                    ),
                );
            }
        }
        self.verify_activation_store_ownership(function);
    }

    fn verify_path_condition_identity(
        &mut self,
        function: MirDefinitionRef<'_>,
        condition: &MirPathCondition,
        index: usize,
        ids: &mut HashSet<PathConditionId>,
    ) {
        let expected = PathConditionId::new(function.callable(), index);
        if condition.id != expected {
            self.function_error(
                function.callable(),
                format!(
                    "path-condition table index {index} contains {}, expected {expected}",
                    condition.id,
                ),
            );
        }
        if !ids.insert(condition.id) {
            self.function_error(function.callable(), "duplicate path-condition ID");
        }
        if let Some(parent) = condition.parent {
            if parent.callable() != function.callable()
                || parent.index() >= condition.id.index()
                || function.path_condition(parent).is_none()
            {
                self.function_error(
                    function.callable(),
                    format!(
                        "path condition {} has an invalid or non-preceding parent {parent}",
                        condition.id
                    ),
                );
            }
        }
    }

    fn verify_path_condition_blocks(
        &mut self,
        function: MirDefinitionRef<'_>,
        condition: &MirPathCondition,
        predecessors: &HashMap<BlockId, BTreeSet<BlockId>>,
    ) {
        if condition.active_predecessor == condition.inactive_predecessor {
            self.function_error(
                function.callable(),
                format!(
                    "path condition {} must have distinct active and inactive predecessors",
                    condition.id
                ),
            );
        }
        if condition.merge == condition.active_predecessor
            || condition.merge == condition.inactive_predecessor
        {
            self.function_error(
                function.callable(),
                format!(
                    "path condition {} cannot merge in one of its selection predecessors",
                    condition.id
                ),
            );
        }

        for predecessor in [condition.active_predecessor, condition.inactive_predecessor] {
            match function.block(predecessor).and_then(|block| block.terminator.as_ref()) {
                Some(MirTerminator::Goto { target, .. }) if *target == condition.merge => {}
                Some(_) => self.function_error(
                    function.callable(),
                    format!(
                        "path condition {} predecessor {predecessor} must jump directly to {}",
                        condition.id, condition.merge
                    ),
                ),
                None => self.function_error(
                    function.callable(),
                    format!(
                        "path condition {} references missing or unterminated predecessor {predecessor}",
                        condition.id
                    ),
                ),
            }
        }
        if function.block(condition.merge).is_none() {
            self.function_error(
                function.callable(),
                format!(
                    "path condition {} references undeclared merge block {}",
                    condition.id, condition.merge
                ),
            );
        }

        let expected =
            BTreeSet::from([condition.active_predecessor, condition.inactive_predecessor]);
        if predecessors.get(&condition.merge) != Some(&expected) {
            self.function_error(
                function.callable(),
                format!(
                    "path condition {} merge {} must have exactly its active and inactive predecessors",
                    condition.id, condition.merge
                ),
            );
        }
    }

    fn verify_path_condition_selection(
        &mut self,
        function: MirDefinitionRef<'_>,
        condition: &MirPathCondition,
        active: bool,
    ) {
        let predecessor = if active {
            condition.active_predecessor
        } else {
            condition.inactive_predecessor
        };
        let Some(block) = function.block(predecessor) else {
            return;
        };
        let Some(MirInstruction::Store(store)) = block.instructions.last() else {
            self.block_error(
                function.callable(),
                predecessor,
                format!(
                    "path condition {} predecessor must end by storing its selection",
                    condition.id
                ),
            );
            return;
        };
        if store.destination.base != MirPlaceBase::Storage(condition.activation)
            || !store.destination.projections.is_empty()
        {
            self.block_error(
                function.callable(),
                predecessor,
                format!(
                    "path condition {} predecessor stores the wrong activation destination",
                    condition.id
                ),
            );
            return;
        }
        if constant_bool_in_block(block, store.value) != Some(active) {
            self.block_error(
                function.callable(),
                predecessor,
                format!(
                    "path condition {} predecessor must store canonical `{active}`",
                    condition.id
                ),
            );
        }
    }

    fn verify_activation_store_ownership(&mut self, function: MirDefinitionRef<'_>) {
        let owners: HashMap<StorageId, &MirPathCondition> = function
            .path_conditions()
            .iter()
            .map(|condition| (condition.activation, condition))
            .collect();
        let mut store_counts = HashMap::new();
        for block in &function.body().blocks {
            for instruction in &block.instructions {
                let MirInstruction::Store(store) = instruction else {
                    continue;
                };
                let MirPlaceBase::Storage(storage) = store.destination.base else {
                    continue;
                };
                let Some(condition) = owners.get(&storage) else {
                    continue;
                };
                *store_counts.entry((storage, block.id)).or_insert(0_usize) += 1;
                if !store.destination.projections.is_empty()
                    || (block.id != condition.active_predecessor
                        && block.id != condition.inactive_predecessor)
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!(
                            "path-condition activation {} may be written only by its declared predecessors",
                            condition.id
                        ),
                    );
                }
            }
        }
        for condition in function.path_conditions() {
            for predecessor in [condition.active_predecessor, condition.inactive_predecessor] {
                if store_counts
                    .get(&(condition.activation, predecessor))
                    .copied()
                    .unwrap_or(0)
                    != 1
                {
                    self.block_error(
                        function.callable(),
                        predecessor,
                        format!(
                            "path condition {} predecessor must write its activation exactly once",
                            condition.id
                        ),
                    );
                }
            }
        }
    }

    fn verify_path_condition_flow(&mut self, function: MirDefinitionRef<'_>) {
        let reads = condition_reads(function);
        let activation_conditions: HashMap<_, _> = function
            .path_conditions()
            .iter()
            .map(|condition| (condition.activation, condition.id))
            .collect();
        let mut flow = ForwardDataflow::new(function.callable(), function.body().blocks.len());
        flow.seed(function.body().entry, PathStates::initial(()));
        let mut reported_edges = HashSet::new();
        let mut reported_reads = HashSet::new();
        let mut reported_ends = HashSet::new();

        loop {
            while let Some((block_id, mut state)) = flow.pop() {
                let Some(block) = function.block(block_id) else {
                    continue;
                };
                for instruction in &block.instructions {
                    match instruction {
                        MirInstruction::Assign(assignment) => {
                            if let MirRvalueKind::PathCondition(condition) = assignment.rvalue.kind
                            {
                                if !state.all_select(condition.condition)
                                    && reported_reads.insert((block.id, condition.condition))
                                {
                                    self.block_error(
                                        function.callable(),
                                        block.id,
                                        format!(
                                            "path condition {} is read before selection or outside its active parent path",
                                            condition.condition
                                        ),
                                    );
                                }
                            }
                        }
                        MirInstruction::StorageDead(operation)
                            if activation_conditions.contains_key(&operation.storage) =>
                        {
                            let condition = activation_conditions[&operation.storage];
                            for child in function
                                .path_conditions()
                                .iter()
                                .filter(|candidate| candidate.parent == Some(condition))
                            {
                                if state.any_select(child.id)
                                    && reported_ends.insert((block.id, condition))
                                {
                                    self.block_error(
                                        function.callable(),
                                        block.id,
                                        format!(
                                            "path condition {condition} ends while child {} remains selected",
                                            child.id
                                        ),
                                    );
                                }
                            }
                            if state.end_condition(condition, |_, _| {})
                                && reported_ends.insert((block.id, condition))
                            {
                                self.block_error(
                                    function.callable(),
                                    block.id,
                                    format!(
                                        "path condition {condition} ends outside its selected control-flow region"
                                    ),
                                );
                            }
                        }
                        _ => {}
                    }
                }

                let Some(terminator) = &block.terminator else {
                    continue;
                };
                match terminator {
                    MirTerminator::Branch {
                        condition,
                        true_target,
                        false_target,
                        ..
                    } if reads.contains_key(condition) => {
                        let path_condition = reads[condition];
                        for (target, active) in [(*true_target, true), (*false_target, false)] {
                            let (selected, missing) = state.select(path_condition, active);
                            if missing && reported_reads.insert((block.id, path_condition)) {
                                self.block_error(
                                    function.callable(),
                                    block.id,
                                    format!(
                                        "path condition {path_condition} is tested before selection or outside its active parent path"
                                    ),
                                );
                            }
                            self.merge_path_edge(
                                function,
                                block.id,
                                target,
                                &selected,
                                &mut flow,
                                &mut reported_edges,
                            );
                        }
                    }
                    _ => {
                        for target in terminator.successors() {
                            self.merge_path_edge(
                                function,
                                block.id,
                                target,
                                &state,
                                &mut flow,
                                &mut reported_edges,
                            );
                        }
                    }
                }
            }
            if !flow.seed_next_component(&function.body().blocks, PathStates::initial(())) {
                break;
            }
        }
    }

    fn merge_path_edge(
        &mut self,
        function: MirDefinitionRef<'_>,
        predecessor: BlockId,
        target: BlockId,
        state: &PathStates<()>,
        flow: &mut ForwardDataflow<PathStates<()>>,
        reported: &mut HashSet<(BlockId, BlockId)>,
    ) {
        let selected = match state.on_edge(function, predecessor, target) {
            Ok(selected) => selected,
            Err(error) => {
                if reported.insert((predecessor, target)) {
                    self.block_error(
                        function.callable(),
                        predecessor,
                        path_edge_error(error, target),
                    );
                }
                state.clone()
            }
        };
        flow.merge(target, &selected, |existing, incoming| {
            existing.merge(incoming, |_, _| {})
        });
    }
}

fn predecessors(function: MirDefinitionRef<'_>) -> HashMap<BlockId, BTreeSet<BlockId>> {
    let mut predecessors: HashMap<_, BTreeSet<_>> = HashMap::new();
    for block in &function.body().blocks {
        for target in block.terminator.iter().flat_map(MirTerminator::successors) {
            predecessors.entry(target).or_default().insert(block.id);
        }
    }
    predecessors
}

fn constant_bool_in_block(block: &crate::mir::MirBasicBlock, value: ValueId) -> Option<bool> {
    block
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) if assignment.result == value => {
                match assignment.rvalue.kind {
                    MirRvalueKind::ConstantBool(value) => Some(value),
                    _ => None,
                }
            }
            _ => None,
        })
}

fn path_edge_error(error: PathEdgeError, target: BlockId) -> String {
    match error {
        PathEdgeError::ParentNotActive { condition, parent } => format!(
            "path condition {condition} reaches merge {target} outside active parent {parent}"
        ),
        PathEdgeError::ConditionAlreadySelected(condition) => {
            format!("path condition {condition} is selected more than once before merge {target}")
        }
    }
}
