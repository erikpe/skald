//! Structural verification for checked integer-shift diamonds.

use std::collections::{HashMap, HashSet};

use super::{
    super::model::{
        BlockId, MirBasicBlock, MirDefinitionRef, MirInstruction, MirPlace, MirRvalueKind,
        MirShiftCountCheck, MirShiftOperation, MirStorageKind, MirTerminator, MirType, StorageId,
    },
    context::Verifier,
};

impl Verifier<'_> {
    pub(super) fn verify_shift_count_check(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        check: &MirShiftCountCheck,
        success_target: BlockId,
        failure_target: BlockId,
    ) {
        self.verify_block_target(function, block, success_target);
        self.verify_block_target(function, block, failure_target);

        if check.left == check.count || check.left == check.result || check.count == check.result {
            self.block_error(
                function.callable(),
                block.id,
                "shift check requires distinct left, count, and result carriers",
            );
        }
        self.verify_shift_carrier(
            function,
            block,
            check.left,
            check.operation.left_type(),
            "left",
        );
        self.verify_shift_carrier(
            function,
            block,
            check.count,
            check.operation.count_type(),
            "count",
        );
        self.verify_shift_carrier(
            function,
            block,
            check.result,
            check.operation.result_type(),
            "result",
        );

        if !function.block(failure_target).is_some_and(|failure| {
            failure.instructions.is_empty()
                && matches!(
                    failure.terminator,
                    Some(MirTerminator::Terminate {
                        reason,
                        ..
                    }) if reason == check.operation.failure_reason()
                )
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "shift failure edge must directly terminate with `shift count out of range`",
            );
        }

        let Some(success) = function.block(success_target) else {
            return;
        };
        let Some((join, shifted)) = checked_shift_success(success, check) else {
            self.block_error(
                function.callable(),
                block.id,
                "shift success edge must load the secured operands, perform the matching shift, store its result, and join",
            );
            return;
        };
        if function.value(shifted).map(|value| value.ty) != Some(check.operation.result_type()) {
            self.block_error(
                function.callable(),
                block.id,
                "shift success value has the wrong exact result type",
            );
        }
        if !function.block(join).is_some_and(|join| {
            matches!(
                join.instructions.first(),
                Some(MirInstruction::Assign(load))
                    if is_exact_load(&load.rvalue.kind, check.result)
                        && load.rvalue.ty == check.operation.result_type()
            )
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "shift join must begin by loading the checked result carrier",
            );
        }
    }

    pub(super) fn verify_checked_shifts(&mut self, function: MirDefinitionRef<'_>) {
        let mut predecessors: HashMap<BlockId, HashSet<BlockId>> = HashMap::new();
        let mut checked_successes = HashMap::<BlockId, MirShiftOperation>::new();
        for block in &function.body().blocks {
            if let Some(terminator) = &block.terminator {
                for successor in terminator.successors() {
                    predecessors.entry(successor).or_default().insert(block.id);
                }
            }
        }
        for block in &function.body().blocks {
            if let Some(MirTerminator::ShiftCountCheck {
                check,
                success_target,
                ..
            }) = &block.terminator
            {
                if checked_successes
                    .insert(*success_target, check.operation)
                    .is_some()
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "shift success block is shared by multiple checks",
                    );
                }
                if predecessors
                    .get(success_target)
                    .is_some_and(|incoming| incoming.len() != 1 || !incoming.contains(&block.id))
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "shift success block must be dominated by its matching count check",
                    );
                }
            }
        }

        for block in &function.body().blocks {
            for instruction in &block.instructions {
                let MirInstruction::Assign(assignment) = instruction else {
                    continue;
                };
                let MirRvalueKind::Shift { operation, .. } = assignment.rvalue.kind else {
                    continue;
                };
                if checked_successes.get(&block.id) != Some(&operation) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "shift operation is not protected by its matching count check",
                    );
                }
            }
        }
    }

    fn verify_shift_carrier(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        carrier: StorageId,
        expected: MirType,
        name: &str,
    ) {
        if function
            .storage(carrier)
            .map(|storage| (storage.kind, storage.ty))
            != Some((MirStorageKind::ScalarSpill, expected))
        {
            self.block_error(
                function.callable(),
                block.id,
                format!("shift {name} carrier must be an exact `{expected}` scalar spill"),
            );
        }
    }
}

fn checked_shift_success(
    block: &MirBasicBlock,
    check: &MirShiftCountCheck,
) -> Option<(BlockId, super::super::model::ValueId)> {
    let [MirInstruction::Assign(left), MirInstruction::Assign(count), MirInstruction::Assign(shift), MirInstruction::Store(store)] =
        block.instructions.as_slice()
    else {
        return None;
    };
    let MirRvalueKind::Shift {
        operation,
        left: shift_left,
        count: shift_count,
    } = shift.rvalue.kind
    else {
        return None;
    };
    if !is_exact_load(&left.rvalue.kind, check.left)
        || left.rvalue.ty != check.operation.left_type()
        || !is_exact_load(&count.rvalue.kind, check.count)
        || count.rvalue.ty != check.operation.count_type()
        || operation != check.operation
        || shift_left != left.result
        || shift_count != count.result
        || shift.rvalue.ty != check.operation.result_type()
        || store.destination != MirPlace::base(check.result)
        || store.value != shift.result
    {
        return None;
    }
    match block.terminator {
        Some(MirTerminator::Goto { target, .. }) => Some((target, shift.result)),
        _ => None,
    }
}

fn is_exact_load(rvalue: &MirRvalueKind, storage: StorageId) -> bool {
    matches!(rvalue, MirRvalueKind::Load(place) if *place == MirPlace::base(storage))
}
