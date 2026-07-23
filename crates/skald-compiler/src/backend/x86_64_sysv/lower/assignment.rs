//! Assignment and rvalue instruction selection.

use crate::{
    backend::BackendError,
    mir::{
        MirAssignment, MirBinaryOperation, MirPlace, MirRvalueKind, MirType, MirUnaryOperation,
        ValueId,
    },
};

use super::{
    super::machine::{Instruction, Operand, Register, XmmRegister},
    value, InstructionSelector,
};

#[derive(Clone, Copy)]
enum IntegerBinaryOperation {
    Add,
    Subtract,
    Multiply,
}

#[derive(Clone, Copy)]
enum FloatBinaryOperation {
    Add,
    Subtract,
    Multiply,
}

impl InstructionSelector<'_, '_> {
    pub(super) fn select_assignment(
        &mut self,
        assignment: &MirAssignment,
    ) -> Result<(), BackendError> {
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
            MirRvalueKind::TypeTest { .. } => {
                unreachable!("backend legality rejects runtime type tests")
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
        });
        value::store_canonical_rax(ty, destination, self.output);
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
        });
        value::store_float(
            XmmRegister::Xmm14,
            value::float_operand(destination),
            self.output,
        );
    }
}
