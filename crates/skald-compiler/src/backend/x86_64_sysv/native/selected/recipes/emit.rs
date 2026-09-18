//! Selection-time construction over values and blocks owned by this builder.
use super::*;
impl<'p> Recipes<'_, 'p> {
    pub(super) fn value(&mut self, ty: ScalarType) -> Result<ValueRef> {
        let representation =
            Representation::from_scalar(ty, 64).ok_or(SelectedBuildError::TypeMismatch)?;
        let value = self.builder.value(representation, self.origin.span)?.id();
        Ok(ValueRef {
            value,
            representation,
        })
    }
    pub(super) fn emit(&mut self, block: Block<'p>, op: Opcode) -> Result<()> {
        self.builder
            .append(block, Instruction::new(op, self.origin, self.resources))
    }
    pub(super) fn cell(
        &mut self,
        block: Block<'p>,
        cell: Cell,
        input: ValueRef,
        ty: ScalarType,
    ) -> Result<ValueRef> {
        let out = self.value(ty)?;
        self.emit(block, Opcode::Numeric(Numeric::Cell { cell, input, out }))?;
        Ok(out)
    }
    pub(super) fn constant(&mut self, block: Block<'p>, constant: Constant) -> Result<ValueRef> {
        let out = self.value(
            constant
                .scalar_type()
                .map_err(|_| SelectedBuildError::TypeMismatch)?,
        )?;
        self.emit(block, Opcode::Constant { constant, out })?;
        Ok(out)
    }
    pub(super) fn alu(
        &mut self,
        block: Block<'p>,
        operation: BinaryOperation,
        left: ValueRef,
        right: ValueRef,
    ) -> Result<ValueRef> {
        let out = self.value(
            if left.representation.kind == crate::backend::selected::RepresentationKind::Float {
                ScalarType::F64
            } else if left.representation.bits() == 8 {
                ScalarType::U8
            } else {
                ScalarType::U64
            },
        )?;
        self.emit(
            block,
            Opcode::Alu {
                operation,
                left,
                right,
                out,
            },
        )?;
        Ok(out)
    }
    pub(super) fn cmp(
        &mut self,
        block: Block<'p>,
        predicate: Predicate,
        signed: bool,
        left: ValueRef,
        right: ValueRef,
    ) -> Result<ValueRef> {
        let out = self.value(ScalarType::Bool)?;
        let op = if left.representation.kind == crate::backend::selected::RepresentationKind::Float
        {
            Opcode::FloatCompare {
                condition: super::super::verify::float_condition(predicate),
                predicate,
                left,
                right,
                out,
            }
        } else {
            Opcode::IntegerCompare {
                condition: super::super::verify::integer_condition(predicate, signed),
                predicate,
                signed,
                left,
                right,
                out,
            }
        };
        self.emit(block, op)?;
        Ok(out)
    }
    pub(super) fn branch(
        &mut self,
        block: Block<'p>,
        condition: ValueRef,
        yes: Block<'p>,
        no: Block<'p>,
    ) -> Result<()> {
        self.builder.terminate(
            block,
            Instruction::new(Opcode::Branch { condition }, self.origin, self.resources),
            &[(yes, vec![]), (no, vec![])],
        )
    }
    pub(super) fn jump(
        &mut self,
        block: Block<'p>,
        to: Block<'p>,
        args: &[ValueRef],
    ) -> Result<()> {
        let args = args
            .iter()
            .map(|v| self.builder.value_handle(v.value))
            .collect::<Result<Vec<_>>>()?;
        self.builder.terminate(
            block,
            Instruction::new(Opcode::Jump, self.origin, self.resources),
            &[(to, args)],
        )
    }
    pub(super) fn block(&mut self) -> Result<Block<'p>> {
        self.builder.block(&[], self.origin.span)
    }
    pub(super) fn join(&mut self, out: ValueRef) -> Result<Block<'p>> {
        self.builder
            .block(&[self.builder.value_handle(out.value)?], self.origin.span)
    }
    pub fn check(
        &mut self,
        block: Block<'p>,
        relation: &ScalarCheck<SelectedValueId>,
        source: ValueRef,
    ) -> Result<ValueRef> {
        Ok(match *relation {
            ScalarCheck::NonZeroDivisor { ty, .. } => {
                let zero = self.constant(
                    block,
                    match ty {
                        ScalarType::I64 => Constant::I64(0),
                        ScalarType::U64 => Constant::U64(0),
                        ScalarType::U8 => Constant::U8(0),
                        _ => unreachable!(),
                    },
                )?;
                self.cmp(block, Predicate::NotEqual, false, source, zero)?
            }
            ScalarCheck::ShiftCountBelowWidth { width, .. } => {
                let bound = self.constant(block, Constant::U64(u64::from(width)))?;
                self.cmp(block, Predicate::LessThan, false, source, bound)?
            }
            ScalarCheck::FiniteTruncatedF64InIntegerRange { target, .. } => {
                // Exact binary64 encodings: [-2^63, 2^63), (-1, 2^64), (-1, 256).
                let (lo, hi, inclusive) = match target {
                    ScalarType::I64 => (0xc3e0_0000_0000_0000, 0x43e0_0000_0000_0000, true),
                    ScalarType::U64 => (0xbff0_0000_0000_0000, 0x43f0_0000_0000_0000, false),
                    ScalarType::U8 => (0xbff0_0000_0000_0000, 0x4070_0000_0000_0000, false),
                    _ => unreachable!(),
                };
                let lo = self.constant(block, Constant::F64(lo))?;
                let hi = self.constant(block, Constant::F64(hi))?;
                let a = self.cmp(
                    block,
                    if inclusive {
                        Predicate::GreaterEqual
                    } else {
                        Predicate::GreaterThan
                    },
                    false,
                    source,
                    lo,
                )?;
                let b = self.cmp(block, Predicate::LessThan, false, source, hi)?;
                self.alu(block, BinaryOperation::And, a, b)?
            }
        })
    }
    pub fn shift(
        &mut self,
        block: Block<'p>,
        direction: ShiftDirection,
        input: ValueRef,
        count: ValueRef,
        out: ValueRef,
        domain: Domain,
    ) -> Result<()> {
        let count = self.cell(block, Cell::Narrow, count, ScalarType::U8)?;
        self.emit(
            block,
            Opcode::Numeric(Numeric::Shift {
                direction,
                input,
                count,
                out,
                domain: Box::new(domain),
            }),
        )?;
        Ok(())
    }
}
