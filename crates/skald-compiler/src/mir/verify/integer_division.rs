//! Structural verification for checked integer division/remainder diamonds.

use std::collections::{HashMap, HashSet};

use super::{
    super::model::{
        BlockId, MirBasicBlock, MirDefinitionRef, MirInstruction, MirIntegerDivisionOperation,
        MirIntegerDivisorCheck, MirPlace, MirRvalueKind, MirStorageKind, MirTerminator, MirType,
        StorageId, ValueId,
    },
    checked_scalar::{dominates, is_exact_load, predecessors, storage_writes},
    context::Verifier,
};

impl Verifier<'_> {
    pub(super) fn verify_integer_divisor_check(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        check: &MirIntegerDivisorCheck,
        success_target: BlockId,
        failure_target: BlockId,
    ) {
        self.verify_block_target(function, block, success_target);
        self.verify_block_target(function, block, failure_target);

        if success_target == failure_target {
            self.block_error(
                function.callable(),
                block.id,
                "integer divisor check requires distinct success and failure targets",
            );
        }
        if check.dividend == check.divisor
            || check.dividend == check.result
            || check.divisor == check.result
        {
            self.block_error(
                function.callable(),
                block.id,
                "integer divisor check requires distinct dividend, divisor, and result carriers",
            );
        }
        self.verify_integer_division_carrier(
            function,
            block,
            check.dividend,
            check.operation.operand_type(),
            "dividend",
        );
        self.verify_integer_division_carrier(
            function,
            block,
            check.divisor,
            check.operation.operand_type(),
            "divisor",
        );
        self.verify_integer_division_carrier(
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
                    Some(MirTerminator::Terminate { reason, .. })
                        if reason == check.operation.failure_reason()
                )
        }) {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "integer divisor failure edge must directly terminate with `{}`",
                    check.operation.failure_reason().mnemonic()
                ),
            );
        }

        let Some(success) = function.block(success_target) else {
            return;
        };
        let Some((join, value)) = checked_integer_division_success(success, check) else {
            self.block_error(
                function.callable(),
                block.id,
                "integer divisor success edge must load the secured operands, perform the matching operation, store its result, and join",
            );
            return;
        };
        if function.value(value).map(|value| value.ty) != Some(check.operation.result_type()) {
            self.block_error(
                function.callable(),
                block.id,
                "integer division or remainder success value has the wrong exact result type",
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
                "integer division or remainder join must begin by loading the checked result carrier",
            );
        }
    }

    pub(super) fn verify_checked_integer_divisions(&mut self, function: MirDefinitionRef<'_>) {
        let predecessors = predecessors(function);
        let mut checked_successes = HashMap::<BlockId, MirIntegerDivisionOperation>::new();
        for block in &function.body().blocks {
            let Some(MirTerminator::IntegerDivisorCheck {
                check,
                success_target,
                failure_target,
                ..
            }) = &block.terminator
            else {
                continue;
            };

            if checked_successes
                .insert(*success_target, check.operation)
                .is_some()
            {
                self.block_error(
                    function.callable(),
                    block.id,
                    "integer divisor success block is shared by multiple checks",
                );
            }
            self.require_exact_predecessor(
                function,
                block,
                &predecessors,
                *success_target,
                block.id,
                "integer divisor success block must be dominated by its matching check",
            );
            self.require_exact_predecessor(
                function,
                block,
                &predecessors,
                *failure_target,
                block.id,
                "integer divisor failure block must be reached only by its matching check",
            );

            self.verify_unique_dominating_carrier_write(
                function,
                block,
                check.dividend,
                "dividend",
            );
            self.verify_unique_dominating_carrier_write(function, block, check.divisor, "divisor");

            if let Some(success) = function.block(*success_target) {
                if let Some((join, _)) = checked_integer_division_success(success, check) {
                    self.require_exact_predecessor(
                        function,
                        block,
                        &predecessors,
                        join,
                        *success_target,
                        "integer division or remainder result join must be reached only from its success block",
                    );
                }
                let result_writes = storage_writes(function, check.result);
                if result_writes.as_slice() != [*success_target] {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "integer division or remainder result carrier must be written exactly once by its success block",
                    );
                }
            }
        }

        for block in &function.body().blocks {
            for instruction in &block.instructions {
                let MirInstruction::Assign(assignment) = instruction else {
                    continue;
                };
                let MirRvalueKind::IntegerDivision { operation, .. } = assignment.rvalue.kind
                else {
                    continue;
                };
                if checked_successes.get(&block.id) != Some(&operation) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "integer division or remainder operation is not protected by its matching divisor check",
                    );
                }
            }
        }
    }

    fn verify_integer_division_carrier(
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
                format!(
                    "integer division {name} carrier must be an exact `{expected}` scalar spill"
                ),
            );
        }
    }

    fn verify_unique_dominating_carrier_write(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        carrier: StorageId,
        name: &str,
    ) {
        let writes = storage_writes(function, carrier);
        if writes.len() != 1 || !dominates(function, writes[0], block.id) {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "integer division {name} carrier must have one write dominating its divisor check"
                ),
            );
        }
    }

    fn require_exact_predecessor(
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

fn checked_integer_division_success(
    block: &MirBasicBlock,
    check: &MirIntegerDivisorCheck,
) -> Option<(BlockId, ValueId)> {
    let [MirInstruction::Assign(dividend), MirInstruction::Assign(divisor), MirInstruction::Assign(operation), MirInstruction::Store(store)] =
        block.instructions.as_slice()
    else {
        return None;
    };
    let MirRvalueKind::IntegerDivision {
        operation: actual_operation,
        dividend: actual_dividend,
        divisor: actual_divisor,
    } = operation.rvalue.kind
    else {
        return None;
    };
    if !is_exact_load(&dividend.rvalue.kind, check.dividend)
        || dividend.rvalue.ty != check.operation.operand_type()
        || !is_exact_load(&divisor.rvalue.kind, check.divisor)
        || divisor.rvalue.ty != check.operation.operand_type()
        || actual_operation != check.operation
        || actual_dividend != dividend.result
        || actual_divisor != divisor.result
        || operation.rvalue.ty != check.operation.result_type()
        || store.destination != MirPlace::base(check.result)
        || store.value != operation.result
    {
        return None;
    }
    match block.terminator {
        Some(MirTerminator::Goto { target, .. }) => Some((target, operation.result)),
        _ => None,
    }
}
