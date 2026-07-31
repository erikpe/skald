//! Structural verification for checked floating-to-integer cast diamonds.

use std::collections::{HashMap, HashSet};

use super::{
    super::model::{
        BlockId, MirBasicBlock, MirDefinitionRef, MirF64ToIntegerRange, MirInstruction, MirPlace,
        MirPrimitiveCastRangeCheck, MirRvalueKind, MirStorageKind, MirTerminator, MirType,
        StorageId,
    },
    checked_scalar::{dominates, is_exact_load, predecessors, storage_writes},
    context::Verifier,
};

impl Verifier<'_> {
    pub(super) fn verify_primitive_cast_range_check(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        check: &MirPrimitiveCastRangeCheck,
        success_target: BlockId,
        failure_target: BlockId,
    ) {
        self.verify_block_target(function, block, success_target);
        self.verify_block_target(function, block, failure_target);

        if success_target == failure_target {
            self.block_error(
                function.callable(),
                block.id,
                "primitive cast range check requires distinct success and failure targets",
            );
        }
        if check.source == check.result {
            self.block_error(
                function.callable(),
                block.id,
                "primitive cast range check requires distinct source and result carriers",
            );
        }
        self.verify_primitive_cast_carrier(
            function,
            block,
            check.source,
            check.relation.source_type(),
            "source",
        );
        self.verify_primitive_cast_carrier(
            function,
            block,
            check.result,
            check.relation.result_type(),
            "result",
        );

        if !function.block(failure_target).is_some_and(|failure| {
            failure.instructions.is_empty()
                && matches!(
                    failure.terminator,
                    Some(MirTerminator::Terminate { reason, .. })
                        if reason == check.relation.failure_reason()
                )
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "primitive cast failure edge must directly terminate with `primitive-cast-out-of-range`",
            );
        }

        let Some(success) = function.block(success_target) else {
            return;
        };
        let Some(join) = checked_primitive_cast_success(success, check) else {
            self.block_error(
                function.callable(),
                block.id,
                "primitive cast success edge must load the secured source, perform the matching checked conversion, store its result, and join",
            );
            return;
        };
        if !function.block(join).is_some_and(|join| {
            matches!(
                join.instructions.first(),
                Some(MirInstruction::Assign(load))
                    if is_exact_load(&load.rvalue.kind, check.result)
                        && load.rvalue.ty == check.relation.result_type()
            )
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "primitive cast join must begin by loading the checked result carrier",
            );
        }
    }

    pub(super) fn verify_checked_primitive_casts(&mut self, function: MirDefinitionRef<'_>) {
        let predecessors = predecessors(function);
        let mut checked_successes = HashMap::<BlockId, MirF64ToIntegerRange>::new();

        for block in &function.body().blocks {
            let Some(MirTerminator::PrimitiveCastRangeCheck {
                check,
                success_target,
                failure_target,
                ..
            }) = &block.terminator
            else {
                continue;
            };

            if checked_successes
                .insert(*success_target, check.relation)
                .is_some()
            {
                self.block_error(
                    function.callable(),
                    block.id,
                    "primitive cast success block is shared by multiple checks",
                );
            }
            self.require_primitive_cast_predecessor(
                function,
                block,
                &predecessors,
                *success_target,
                block.id,
                "primitive cast success block must be dominated by its matching range check",
            );
            self.require_primitive_cast_predecessor(
                function,
                block,
                &predecessors,
                *failure_target,
                block.id,
                "primitive cast failure block must be reached only by its matching range check",
            );
            self.verify_primitive_cast_source_write(function, block, check.source);

            if let Some(success) = function.block(*success_target) {
                if let Some(join) = checked_primitive_cast_success(success, check) {
                    self.require_primitive_cast_predecessor(
                        function,
                        block,
                        &predecessors,
                        join,
                        *success_target,
                        "primitive cast result join must be reached only from its success block",
                    );
                }
                if storage_writes(function, check.result).as_slice() != [*success_target] {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "primitive cast result carrier must be written exactly once by its success block",
                    );
                }
            }
        }

        for block in &function.body().blocks {
            for instruction in &block.instructions {
                let MirInstruction::Assign(assignment) = instruction else {
                    continue;
                };
                let MirRvalueKind::CheckedF64ToInteger { relation, .. } = assignment.rvalue.kind
                else {
                    continue;
                };
                if checked_successes.get(&block.id) != Some(&relation) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "checked floating-to-integer conversion is not protected by its matching range check",
                    );
                }
            }
        }
    }

    fn verify_primitive_cast_carrier(
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
                format!("primitive cast {name} carrier must be an exact `{expected}` scalar spill"),
            );
        }
    }

    fn verify_primitive_cast_source_write(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        source: StorageId,
    ) {
        let writes = storage_writes(function, source);
        if writes.len() != 1 || !dominates(function, writes[0], block.id) {
            self.block_error(
                function.callable(),
                block.id,
                "primitive cast source carrier must have one write dominating its range check",
            );
        }
    }

    fn require_primitive_cast_predecessor(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        predecessors: &HashMap<BlockId, HashSet<BlockId>>,
        target: BlockId,
        expected: BlockId,
        message: &str,
    ) {
        if predecessors.get(&target) != Some(&HashSet::from([expected])) {
            self.block_error(function.callable(), block.id, message);
        }
    }
}

fn checked_primitive_cast_success(
    block: &MirBasicBlock,
    check: &MirPrimitiveCastRangeCheck,
) -> Option<BlockId> {
    let [MirInstruction::Assign(source), MirInstruction::Assign(conversion), MirInstruction::Store(store)] =
        block.instructions.as_slice()
    else {
        return None;
    };
    let MirRvalueKind::CheckedF64ToInteger { relation, operand } = conversion.rvalue.kind else {
        return None;
    };
    if !is_exact_load(&source.rvalue.kind, check.source)
        || source.rvalue.ty != check.relation.source_type()
        || relation != check.relation
        || operand != source.result
        || conversion.rvalue.ty != check.relation.result_type()
        || store.destination != MirPlace::base(check.result)
        || store.value != conversion.result
    {
        return None;
    }
    match block.terminator {
        Some(MirTerminator::Goto { target, .. }) => Some(target),
        _ => None,
    }
}
