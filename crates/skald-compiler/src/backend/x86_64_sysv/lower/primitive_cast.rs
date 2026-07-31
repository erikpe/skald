//! Primitive-cast instruction selection.

use crate::mir::{MirPrimitiveCast, MirPrimitiveCastKind, MirPrimitiveType, MirType, ValueId};

use super::{
    super::machine::{
        BitwiseOperation, ByteRegister, ConditionCode, Instruction, Label, Operand, Register,
        ShiftOperation, XmmRegister,
    },
    symbol, value, InstructionSelector,
};

impl InstructionSelector<'_, '_> {
    pub(super) fn select_primitive_cast(
        &mut self,
        operation: MirPrimitiveCast,
        operand: ValueId,
        destination: Operand,
    ) {
        match operation.kind() {
            MirPrimitiveCastKind::Identity if operation.source == MirPrimitiveType::F64 => {
                self.select_f64_identity(operand, destination);
            }
            MirPrimitiveCastKind::Identity | MirPrimitiveCastKind::IntegerBits => {
                self.select_integer_class_bits(operation, operand, destination);
            }
            MirPrimitiveCastKind::ToBool if operation.source == MirPrimitiveType::F64 => {
                self.select_f64_to_bool(operand, destination);
            }
            MirPrimitiveCastKind::ToBool => {
                self.select_integer_to_bool(operand, destination);
            }
            MirPrimitiveCastKind::FromBool => {
                self.select_bool_to_integer(operation, operand, destination);
            }
            MirPrimitiveCastKind::ToF64 if operation.source == MirPrimitiveType::Bool => {
                self.select_bool_to_f64(operand, destination);
            }
            MirPrimitiveCastKind::ToF64 => {
                self.select_integer_to_f64(operation.source, operand, destination);
            }
            MirPrimitiveCastKind::CheckedF64ToInteger => {
                unreachable!("target legality rejects unsupported primitive casts")
            }
        }
    }

    fn select_f64_identity(&mut self, operand: ValueId, destination: Operand) {
        value::load_float(
            value::float_operand(value::frame_value(self.frame, operand)),
            XmmRegister::Xmm14,
            self.output,
        );
        value::store_float(
            XmmRegister::Xmm14,
            value::float_operand(destination),
            self.output,
        );
    }

    fn select_integer_class_bits(
        &mut self,
        operation: MirPrimitiveCast,
        operand: ValueId,
        destination: Operand,
    ) {
        debug_assert!(
            (operation.source.is_integer() && operation.target.is_integer())
                || (operation.source == MirPrimitiveType::Bool
                    && operation.target == MirPrimitiveType::Bool)
        );
        // Verified integer-class values use canonical eight-byte homes.
        // Loading the complete home preserves every i64/u64 bit and canonical
        // u8/bool value. The target store applies width canonicalization.
        value::load_rax(value::frame_value(self.frame, operand), self.output);
        value::store_canonical_rax(operation.result_type(), destination, self.output);
    }

    fn select_integer_to_bool(&mut self, operand: ValueId, destination: Operand) {
        value::load_rax(value::frame_value(self.frame, operand), self.output);
        self.output.push(Instruction::Test(Register::Rax));
        self.store_boolean_condition(ConditionCode::NotEqual, destination);
    }

    fn select_f64_to_bool(&mut self, operand: ValueId, destination: Operand) {
        value::load_float(
            value::float_operand(value::frame_value(self.frame, operand)),
            XmmRegister::Xmm14,
            self.output,
        );
        self.output.push(Instruction::XorFloat128 {
            source: XmmRegister::Xmm15,
            destination: XmmRegister::Xmm15,
        });
        self.output.push(Instruction::CompareFloat64 {
            source: XmmRegister::Xmm15,
            destination: XmmRegister::Xmm14,
        });
        // `setne` recognizes ordered nonzero values. `setp` explicitly adds
        // unordered values so every NaN is true while both zeroes are false.
        self.output.push(Instruction::SetCondition {
            condition: ConditionCode::NotEqual,
            destination: ByteRegister::Al,
        });
        self.output.push(Instruction::SetCondition {
            condition: ConditionCode::Parity,
            destination: ByteRegister::Cl,
        });
        self.output.push(Instruction::ByteBitwise {
            operation: BitwiseOperation::Or,
            source: ByteRegister::Cl,
            destination: ByteRegister::Al,
        });
        self.canonicalize_boolean(destination);
    }

    fn select_bool_to_integer(
        &mut self,
        operation: MirPrimitiveCast,
        operand: ValueId,
        destination: Operand,
    ) {
        debug_assert!(operation.target.is_integer());
        value::load_rax(value::frame_value(self.frame, operand), self.output);
        value::store_canonical_rax(operation.result_type(), destination, self.output);
    }

    fn select_bool_to_f64(&mut self, operand: ValueId, destination: Operand) {
        value::load_rax(value::frame_value(self.frame, operand), self.output);
        self.convert_signed_rax_to_f64();
        self.store_f64(destination);
    }

    fn select_integer_to_f64(
        &mut self,
        source: MirPrimitiveType,
        operand: ValueId,
        destination: Operand,
    ) {
        debug_assert!(source.is_integer());
        value::load_rax(value::frame_value(self.frame, operand), self.output);
        if source == MirPrimitiveType::U64 {
            self.convert_u64_rax_to_f64();
        } else {
            // Canonical u8 values fit the signed domain exactly, so the same
            // conversion path covers them without weakening their invariant.
            self.convert_signed_rax_to_f64();
        }
        self.store_f64(destination);
    }

    fn convert_u64_rax_to_f64(&mut self) {
        let signed_domain = self.next_primitive_cast_label("u64_signed_domain");
        let result_ready = self.next_primitive_cast_label("u64_result_ready");

        self.output.push(Instruction::Test(Register::Rax));
        self.output
            .push(Instruction::JumpIfNotSign(signed_domain.clone()));

        // For the high unsigned half, convert a half-sized value whose low bit
        // records whether the discarded bit was nonzero, then double it. This
        // sticky-bit form preserves direct round-to-even behavior while fitting
        // the positive signed domain, including at u64::MAX.
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdx.into(),
        });
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::Rcx,
        });
        self.output.push(Instruction::Shift {
            operation: ShiftOperation::LogicalRight,
            destination: Register::Rax,
        });
        self.output.push(Instruction::Bitwise {
            operation: BitwiseOperation::And,
            source: Register::Rcx,
            destination: Register::Rdx,
        });
        self.output.push(Instruction::Bitwise {
            operation: BitwiseOperation::Or,
            source: Register::Rdx,
            destination: Register::Rax,
        });
        self.convert_signed_rax_to_f64();
        self.output.push(Instruction::AddFloat64 {
            source: XmmRegister::Xmm14,
            destination: XmmRegister::Xmm14,
        });
        self.output.push(Instruction::Jump(result_ready.clone()));

        self.output.push(Instruction::Label(signed_domain));
        self.convert_signed_rax_to_f64();
        self.output.push(Instruction::Label(result_ready));
    }

    fn convert_signed_rax_to_f64(&mut self) {
        self.output
            .push(Instruction::ConvertSignedIntegerToFloat64 {
                source: Register::Rax,
                destination: XmmRegister::Xmm14,
            });
    }

    fn store_f64(&mut self, destination: Operand) {
        value::store_float(
            XmmRegister::Xmm14,
            value::float_operand(destination),
            self.output,
        );
    }

    fn next_primitive_cast_label(&mut self, suffix: &str) -> Label {
        let sequence = self.primitive_cast_sequence;
        self.primitive_cast_sequence += 1;
        Label::new(format!(
            ".Lska.{}.primitive_cast_{}_{}_{}",
            symbol::local_label_stem(self.program, self.function.callable()),
            self.block.index(),
            sequence,
            suffix
        ))
    }

    fn store_boolean_condition(&mut self, condition: ConditionCode, destination: Operand) {
        self.output.push(Instruction::SetCondition {
            condition,
            destination: ByteRegister::Al,
        });
        self.canonicalize_boolean(destination);
    }

    fn canonicalize_boolean(&mut self, destination: Operand) {
        self.output.push(Instruction::ZeroExtendByte {
            source: ByteRegister::Al,
            destination: Register::Rax,
        });
        value::store_canonical_rax(MirType::Bool, destination, self.output);
    }
}
