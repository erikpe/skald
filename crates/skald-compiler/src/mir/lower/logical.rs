//! Structured short-circuit boolean expression lowering.

use crate::{
    hir::{HirExpression, HirLogicalExpression, HirLogicalOperation},
    mir::{
        MirInstruction, MirLogicalExpression, MirLogicalOperation, MirPathCondition,
        MirPathConditionValue, MirPlace, MirRvalueKind, MirStorage, MirStorageKind, MirStore,
        MirTerminator, MirType, StorageId, ValueId,
    },
};

use super::BodyLowerer;

impl BodyLowerer<'_> {
    pub(super) fn lower_logical(
        &mut self,
        expression: &HirExpression,
        logical: &HirLogicalExpression,
    ) -> ValueId {
        logical.validate(expression.ty);
        let metadata_slot = self.body.reserve_logical_expression();
        let enclosing_condition = self.current_full_expression_condition();

        // This order is semantic. Reserving metadata above preserves
        // source-structural dump order without moving either operand.
        let left = self
            .lower_expression(&logical.left)
            .expect("typed logical left operand must produce a boolean value");
        assert_eq!(
            self.current_full_expression_condition(),
            enclosing_condition,
            "logical left lowering must restore its enclosing path condition"
        );

        let result = self.new_logical_storage(
            MirStorageKind::ScalarSpill,
            "logical-result",
            expression.span,
        );
        self.track_full_expression_storage(result, expression.span);
        let activation = self.new_logical_storage(
            MirStorageKind::PathCondition,
            "logical-condition",
            expression.span,
        );
        self.begin_storage_lifetime(activation, expression.span);

        // The selection diamond records whether the right operand will run.
        // Its merge precedes the right operand so nested decisions can inherit
        // this condition as an already selected parent.
        let split = self.body.current();
        let active_predecessor = self.body.allocate_block(expression.span);
        let inactive_predecessor = self.body.allocate_block(expression.span);
        let selection = self.body.allocate_block(expression.span);
        let right_entry = self.body.allocate_block(logical.right.span);
        let short = self.body.allocate_block(expression.span);
        let join = self.body.allocate_block(expression.span);

        let condition = self.body.next_path_condition_id();
        self.register_full_expression_condition(MirPathCondition {
            id: condition,
            parent: enclosing_condition,
            activation,
            active_predecessor,
            inactive_predecessor,
            merge: selection,
            span: expression.span,
        });

        let (true_target, false_target) = match logical.operation {
            HirLogicalOperation::And => (active_predecessor, inactive_predecessor),
            HirLogicalOperation::Or => (inactive_predecessor, active_predecessor),
        };
        self.terminate(MirTerminator::Branch {
            condition: left,
            true_target,
            false_target,
            span: expression.span,
        });

        self.emit_logical_selection(active_predecessor, activation, true, selection, expression);
        self.emit_logical_selection(
            inactive_predecessor,
            activation,
            false,
            selection,
            expression,
        );

        self.body
            .select_block(selection)
            .expect("allocated logical selection block must be selectable");
        let selected_path = self.assign(
            MirRvalueKind::PathCondition(MirPathConditionValue {
                condition,
                activation,
            }),
            MirType::Bool,
            expression.span,
        );
        self.terminate(MirTerminator::Branch {
            condition: selected_path,
            true_target: right_entry,
            false_target: short,
            span: expression.span,
        });

        self.body
            .select_block(right_entry)
            .expect("allocated logical right block must be selectable");
        self.select_full_expression_condition(Some(condition));
        let right = self
            .lower_expression(&logical.right)
            .expect("typed logical right operand must produce a boolean value");
        let right_exit = self.body.current();
        self.emit(MirInstruction::Store(MirStore {
            destination: MirPlace::base(result),
            value: right,
            authorization: None,
            final_authorization: None,
            span: logical.right.span,
        }));
        self.terminate(MirTerminator::Goto {
            target: join,
            span: logical.right.span,
        });

        self.body
            .select_block(short)
            .expect("allocated logical short block must be selectable");
        self.select_full_expression_condition(enclosing_condition);
        let fixed = self.assign(
            MirRvalueKind::ConstantBool(logical.operation.fixed_short_result()),
            MirType::Bool,
            expression.span,
        );
        self.emit(MirInstruction::Store(MirStore {
            destination: MirPlace::base(result),
            value: fixed,
            authorization: None,
            final_authorization: None,
            span: expression.span,
        }));
        self.terminate(MirTerminator::Goto {
            target: join,
            span: expression.span,
        });

        self.body
            .select_block(join)
            .expect("allocated logical result join must be selectable");
        self.select_full_expression_condition(enclosing_condition);
        let selected_result = self.assign(
            MirRvalueKind::Load(MirPlace::base(result)),
            MirType::Bool,
            expression.span,
        );

        self.body.define_logical_expression(
            metadata_slot,
            MirLogicalExpression {
                operation: match logical.operation {
                    HirLogicalOperation::And => MirLogicalOperation::And,
                    HirLogicalOperation::Or => MirLogicalOperation::Or,
                },
                condition,
                result,
                left_result: left,
                split,
                selection,
                right_entry,
                right_exit,
                right_result: right,
                short,
                join,
                selected_result,
                span: expression.span,
            },
        );
        selected_result
    }

    fn emit_logical_selection(
        &mut self,
        block: crate::mir::BlockId,
        activation: StorageId,
        active: bool,
        selection: crate::mir::BlockId,
        expression: &HirExpression,
    ) {
        self.body
            .select_block(block)
            .expect("allocated logical predecessor must be selectable");
        let selected = self.assign(
            MirRvalueKind::ConstantBool(active),
            MirType::Bool,
            expression.span,
        );
        self.emit(MirInstruction::Store(MirStore {
            destination: MirPlace::base(activation),
            value: selected,
            authorization: None,
            final_authorization: None,
            span: expression.span,
        }));
        self.terminate(MirTerminator::Goto {
            target: selection,
            span: expression.span,
        });
    }

    fn new_logical_storage(
        &mut self,
        kind: MirStorageKind,
        name: &str,
        span: crate::source::Span,
    ) -> StorageId {
        let storage = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id: storage,
            source: None,
            name: format!("{name}{}", storage.index()),
            kind,
            ty: MirType::Bool,
            span,
        });
        storage
    }
}
