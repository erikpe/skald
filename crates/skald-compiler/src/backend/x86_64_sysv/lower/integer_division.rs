//! x86-64 selection for verified checked integer division and remainder.

use crate::mir::{
    MirIntegerDivisionKind, MirIntegerDivisionOperation, MirIntegerType, MirTerminator, MirType,
    ValueId,
};

use super::{
    super::machine::{
        BitwiseOperation, Instruction, IntegerDivideFlavor, Label, Operand, Register,
    },
    block_label, symbol, value, InstructionSelector,
};

impl InstructionSelector<'_, '_> {
    /// Selects the verified semantic zero check before the success block can
    /// place a divisor in the fixed divide registers.
    pub(super) fn select_integer_division_terminator(
        &mut self,
        terminator: &MirTerminator,
    ) -> bool {
        let MirTerminator::IntegerDivisorCheck {
            check,
            success_target,
            failure_target,
            ..
        } = terminator
        else {
            return false;
        };

        value::load_rax(value::frame_storage(self.frame, check.divisor), self.output);
        self.output.push(Instruction::Test(Register::Rax));
        self.output.push(Instruction::JumpIfNotZero(block_label(
            self.program,
            *success_target,
        )));
        self.output.push(Instruction::Jump(block_label(
            self.program,
            *failure_target,
        )));
        true
    }

    pub(super) fn select_integer_division(
        &mut self,
        operation: MirIntegerDivisionOperation,
        dividend: ValueId,
        divisor: ValueId,
        ty: MirType,
        destination: Operand,
    ) {
        value::load_rax(value::frame_value(self.frame, dividend), self.output);
        self.output.push(Instruction::Move {
            source: value::frame_value(self.frame, divisor),
            destination: Register::Rcx.into(),
        });

        match operation.operand {
            MirIntegerType::I64 => self.select_signed_integer_division(operation),
            MirIntegerType::U64 | MirIntegerType::U8 => {
                self.select_unsigned_integer_division(operation.kind)
            }
        }
        value::store_canonical_rax(ty, destination, self.output);
    }

    fn select_unsigned_integer_division(&mut self, kind: MirIntegerDivisionKind) {
        // `div` consumes RDX:RAX. A zero high half is the exact 64-bit
        // unsigned dividend and also prevents a quotient-overflow trap.
        self.output.push(Instruction::Bitwise {
            operation: BitwiseOperation::Xor,
            source: Register::Rdx,
            destination: Register::Rdx,
        });
        self.output.push(Instruction::IntegerDivide {
            flavor: IntegerDivideFlavor::Unsigned,
            divisor: Register::Rcx,
        });
        if kind == MirIntegerDivisionKind::Remainder {
            self.move_remainder_to_result();
        }
    }

    fn select_signed_integer_division(&mut self, operation: MirIntegerDivisionOperation) {
        let ordinary = self.next_integer_division_label("ordinary");
        let correction_done = self.next_integer_division_label("correction_done");
        let result_ready = self.next_integer_division_label("result_ready");

        // x86 traps on MIN / -1 even though the language defines this pair.
        // Recognize it before `idiv` and synthesize the exact semantic result.
        self.output.push(Instruction::MoveImmediate64 {
            bits: i64::MIN as u64,
            destination: Register::R11,
        });
        self.output.push(Instruction::Compare {
            source: Register::R11,
            destination: Register::Rax,
        });
        self.output
            .push(Instruction::JumpIfNotZero(ordinary.clone()));
        self.output.push(Instruction::MoveImmediate64 {
            bits: (-1_i64) as u64,
            destination: Register::R11,
        });
        self.output.push(Instruction::Compare {
            source: Register::R11,
            destination: Register::Rcx,
        });
        self.output
            .push(Instruction::JumpIfNotZero(ordinary.clone()));
        self.output.push(Instruction::MoveImmediate64 {
            bits: match operation.kind {
                MirIntegerDivisionKind::Quotient => i64::MIN as u64,
                MirIntegerDivisionKind::Remainder => 0,
            },
            destination: Register::Rax,
        });
        self.output.push(Instruction::Jump(result_ready.clone()));

        self.output.push(Instruction::Label(ordinary));
        self.output.push(Instruction::SignExtendDividend);
        self.output.push(Instruction::IntegerDivide {
            flavor: IntegerDivideFlavor::Signed,
            divisor: Register::Rcx,
        });

        // `idiv` truncates toward zero and gives the remainder the dividend's
        // sign. Floor division differs exactly when a nonzero remainder and
        // the divisor have opposite signs.
        self.output.push(Instruction::Test(Register::Rdx));
        self.output
            .push(Instruction::JumpIfEqual(correction_done.clone()));
        self.output.push(Instruction::Move {
            source: Register::Rdx.into(),
            destination: Register::R11.into(),
        });
        self.output.push(Instruction::Bitwise {
            operation: BitwiseOperation::Xor,
            source: Register::Rcx,
            destination: Register::R11,
        });
        self.output
            .push(Instruction::JumpIfNotSign(correction_done.clone()));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R10,
        });
        self.output.push(Instruction::Subtract {
            source: Register::R10,
            destination: Register::Rax,
        });
        self.output.push(Instruction::Add {
            source: Register::Rcx,
            destination: Register::Rdx,
        });
        self.output.push(Instruction::Label(correction_done));

        if operation.kind == MirIntegerDivisionKind::Remainder {
            self.move_remainder_to_result();
        }
        self.output.push(Instruction::Label(result_ready));
    }

    fn move_remainder_to_result(&mut self) {
        self.output.push(Instruction::Move {
            source: Register::Rdx.into(),
            destination: Register::Rax.into(),
        });
    }

    fn next_integer_division_label(&mut self, suffix: &str) -> Label {
        let sequence = self.integer_division_sequence;
        self.integer_division_sequence += 1;
        Label::new(format!(
            ".Lska.{}.integer_division_{}_{}_{}",
            symbol::local_label_stem(self.program, self.function.callable()),
            self.block.index(),
            sequence,
            suffix
        ))
    }
}
