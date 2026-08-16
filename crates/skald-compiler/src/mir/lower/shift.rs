//! Checked integer shift lowering.

use crate::{
    hir::{HirCheckedShift, HirExpression, HirShiftDirection},
    mir::{
        MirInstruction, MirPlace, MirRvalueKind, MirShiftCountCheck, MirShiftDirection,
        MirShiftOperation, MirStore, MirTerminator, StorageId, ValueId,
    },
};

use super::{expression::lower_integer_type, BodyLowerer};

impl BodyLowerer<'_> {
    pub(super) fn lower_checked_shift(
        &mut self,
        expression: &HirExpression,
        shift: &HirCheckedShift,
    ) -> ValueId {
        shift.validate(expression.ty);
        let operation = MirShiftOperation {
            direction: match shift.operation.direction {
                HirShiftDirection::Left => MirShiftDirection::Left,
                HirShiftDirection::Right => MirShiftDirection::Right,
            },
            left: lower_integer_type(shift.operation.left),
        };

        // Evaluation and securing order is semantic. Each operand completes
        // before its value is copied into a carrier that crosses the check.
        let left = self
            .lower_expression(&shift.left)
            .expect("typed shift left operand must produce a value");
        let (left, _) = self.spill_scalar(left, operation.left_type(), shift.left.span);
        let count = self
            .lower_expression(&shift.count)
            .expect("typed shift count must produce a value");
        let (count, _) = self.spill_scalar(count, operation.count_type(), shift.count.span);
        let result = self.new_shift_result(operation.result_type(), expression.span);

        let success = self.body.allocate_block(expression.span);
        let failure = self.body.allocate_block(expression.span);
        let join = self.body.allocate_block(expression.span);
        self.terminate(MirTerminator::ShiftCountCheck {
            check: MirShiftCountCheck {
                operation,
                left,
                count,
                result,
            },
            success_target: success,
            failure_target: failure,
            span: expression.span,
        });

        self.body
            .select_block(success)
            .expect("allocated shift success block must be selectable");
        let left_value = self.assign(
            MirRvalueKind::Load(MirPlace::base(left)),
            operation.left_type(),
            shift.left.span,
        );
        let count_value = self.assign(
            MirRvalueKind::Load(MirPlace::base(count)),
            operation.count_type(),
            shift.count.span,
        );
        let shifted = self.assign(
            MirRvalueKind::Shift {
                operation,
                left: left_value,
                count: count_value,
            },
            operation.result_type(),
            expression.span,
        );
        self.emit(MirInstruction::Store(MirStore {
            destination: MirPlace::base(result),
            value: shifted,
            authorization: None,
            final_authorization: None,
            span: expression.span,
        }));
        self.terminate(MirTerminator::Goto {
            target: join,
            span: expression.span,
        });

        self.body
            .select_block(failure)
            .expect("allocated shift failure block must be selectable");
        self.terminate(MirTerminator::Terminate {
            reason: operation.failure_reason(),
            span: expression.span,
        });

        self.body
            .select_block(join)
            .expect("allocated shift result join must be selectable");
        self.assign(
            MirRvalueKind::Load(MirPlace::base(result)),
            operation.result_type(),
            expression.span,
        )
    }

    fn new_shift_result(
        &mut self,
        ty: crate::mir::MirType,
        span: crate::source::Span,
    ) -> StorageId {
        self.new_scalar_spill_storage("shift-result", ty, span)
    }
}
