//! Assignment and rvalue instruction selection.

use crate::{
    backend::BackendError,
    mir::{
        MirAssignment, MirBinaryOperation, MirComparisonOperand, MirComparisonPredicate,
        MirIntegerBitwiseOperation, MirIntegerType, MirPlace, MirPrimitiveComparison,
        MirRvalueKind, MirType, MirUnaryOperation, ValueId,
    },
};

use super::{
    super::machine::{
        BitwiseOperation, ByteRegister, ConditionCode, Instruction, Operand, Register, XmmRegister,
    },
    value, InstructionSelector,
};

#[derive(Clone, Copy)]
enum IntegerBinaryOperation {
    Add,
    Subtract,
    Multiply,
    Bitwise(BitwiseOperation),
}

#[derive(Clone, Copy)]
enum FloatBinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl InstructionSelector<'_, '_> {
    pub(super) fn select_assignment(
        &mut self,
        assignment: &MirAssignment,
    ) -> Result<(), BackendError> {
        if let MirRvalueKind::TypeTest { source, target } = &assignment.rvalue.kind {
            self.select_type_test(source, *target, assignment.result);
            return Ok(());
        }
        if let MirRvalueKind::OptionalPresence { source, kind } = &assignment.rvalue.kind {
            return self.select_optional_presence(source, *kind, assignment.result);
        }
        if let MirRvalueKind::ArrayLength { source, .. } = &assignment.rvalue.kind {
            return self.select_array_length(source, assignment.result);
        }
        let destination = value::frame_value(self.frame, assignment.result);
        self.select_rvalue(&assignment.rvalue.kind, assignment.rvalue.ty, destination)
    }

    fn select_rvalue(
        &mut self,
        kind: &MirRvalueKind,
        ty: MirType,
        destination: Operand,
    ) -> Result<(), BackendError> {
        match kind {
            MirRvalueKind::ConstantI64(value) => {
                self.select_integer_constant(*value as u64, ty, destination)
            }
            MirRvalueKind::ConstantU64(value) => {
                self.select_integer_constant(*value, ty, destination)
            }
            MirRvalueKind::ConstantU8(value) => {
                self.select_integer_constant(u64::from(*value), ty, destination)
            }
            MirRvalueKind::ConstantF64Bits(bits) => self.select_float_constant(*bits, destination),
            MirRvalueKind::ConstantBool(value) => {
                self.select_integer_constant(u64::from(*value), ty, destination)
            }
            MirRvalueKind::PathCondition(condition) => {
                self.select_load(&MirPlace::base(condition.activation), ty, destination)?;
            }
            MirRvalueKind::Load(place) => {
                self.select_load(place, ty, destination)?;
            }
            MirRvalueKind::Unary { operation, operand } => {
                self.select_unary(*operation, *operand, ty, destination)
            }
            MirRvalueKind::Binary {
                operation,
                left,
                right,
            } => self.select_binary(*operation, *left, *right, ty, destination),
            MirRvalueKind::IntegerDivision {
                operation,
                dividend,
                divisor,
            } => self.select_integer_division(*operation, *dividend, *divisor, ty, destination),
            MirRvalueKind::Shift {
                operation,
                left,
                count,
            } => self.select_shift(*operation, *left, *count, ty, destination),
            MirRvalueKind::PrimitiveComparison {
                operation,
                left,
                right,
            } => self.select_primitive_comparison(*operation, *left, *right, destination),
            MirRvalueKind::PrimitiveCast { operation, operand } => {
                self.select_primitive_cast(*operation, *operand, destination)
            }
            MirRvalueKind::CheckedF64ToInteger { relation, operand } => {
                self.select_checked_f64_to_integer(*relation, *operand, ty, destination)
            }
            MirRvalueKind::TypeTest { .. } => {
                unreachable!("runtime type tests are selected before ordinary rvalues")
            }
            MirRvalueKind::OptionalPresence { .. } => {
                unreachable!("optional presence tests are selected before ordinary rvalues")
            }
            MirRvalueKind::ArrayLength { .. } => {
                unreachable!("array length is selected before ordinary rvalues")
            }
        }
        Ok(())
    }

    fn select_integer_constant(&mut self, bits: u64, ty: MirType, destination: Operand) {
        self.output.push(Instruction::MoveImmediate64 {
            bits,
            destination: Register::Rax,
        });
        value::store_canonical_rax(ty, destination, self.output);
    }

    fn select_float_constant(&mut self, bits: u64, destination: Operand) {
        self.output.push(Instruction::MoveImmediate64 {
            bits,
            destination: Register::Rax,
        });
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

    fn select_load(
        &mut self,
        place: &MirPlace,
        ty: MirType,
        destination: Operand,
    ) -> Result<(), BackendError> {
        let (source_layout, source) = self.frame_place(place)?;
        debug_assert_eq!(source_layout.ty(), ty);
        if ty == MirType::F64 {
            value::load_float(
                value::float_operand(source),
                XmmRegister::Xmm14,
                self.output,
            );
            value::store_float(
                XmmRegister::Xmm14,
                value::float_operand(destination),
                self.output,
            );
        } else {
            if source_layout.uses_byte_access() {
                value::load_byte_rax(source, self.output);
            } else {
                value::load_rax(source, self.output);
            }
            value::store_canonical_rax(ty, destination, self.output);
        }
        Ok(())
    }

    fn select_unary(
        &mut self,
        operation: MirUnaryOperation,
        operand: ValueId,
        ty: MirType,
        destination: Operand,
    ) {
        match operation {
            MirUnaryOperation::NegateI64 => {
                value::load_rax(value::frame_value(self.frame, operand), self.output);
                self.output.push(Instruction::Negate(Register::Rax));
                value::store_canonical_rax(ty, destination, self.output);
            }
            MirUnaryOperation::NegateF64 => self.select_float_negate(operand, destination),
            MirUnaryOperation::LogicalNotBool => {
                value::load_rax(value::frame_value(self.frame, operand), self.output);
                self.output.push(Instruction::Test(Register::Rax));
                self.store_condition(ConditionCode::Equal, destination);
            }
            MirUnaryOperation::BitwiseComplement(_) => {
                value::load_rax(value::frame_value(self.frame, operand), self.output);
                self.output.push(Instruction::BitwiseNot(Register::Rax));
                value::store_canonical_rax(ty, destination, self.output);
            }
        }
    }

    fn select_float_negate(&mut self, operand: ValueId, destination: Operand) {
        value::load_float(
            value::float_operand(value::frame_value(self.frame, operand)),
            XmmRegister::Xmm14,
            self.output,
        );
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1_u64 << 63,
            destination: Register::Rax,
        });
        self.output.push(Instruction::MoveBitsToFloat {
            source: Register::Rax,
            destination: XmmRegister::Xmm15,
        });
        self.output.push(Instruction::XorFloat128 {
            source: XmmRegister::Xmm15,
            destination: XmmRegister::Xmm14,
        });
        value::store_float(
            XmmRegister::Xmm14,
            value::float_operand(destination),
            self.output,
        );
    }

    fn select_binary(
        &mut self,
        operation: MirBinaryOperation,
        left: ValueId,
        right: ValueId,
        ty: MirType,
        destination: Operand,
    ) {
        match operation {
            MirBinaryOperation::AddI64 | MirBinaryOperation::AddU64 | MirBinaryOperation::AddU8 => {
                self.select_integer_binary(
                    IntegerBinaryOperation::Add,
                    left,
                    right,
                    ty,
                    destination,
                )
            }
            MirBinaryOperation::SubtractI64
            | MirBinaryOperation::SubtractU64
            | MirBinaryOperation::SubtractU8 => self.select_integer_binary(
                IntegerBinaryOperation::Subtract,
                left,
                right,
                ty,
                destination,
            ),
            MirBinaryOperation::MultiplyI64
            | MirBinaryOperation::MultiplyU64
            | MirBinaryOperation::MultiplyU8 => self.select_integer_binary(
                IntegerBinaryOperation::Multiply,
                left,
                right,
                ty,
                destination,
            ),
            MirBinaryOperation::AddF64 => {
                self.select_float_binary(FloatBinaryOperation::Add, left, right, destination)
            }
            MirBinaryOperation::SubtractF64 => {
                self.select_float_binary(FloatBinaryOperation::Subtract, left, right, destination)
            }
            MirBinaryOperation::MultiplyF64 => {
                self.select_float_binary(FloatBinaryOperation::Multiply, left, right, destination)
            }
            MirBinaryOperation::DivideF64 => {
                self.select_float_binary(FloatBinaryOperation::Divide, left, right, destination)
            }
            MirBinaryOperation::IntegerBitwise { operation, .. } => self.select_integer_binary(
                IntegerBinaryOperation::Bitwise(match operation {
                    MirIntegerBitwiseOperation::And => BitwiseOperation::And,
                    MirIntegerBitwiseOperation::Or => BitwiseOperation::Or,
                    MirIntegerBitwiseOperation::Xor => BitwiseOperation::Xor,
                }),
                left,
                right,
                ty,
                destination,
            ),
        }
    }

    fn select_integer_binary(
        &mut self,
        operation: IntegerBinaryOperation,
        left: ValueId,
        right: ValueId,
        ty: MirType,
        destination: Operand,
    ) {
        value::load_rax(value::frame_value(self.frame, left), self.output);
        self.output.push(Instruction::Move {
            source: value::frame_value(self.frame, right),
            destination: Register::Rcx.into(),
        });
        self.output.push(match operation {
            IntegerBinaryOperation::Add => Instruction::Add {
                source: Register::Rcx,
                destination: Register::Rax,
            },
            IntegerBinaryOperation::Subtract => Instruction::Subtract {
                source: Register::Rcx,
                destination: Register::Rax,
            },
            IntegerBinaryOperation::Multiply => Instruction::Multiply {
                source: Register::Rcx,
                destination: Register::Rax,
            },
            IntegerBinaryOperation::Bitwise(operation) => Instruction::Bitwise {
                operation,
                source: Register::Rcx,
                destination: Register::Rax,
            },
        });
        value::store_canonical_rax(ty, destination, self.output);
    }

    fn select_primitive_comparison(
        &mut self,
        operation: MirPrimitiveComparison,
        left: ValueId,
        right: ValueId,
        destination: Operand,
    ) {
        if operation.operand == MirComparisonOperand::F64 {
            self.select_float_comparison(operation.predicate, left, right, destination);
            return;
        }

        value::load_rax(value::frame_value(self.frame, left), self.output);
        self.output.push(Instruction::Move {
            source: value::frame_value(self.frame, right),
            destination: Register::Rcx.into(),
        });
        self.output.push(Instruction::Compare {
            source: Register::Rcx,
            destination: Register::Rax,
        });
        self.store_condition(comparison_condition(operation), destination);
    }

    fn select_float_comparison(
        &mut self,
        predicate: MirComparisonPredicate,
        left: ValueId,
        right: ValueId,
        destination: Operand,
    ) {
        value::load_float(
            value::float_operand(value::frame_value(self.frame, left)),
            XmmRegister::Xmm14,
            self.output,
        );
        value::load_float(
            value::float_operand(value::frame_value(self.frame, right)),
            XmmRegister::Xmm15,
            self.output,
        );
        self.output.push(Instruction::CompareFloat64 {
            source: XmmRegister::Xmm15,
            destination: XmmRegister::Xmm14,
        });

        let (relation, ordered, combine) = floating_comparison_conditions(predicate);
        self.output.push(Instruction::SetCondition {
            condition: relation,
            destination: ByteRegister::Al,
        });
        self.output.push(Instruction::SetCondition {
            condition: ordered,
            destination: ByteRegister::Cl,
        });
        self.output.push(Instruction::ByteBitwise {
            operation: combine,
            source: ByteRegister::Cl,
            destination: ByteRegister::Al,
        });
        self.output.push(Instruction::ZeroExtendByte {
            source: ByteRegister::Al,
            destination: Register::Rax,
        });
        value::store_canonical_rax(MirType::Bool, destination, self.output);
    }

    fn store_condition(&mut self, condition: ConditionCode, destination: Operand) {
        self.output.push(Instruction::SetCondition {
            condition,
            destination: ByteRegister::Al,
        });
        self.output.push(Instruction::ZeroExtendByte {
            source: ByteRegister::Al,
            destination: Register::Rax,
        });
        value::store_canonical_rax(MirType::Bool, destination, self.output);
    }

    fn select_float_binary(
        &mut self,
        operation: FloatBinaryOperation,
        left: ValueId,
        right: ValueId,
        destination: Operand,
    ) {
        value::load_float(
            value::float_operand(value::frame_value(self.frame, left)),
            XmmRegister::Xmm14,
            self.output,
        );
        value::load_float(
            value::float_operand(value::frame_value(self.frame, right)),
            XmmRegister::Xmm15,
            self.output,
        );
        self.output.push(match operation {
            FloatBinaryOperation::Add => Instruction::AddFloat64 {
                source: XmmRegister::Xmm15,
                destination: XmmRegister::Xmm14,
            },
            FloatBinaryOperation::Subtract => Instruction::SubtractFloat64 {
                source: XmmRegister::Xmm15,
                destination: XmmRegister::Xmm14,
            },
            FloatBinaryOperation::Multiply => Instruction::MultiplyFloat64 {
                source: XmmRegister::Xmm15,
                destination: XmmRegister::Xmm14,
            },
            FloatBinaryOperation::Divide => Instruction::DivideFloat64 {
                source: XmmRegister::Xmm15,
                destination: XmmRegister::Xmm14,
            },
        });
        value::store_float(
            XmmRegister::Xmm14,
            value::float_operand(destination),
            self.output,
        );
    }
}

fn comparison_condition(operation: MirPrimitiveComparison) -> ConditionCode {
    debug_assert_ne!(operation.operand, MirComparisonOperand::F64);
    match operation.predicate {
        MirComparisonPredicate::Equal => ConditionCode::Equal,
        MirComparisonPredicate::NotEqual => ConditionCode::NotEqual,
        MirComparisonPredicate::LessThan => ordering_condition(
            operation.operand,
            ConditionCode::SignedLess,
            ConditionCode::UnsignedBelow,
        ),
        MirComparisonPredicate::LessEqual => ordering_condition(
            operation.operand,
            ConditionCode::SignedLessEqual,
            ConditionCode::UnsignedBelowEqual,
        ),
        MirComparisonPredicate::GreaterThan => ordering_condition(
            operation.operand,
            ConditionCode::SignedGreater,
            ConditionCode::UnsignedAbove,
        ),
        MirComparisonPredicate::GreaterEqual => ordering_condition(
            operation.operand,
            ConditionCode::SignedGreaterEqual,
            ConditionCode::UnsignedAboveEqual,
        ),
    }
}

fn floating_comparison_conditions(
    predicate: MirComparisonPredicate,
) -> (ConditionCode, ConditionCode, BitwiseOperation) {
    match predicate {
        MirComparisonPredicate::Equal => (
            ConditionCode::Equal,
            ConditionCode::NotParity,
            BitwiseOperation::And,
        ),
        MirComparisonPredicate::NotEqual => (
            ConditionCode::NotEqual,
            ConditionCode::Parity,
            BitwiseOperation::Or,
        ),
        MirComparisonPredicate::LessThan => (
            ConditionCode::UnsignedBelow,
            ConditionCode::NotParity,
            BitwiseOperation::And,
        ),
        MirComparisonPredicate::LessEqual => (
            ConditionCode::UnsignedBelowEqual,
            ConditionCode::NotParity,
            BitwiseOperation::And,
        ),
        MirComparisonPredicate::GreaterThan => (
            ConditionCode::UnsignedAbove,
            ConditionCode::NotParity,
            BitwiseOperation::And,
        ),
        MirComparisonPredicate::GreaterEqual => (
            ConditionCode::UnsignedAboveEqual,
            ConditionCode::NotParity,
            BitwiseOperation::And,
        ),
    }
}

fn ordering_condition(
    operand: MirComparisonOperand,
    signed: ConditionCode,
    unsigned: ConditionCode,
) -> ConditionCode {
    match operand {
        MirComparisonOperand::Integer(MirIntegerType::I64) => signed,
        MirComparisonOperand::Integer(MirIntegerType::U64 | MirIntegerType::U8) => unsigned,
        MirComparisonOperand::F64 => {
            unreachable!("floating comparisons use explicit unordered lowering")
        }
        MirComparisonOperand::Bool => {
            unreachable!("verified boolean comparisons cannot use ordering")
        }
    }
}
