//! Verification of structured logical-expression MIR provenance.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::mir::{
    BlockId, MirDefinitionRef, MirInstruction, MirLogicalExpression, MirLogicalOperation,
    MirPlaceBase, MirRvalueKind, MirStorageKind, MirTerminator, MirType, StorageId, ValueId,
};

use super::context::Verifier;

impl Verifier<'_> {
    pub(super) fn verify_logical_expressions(&mut self, function: MirDefinitionRef<'_>) {
        let predecessors = predecessors(function);
        let mut conditions = HashSet::new();
        for logical in function.logical_expressions() {
            if !conditions.insert(logical.condition) {
                self.function_error(
                    function.callable(),
                    format!(
                        "path condition {} describes more than one logical expression",
                        logical.condition
                    ),
                );
            }
            self.verify_logical_expression(function, logical, &predecessors);
        }
    }

    fn verify_logical_expression(
        &mut self,
        function: MirDefinitionRef<'_>,
        logical: &MirLogicalExpression,
        predecessors: &HashMap<BlockId, HashSet<BlockId>>,
    ) {
        let Some(condition) = function.path_condition(logical.condition) else {
            self.function_error(
                function.callable(),
                format!(
                    "logical expression references undeclared path condition {}",
                    logical.condition
                ),
            );
            return;
        };
        if condition.merge != logical.selection {
            self.logical_error(
                function,
                logical,
                "logical selection block differs from its path-condition merge",
            );
        }

        match function.storage(logical.result) {
            Some(storage)
                if storage.kind == MirStorageKind::ScalarSpill && storage.ty == MirType::Bool => {}
            Some(_) => self.logical_error(
                function,
                logical,
                "logical result carrier must be `bool` scalar-spill storage",
            ),
            None => self.logical_error(
                function,
                logical,
                "logical result carrier storage is not declared",
            ),
        }
        self.require_bool_value(function, logical, logical.left_result, "left result");
        self.require_bool_value(function, logical, logical.right_result, "right result");
        self.require_bool_value(
            function,
            logical,
            logical.selected_result,
            "selected result",
        );

        let Some(split) = function.block(logical.split) else {
            self.logical_error(function, logical, "logical split block is not declared");
            return;
        };
        let expected_targets = match logical.operation {
            MirLogicalOperation::And => {
                (condition.active_predecessor, condition.inactive_predecessor)
            }
            MirLogicalOperation::Or => {
                (condition.inactive_predecessor, condition.active_predecessor)
            }
        };
        if !matches!(
            split.terminator,
            Some(MirTerminator::Branch {
                condition,
                true_target,
                false_target,
                ..
            }) if condition == logical.left_result
                && (true_target, false_target) == expected_targets
        ) {
            self.logical_error(
                function,
                logical,
                "logical split has the wrong operand or branch targets",
            );
        }
        for storage in [logical.result, condition.activation] {
            if !split.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::StorageLive(operation) if operation.storage == storage
                )
            }) {
                self.logical_error(
                    function,
                    logical,
                    format!("logical carrier {storage} is not live before the split"),
                );
            }
        }

        self.verify_selection_block(function, logical, condition.activation);
        self.verify_result_predecessor(
            function,
            logical,
            logical.short,
            None,
            Some(logical.operation.fixed_short_result()),
            "short",
        );
        self.verify_result_predecessor(
            function,
            logical,
            logical.right_exit,
            Some(logical.right_result),
            None,
            "right",
        );

        let expected_predecessors = HashSet::from([logical.short, logical.right_exit]);
        if predecessors.get(&logical.join) != Some(&expected_predecessors) {
            self.logical_error(
                function,
                logical,
                "logical result join must have exactly its short and right predecessors",
            );
        }
        let Some(join) = function.block(logical.join) else {
            self.logical_error(function, logical, "logical result join is not declared");
            return;
        };
        if !join.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                MirInstruction::Assign(assignment)
                    if assignment.result == logical.selected_result
                        && matches!(
                            &assignment.rvalue.kind,
                            MirRvalueKind::Load(place)
                                if place.base == MirPlaceBase::Storage(logical.result)
                                    && place.projections.is_empty()
                        )
                        && assignment.rvalue.ty == MirType::Bool
            )
        }) {
            self.logical_error(
                function,
                logical,
                "logical selected result must load its carrier in the result join",
            );
        }

        let stores: Vec<_> = function
            .body()
            .blocks
            .iter()
            .flat_map(|block| {
                block.instructions.iter().filter_map(move |instruction| {
                    let MirInstruction::Store(store) = instruction else {
                        return None;
                    };
                    (store.destination.base == MirPlaceBase::Storage(logical.result)
                        && store.destination.projections.is_empty())
                    .then_some(block.id)
                })
            })
            .collect();
        if stores.len() != 2
            || !stores.contains(&logical.short)
            || !stores.contains(&logical.right_exit)
        {
            self.logical_error(
                function,
                logical,
                "logical result carrier must be written exactly once on each selected path",
            );
        }

        if !reachable_without(
            function,
            logical.right_entry,
            logical.right_exit,
            &[logical.short, logical.join],
        ) {
            self.logical_error(
                function,
                logical,
                "logical right completion is not reachable exclusively from its right entry",
            );
        }
        self.verify_right_region_exclusivity(function, logical, predecessors);
    }

    fn verify_right_region_exclusivity(
        &mut self,
        function: MirDefinitionRef<'_>,
        logical: &MirLogicalExpression,
        predecessors: &HashMap<BlockId, HashSet<BlockId>>,
    ) {
        let region = reachable_region(
            function,
            logical.right_entry,
            &[logical.short, logical.join],
        );
        for block in &region {
            let has_external_predecessor = if *block == logical.right_entry {
                predecessors
                    .get(block)
                    .is_some_and(|incoming| incoming != &HashSet::from([logical.selection]))
            } else {
                predecessors
                    .get(block)
                    .is_some_and(|incoming| !incoming.is_subset(&region))
            };
            if has_external_predecessor {
                self.logical_error(
                    function,
                    logical,
                    format!(
                        "logical right-only block {block} has an incoming edge outside its selected region"
                    ),
                );
            }
        }
    }

    fn verify_selection_block(
        &mut self,
        function: MirDefinitionRef<'_>,
        logical: &MirLogicalExpression,
        activation: StorageId,
    ) {
        let Some(selection) = function.block(logical.selection) else {
            self.logical_error(function, logical, "logical selection block is not declared");
            return;
        };
        let read = selection.instructions.iter().find_map(|instruction| {
            let MirInstruction::Assign(assignment) = instruction else {
                return None;
            };
            match assignment.rvalue.kind {
                MirRvalueKind::PathCondition(read)
                    if read.condition == logical.condition && read.activation == activation =>
                {
                    Some(assignment.result)
                }
                _ => None,
            }
        });
        if !matches!(
            (read, &selection.terminator),
            (
                Some(read),
                Some(MirTerminator::Branch {
                    condition,
                    true_target,
                    false_target,
                    ..
                })
            ) if *condition == read
                && *true_target == logical.right_entry
                && *false_target == logical.short
        ) {
            self.logical_error(
                function,
                logical,
                "logical selection must branch on its activation to right and short paths",
            );
        }
    }

    fn verify_result_predecessor(
        &mut self,
        function: MirDefinitionRef<'_>,
        logical: &MirLogicalExpression,
        block_id: BlockId,
        expected_value: Option<ValueId>,
        fixed: Option<bool>,
        path: &str,
    ) {
        let Some(block) = function.block(block_id) else {
            self.logical_error(
                function,
                logical,
                format!("logical {path} result block is not declared"),
            );
            return;
        };
        if !matches!(
            block.terminator,
            Some(MirTerminator::Goto { target, .. }) if target == logical.join
        ) {
            self.logical_error(
                function,
                logical,
                format!("logical {path} result block must jump directly to the result join"),
            );
        }
        let Some(MirInstruction::Store(store)) = block.instructions.last() else {
            self.logical_error(
                function,
                logical,
                format!("logical {path} result block must end by storing its result"),
            );
            return;
        };
        if store.destination.base != MirPlaceBase::Storage(logical.result)
            || !store.destination.projections.is_empty()
            || expected_value.is_some_and(|expected| store.value != expected)
            || fixed.is_some_and(|expected| constant_bool(block, store.value) != Some(expected))
        {
            self.logical_error(
                function,
                logical,
                format!("logical {path} path stores the wrong selected result"),
            );
        }
    }

    fn require_bool_value(
        &mut self,
        function: MirDefinitionRef<'_>,
        logical: &MirLogicalExpression,
        value: ValueId,
        role: &str,
    ) {
        if function.value(value).map(|value| value.ty) != Some(MirType::Bool) {
            self.logical_error(
                function,
                logical,
                format!("logical {role} must have exact type `bool`"),
            );
        }
    }

    fn logical_error(
        &mut self,
        function: MirDefinitionRef<'_>,
        logical: &MirLogicalExpression,
        message: impl Into<String>,
    ) {
        self.block_error(function.callable(), logical.split, message);
    }
}

fn constant_bool(block: &crate::mir::MirBasicBlock, value: ValueId) -> Option<bool> {
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

fn predecessors(function: MirDefinitionRef<'_>) -> HashMap<BlockId, HashSet<BlockId>> {
    let mut predecessors: HashMap<_, HashSet<_>> = HashMap::new();
    for block in &function.body().blocks {
        for target in block.terminator.iter().flat_map(MirTerminator::successors) {
            predecessors.entry(target).or_default().insert(block.id);
        }
    }
    predecessors
}

fn reachable_without(
    function: MirDefinitionRef<'_>,
    start: BlockId,
    target: BlockId,
    forbidden: &[BlockId],
) -> bool {
    let mut pending = VecDeque::from([start]);
    let mut visited = HashSet::new();
    while let Some(block) = pending.pop_front() {
        if block == target {
            return true;
        }
        if forbidden.contains(&block) || !visited.insert(block) {
            continue;
        }
        if let Some(block) = function.block(block) {
            pending.extend(block.terminator.iter().flat_map(MirTerminator::successors));
        }
    }
    false
}

fn reachable_region(
    function: MirDefinitionRef<'_>,
    start: BlockId,
    boundaries: &[BlockId],
) -> HashSet<BlockId> {
    let mut pending = VecDeque::from([start]);
    let mut region = HashSet::new();
    while let Some(block) = pending.pop_front() {
        if boundaries.contains(&block) || !region.insert(block) {
            continue;
        }
        if let Some(block) = function.block(block) {
            pending.extend(block.terminator.iter().flat_map(MirTerminator::successors));
        }
    }
    region
}
