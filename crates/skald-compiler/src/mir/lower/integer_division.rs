//! Checked integer division and remainder lowering.

use crate::{
    hir::{HirCheckedIntegerDivision, HirExpression, HirIntegerDivisionKind},
    mir::{
        MirInstruction, MirIntegerDivisionKind, MirIntegerDivisionOperation,
        MirIntegerDivisorCheck, MirPlace, MirRvalueKind, MirStore, MirTerminator, StorageId,
        ValueId,
    },
};

use super::{expression::lower_integer_type, BodyLowerer};

impl BodyLowerer<'_> {
    pub(super) fn lower_checked_integer_division(
        &mut self,
        expression: &HirExpression,
        division: &HirCheckedIntegerDivision,
    ) -> ValueId {
        division.validate(expression.ty);
        let operation = MirIntegerDivisionOperation {
            kind: match division.operation.kind {
                HirIntegerDivisionKind::Quotient => MirIntegerDivisionKind::Quotient,
                HirIntegerDivisionKind::Remainder => MirIntegerDivisionKind::Remainder,
            },
            operand: lower_integer_type(division.operation.operand),
        };

        // Evaluation and securing order is semantic. Each operand completes
        // before its value is copied into a carrier that crosses the check.
        let dividend = self
            .lower_expression(&division.dividend)
            .expect("typed integer dividend must produce a value");
        let (dividend, _) =
            self.spill_scalar(dividend, operation.operand_type(), division.dividend.span);
        let divisor = self
            .lower_expression(&division.divisor)
            .expect("typed integer divisor must produce a value");
        let (divisor, _) =
            self.spill_scalar(divisor, operation.operand_type(), division.divisor.span);
        let result = self.new_integer_division_result(operation.result_type(), expression.span);

        let success = self.body.allocate_block(expression.span);
        let failure = self.body.allocate_block(expression.span);
        let join = self.body.allocate_block(expression.span);
        self.terminate(MirTerminator::IntegerDivisorCheck {
            check: MirIntegerDivisorCheck {
                operation,
                dividend,
                divisor,
                result,
            },
            success_target: success,
            failure_target: failure,
            span: expression.span,
        });

        self.body
            .select_block(success)
            .expect("allocated integer-division success block must be selectable");
        let dividend_value = self.assign(
            MirRvalueKind::Load(MirPlace::base(dividend)),
            operation.operand_type(),
            division.dividend.span,
        );
        let divisor_value = self.assign(
            MirRvalueKind::Load(MirPlace::base(divisor)),
            operation.operand_type(),
            division.divisor.span,
        );
        let value = self.assign(
            MirRvalueKind::IntegerDivision {
                operation,
                dividend: dividend_value,
                divisor: divisor_value,
            },
            operation.result_type(),
            expression.span,
        );
        self.emit(MirInstruction::Store(MirStore {
            destination: MirPlace::base(result),
            value,
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
            .expect("allocated integer-division failure block must be selectable");
        self.terminate(MirTerminator::Terminate {
            reason: operation.failure_reason(),
            span: expression.span,
        });

        self.body
            .select_block(join)
            .expect("allocated integer-division result join must be selectable");
        self.load_checked_scalar_result(result, operation.result_type(), expression.span)
    }

    fn new_integer_division_result(
        &mut self,
        ty: crate::mir::MirType,
        span: crate::source::Span,
    ) -> StorageId {
        self.new_scalar_spill_storage("integer-division-result", ty, span)
    }
}
