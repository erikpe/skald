//! Primitive-cast instruction selection.

use crate::mir::{
    MirF64ToIntegerRange, MirIntegerType, MirPrimitiveCast, MirPrimitiveCastKind, MirPrimitiveType,
    MirTerminator, MirType, ValueId,
};

use super::{
    super::machine::{
        BitwiseOperation, ByteRegister, ConditionCode, Instruction, Label, Operand, Register,
        ShiftOperation, XmmRegister,
    },
    block_label, symbol, value, InstructionSelector,
};

const NEGATIVE_ONE_BITS: u64 = 0xbff0_0000_0000_0000;
const TWO_TO_8_BITS: u64 = 0x4070_0000_0000_0000;
const TWO_TO_63_BITS: u64 = 0x43e0_0000_0000_0000;
const NEGATIVE_TWO_TO_63_BITS: u64 = 0xc3e0_0000_0000_0000;
const TWO_TO_64_BITS: u64 = 0x43f0_0000_0000_0000;

#[derive(Clone, Copy)]
struct F64IntegerBounds {
    lower_bits: u64,
    lower_failure: ConditionCode,
    upper_bits: u64,
}

impl F64IntegerBounds {
    const fn for_target(target: MirIntegerType) -> Self {
        match target {
            MirIntegerType::I64 => Self {
                lower_bits: NEGATIVE_TWO_TO_63_BITS,
                lower_failure: ConditionCode::UnsignedBelow,
                upper_bits: TWO_TO_63_BITS,
            },
            MirIntegerType::U64 => Self {
                lower_bits: NEGATIVE_ONE_BITS,
                lower_failure: ConditionCode::UnsignedBelowEqual,
                upper_bits: TWO_TO_64_BITS,
            },
            MirIntegerType::U8 => Self {
                lower_bits: NEGATIVE_ONE_BITS,
                lower_failure: ConditionCode::UnsignedBelowEqual,
                upper_bits: TWO_TO_8_BITS,
            },
        }
    }
}

impl InstructionSelector<'_, '_> {
    /// Selects the finite and post-truncation range relation before control
    /// can enter the success-only conversion block.
    pub(super) fn select_primitive_cast_terminator(&mut self, terminator: &MirTerminator) -> bool {
        let MirTerminator::PrimitiveCastRangeCheck {
            check,
            success_target,
            failure_target,
            ..
        } = terminator
        else {
            return false;
        };

        let failure = block_label(self.program, *failure_target);
        let bounds = F64IntegerBounds::for_target(check.relation.target);
        value::load_float(
            value::float_operand(value::frame_storage(self.frame, check.source)),
            XmmRegister::Xmm14,
            self.output,
        );

        // An unordered self-comparison identifies every NaN before the
        // ordered threshold checks. Infinities fail one of those thresholds.
        self.output.push(Instruction::CompareFloat64 {
            source: XmmRegister::Xmm14,
            destination: XmmRegister::Xmm14,
        });
        self.output.push(Instruction::JumpIf {
            condition: ConditionCode::Parity,
            target: failure.clone(),
        });

        self.load_f64_bits(bounds.lower_bits, XmmRegister::Xmm15);
        self.output.push(Instruction::CompareFloat64 {
            source: XmmRegister::Xmm15,
            destination: XmmRegister::Xmm14,
        });
        self.output.push(Instruction::JumpIf {
            condition: bounds.lower_failure,
            target: failure.clone(),
        });

        self.load_f64_bits(bounds.upper_bits, XmmRegister::Xmm15);
        self.output.push(Instruction::CompareFloat64 {
            source: XmmRegister::Xmm15,
            destination: XmmRegister::Xmm14,
        });
        self.output.push(Instruction::JumpIf {
            condition: ConditionCode::UnsignedAboveEqual,
            target: failure,
        });
        self.output.push(Instruction::Jump(block_label(
            self.program,
            *success_target,
        )));
        true
    }

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
            MirPrimitiveCastKind::BitReinterpretation => {
                self.select_bit_reinterpretation(operation, operand, destination);
            }
            MirPrimitiveCastKind::CheckedF64ToInteger => {
                unreachable!("checked primitive casts use their dedicated success conversion")
            }
        }
    }

    pub(super) fn select_checked_f64_to_integer(
        &mut self,
        relation: MirF64ToIntegerRange,
        operand: ValueId,
        ty: MirType,
        destination: Operand,
    ) {
        debug_assert_eq!(relation.result_type(), ty);
        value::load_float(
            value::float_operand(value::frame_value(self.frame, operand)),
            XmmRegister::Xmm14,
            self.output,
        );
        if relation.target == MirIntegerType::U64 {
            self.convert_checked_u64();
        } else {
            self.convert_f64_to_signed_rax();
        }
        value::store_canonical_rax(ty, destination, self.output);
    }

    fn convert_checked_u64(&mut self) {
        let signed_domain = self.next_primitive_cast_label("f64_u64_signed_domain");
        let result_ready = self.next_primitive_cast_label("f64_u64_result_ready");

        self.load_f64_bits(TWO_TO_63_BITS, XmmRegister::Xmm15);
        self.output.push(Instruction::CompareFloat64 {
            source: XmmRegister::Xmm15,
            destination: XmmRegister::Xmm14,
        });
        self.output.push(Instruction::JumpIf {
            condition: ConditionCode::UnsignedBelow,
            target: signed_domain.clone(),
        });

        // The verified upper bound makes `(source - 2^63)` a nonnegative
        // signed-domain value. The subtraction is exact for this interval;
        // restoring the high bit yields the complete u64 domain inline.
        self.output.push(Instruction::SubtractFloat64 {
            source: XmmRegister::Xmm15,
            destination: XmmRegister::Xmm14,
        });
        self.convert_f64_to_signed_rax();
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1_u64 << 63,
            destination: Register::R11,
        });
        self.output.push(Instruction::Bitwise {
            operation: BitwiseOperation::Or,
            source: Register::R11,
            destination: Register::Rax,
        });
        self.output.push(Instruction::Jump(result_ready.clone()));

        self.output.push(Instruction::Label(signed_domain));
        self.convert_f64_to_signed_rax();
        self.output.push(Instruction::Label(result_ready));
    }

    fn convert_f64_to_signed_rax(&mut self) {
        self.output
            .push(Instruction::ConvertFloat64ToSignedInteger {
                source: XmmRegister::Xmm14,
                destination: Register::Rax,
            });
    }

    fn load_f64_bits(&mut self, bits: u64, destination: XmmRegister) {
        self.output.push(Instruction::MoveImmediate64 {
            bits,
            destination: Register::Rax,
        });
        self.output.push(Instruction::MoveBitsToFloat {
            source: Register::Rax,
            destination,
        });
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

    fn select_bit_reinterpretation(
        &mut self,
        operation: MirPrimitiveCast,
        operand: ValueId,
        destination: Operand,
    ) {
        match (operation.source, operation.target) {
            (MirPrimitiveType::F64, MirPrimitiveType::U64) => {
                value::load_float(
                    value::float_operand(value::frame_value(self.frame, operand)),
                    XmmRegister::Xmm14,
                    self.output,
                );
                self.output.push(Instruction::MoveFloatBitsToInteger {
                    source: XmmRegister::Xmm14,
                    destination: Register::Rax,
                });
                value::store_canonical_rax(MirType::U64, destination, self.output);
            }
            (MirPrimitiveType::U64, MirPrimitiveType::F64) => {
                value::load_rax(value::frame_value(self.frame, operand), self.output);
                self.output.push(Instruction::MoveBitsToFloat {
                    source: Register::Rax,
                    destination: XmmRegister::Xmm14,
                });
                value::store_float(
                    XmmRegister::Xmm14,
                    value::float_operand(destination),
                    self.output,
                );
            }
            _ => unreachable!("verified bit reinterpretation has an exact f64/u64 pair"),
        }
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
