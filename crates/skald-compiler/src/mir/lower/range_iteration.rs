//! Immediate primitive ranges lowered to existing scalar MIR operations.

use super::*;
use crate::hir::{HirBinaryOperation, HirForIn, HirForInPlan, HirIntegerType};

impl BodyLowerer<'_> {
    pub(super) fn lower_for_in(&mut self, statement: &HirForIn) {
        match &statement.plan {
            HirForInPlan::Protocol(_) => self.lower_protocol_for_in(statement),
            HirForInPlan::PrimitiveRange(plan) => {
                self.lower_primitive_range_for_in(statement, plan)
            }
        }
    }

    fn lower_primitive_range_for_in(
        &mut self,
        statement: &HirForIn,
        plan: &crate::hir::HirPrimitiveRangeIterationPlan,
    ) {
        let reaches_latch = statement.body.effects.can_fall_through()
            || statement.body.effects.can_continue_to(statement.loop_id);
        let header = self.body.allocate_block(statement.spans.iterable_span);
        let body = self.body.allocate_block(statement.body.span);
        let latch = reaches_latch.then(|| self.body.allocate_block(statement.spans.span));
        let outer_cleanup = self.body.allocate_block(statement.spans.span);
        let exit = self.body.allocate_block(statement.spans.span);
        let scalar = plan.integer.operand_type();
        let mir_scalar = self.lower_type(scalar);

        let break_retained_depth = self.cleanup.retained_scope_depth();
        self.cleanup.enter_scope();

        let current = self.new_iteration_storage(
            "range-current",
            MirStorageKind::Local,
            mir_scalar,
            plan.lower.span,
        );
        self.begin_storage_lifetime(current, plan.lower.span);
        self.cleanup.register_storage(current);
        let lower = self
            .lower_expression(&plan.lower)
            .expect("typed range lower endpoint must produce a scalar value");
        self.emit(MirInstruction::Store(MirStore {
            destination: MirPlace::base(current),
            value: lower,
            authorization: None,
            final_authorization: None,
            span: plan.lower.span,
        }));

        let end = self.new_iteration_storage(
            "range-end",
            MirStorageKind::Local,
            mir_scalar,
            plan.upper.span,
        );
        self.begin_storage_lifetime(end, plan.upper.span);
        self.cleanup.register_storage(end);
        let upper = self
            .lower_expression(&plan.upper)
            .expect("typed range upper endpoint must produce a scalar value");
        self.emit(MirInstruction::Store(MirStore {
            destination: MirPlace::base(end),
            value: upper,
            authorization: None,
            final_authorization: None,
            span: plan.upper.span,
        }));
        self.finish_full_expression(statement.spans.iterable_span);
        self.terminate(MirTerminator::Goto {
            target: header,
            span: statement.spans.span,
        });

        self.body
            .select_block(header)
            .expect("allocated range header must be selectable");
        let current_value = self.assign(
            MirRvalueKind::Load(MirPlace::base(current)),
            mir_scalar,
            plan.origin.operator_span,
        );
        let end_value = self.assign(
            MirRvalueKind::Load(MirPlace::base(end)),
            mir_scalar,
            plan.origin.operator_span,
        );
        let condition = self.assign(
            MirRvalueKind::PrimitiveComparison {
                operation: MirPrimitiveComparison {
                    predicate: MirComparisonPredicate::LessThan,
                    operand: MirComparisonOperand::Integer(lower_integer(plan.integer)),
                },
                left: current_value,
                right: end_value,
            },
            MirType::Bool,
            plan.origin.operator_span,
        );
        self.terminate(MirTerminator::Branch {
            condition,
            true_target: body,
            false_target: outer_cleanup,
            span: statement.spans.span,
        });

        let continue_retained_depth = self.cleanup.retained_scope_depth();
        let context = loop_context::LoopContext::new(
            statement.loop_id,
            exit,
            latch,
            break_retained_depth,
            continue_retained_depth,
        )
        .expect("typed range loop and its MIR targets must share a callable owner");
        self.loop_contexts.push(context);

        self.body
            .select_block(body)
            .expect("allocated range body must be selectable");
        self.cleanup.enter_scope();
        let item = self.local_storage[statement.binding.index()];
        self.begin_storage_lifetime(item, statement.spans.binding_span);
        self.cleanup.register_storage(item);
        let item_value = self.assign(
            MirRvalueKind::Load(MirPlace::base(current)),
            mir_scalar,
            statement.spans.binding_span,
        );
        self.emit(MirInstruction::Store(MirStore {
            destination: MirPlace::base(item),
            value: item_value,
            authorization: None,
            final_authorization: None,
            span: statement.spans.binding_span,
        }));

        let current_value = self.assign(
            MirRvalueKind::Load(MirPlace::base(current)),
            mir_scalar,
            plan.origin.operator_span,
        );
        let one = self.assign(
            range_one(plan.integer),
            mir_scalar,
            plan.origin.operator_span,
        );
        let incremented = self.assign(
            MirRvalueKind::Binary {
                operation: lower_increment(plan.increment),
                left: current_value,
                right: one,
            },
            mir_scalar,
            plan.origin.operator_span,
        );
        self.emit(MirInstruction::Store(MirStore {
            destination: MirPlace::base(current),
            value: incremented,
            authorization: None,
            final_authorization: None,
            span: plan.origin.operator_span,
        }));

        self.lower_block(&statement.body);
        if !self.body.is_current_terminated() {
            self.emit_scope_exit(self.cleanup.for_current_scope(statement.spans.span));
            self.terminate(MirTerminator::Goto {
                target: latch.expect("a falling-through range body requires a latch"),
                span: statement.spans.span,
            });
        }
        self.cleanup.leave_scope();
        self.loop_contexts.pop(statement.loop_id);

        if let Some(latch) = latch {
            self.body
                .select_block(latch)
                .expect("allocated range latch must be selectable");
            self.terminate(MirTerminator::Goto {
                target: header,
                span: statement.spans.span,
            });
        }

        self.body
            .select_block(outer_cleanup)
            .expect("allocated range outer-cleanup block must be selectable");
        self.emit_scope_exit(self.cleanup.for_current_scope(statement.spans.span));
        self.terminate(MirTerminator::Goto {
            target: exit,
            span: statement.spans.span,
        });
        self.cleanup.leave_scope();

        self.body
            .select_block(exit)
            .expect("allocated range exit must be selectable");
    }
}

const fn lower_integer(integer: HirIntegerType) -> MirIntegerType {
    match integer {
        HirIntegerType::I64 => MirIntegerType::I64,
        HirIntegerType::U64 => MirIntegerType::U64,
        HirIntegerType::U8 => MirIntegerType::U8,
    }
}

const fn range_one(integer: HirIntegerType) -> MirRvalueKind {
    match integer {
        HirIntegerType::I64 => MirRvalueKind::ConstantI64(1),
        HirIntegerType::U64 => MirRvalueKind::ConstantU64(1),
        HirIntegerType::U8 => MirRvalueKind::ConstantU8(1),
    }
}

const fn lower_increment(operation: HirBinaryOperation) -> MirBinaryOperation {
    match operation {
        HirBinaryOperation::AddI64 => MirBinaryOperation::AddI64,
        HirBinaryOperation::AddU64 => MirBinaryOperation::AddU64,
        HirBinaryOperation::AddU8 => MirBinaryOperation::AddU8,
        _ => panic!("validated primitive range plan must increment by same-typed addition"),
    }
}
