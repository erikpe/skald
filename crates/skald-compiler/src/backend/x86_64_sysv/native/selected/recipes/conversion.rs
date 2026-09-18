use super::*;
impl<'p> Recipes<'_, 'p> {
    pub fn convert(
        &mut self,
        block: Block<'p>,
        conversion: Conversion,
        types: (ScalarType, ScalarType),
        input: ValueRef,
        out: ValueRef,
        domain: Option<Domain>,
    ) -> Result<Block<'p>> {
        let (from, to) = types;
        let domain = domain.map(Box::new);
        let recipe = Recipe::Conversion {
            conversion,
            from,
            to,
            source: input,
            out,
        };
        match conversion {
            Conversion::Identity => self.emit(
                block,
                Opcode::Numeric(Numeric::Cell {
                    cell: Cell::Copy,
                    input,
                    out,
                }),
            )?,
            Conversion::IntegerBits | Conversion::FromBoolean => {
                let cell = match (input.representation.bits(), out.representation.bits()) {
                    (8, 64) => Cell::ZeroExtend,
                    (64, 8) => Cell::Narrow,
                    _ => Cell::Copy,
                };
                self.emit(block, Opcode::Numeric(Numeric::Cell { cell, input, out }))?;
            }
            Conversion::FloatBits => self.emit(
                block,
                Opcode::Numeric(Numeric::Cell {
                    cell: Cell::FloatBits,
                    input,
                    out,
                }),
            )?,
            Conversion::ToBoolean => {
                let zero = self.constant(
                    block,
                    match from {
                        ScalarType::F64 => Constant::F64(0),
                        ScalarType::U8 => Constant::U8(0),
                        ScalarType::I64 => Constant::I64(0),
                        _ => Constant::U64(0),
                    },
                )?;
                let boolean = self.cmp(block, Predicate::NotEqual, false, input, zero)?;
                self.emit(
                    block,
                    Opcode::Numeric(Numeric::Cell {
                        cell: Cell::Copy,
                        input: boolean,
                        out,
                    }),
                )?;
            }
            Conversion::ToFloat if from != ScalarType::U64 => {
                let input = if matches!(from, ScalarType::U8 | ScalarType::Bool) {
                    self.cell(block, Cell::ZeroExtend, input, ScalarType::U64)?
                } else {
                    input
                };
                self.emit(
                    block,
                    Opcode::Numeric(Numeric::Cell {
                        cell: Cell::SignedToFloat,
                        input,
                        out,
                    }),
                )?;
            }
            Conversion::ToFloat => {
                let boundary = self.constant(block, Constant::U64(1 << 63))?;
                let lower = self.cmp(block, Predicate::LessThan, false, input, boundary)?;
                let direct = self.block()?;
                let upper = self.block()?;
                let join = self.join(out)?;
                self.branch(block, lower, direct, upper)?;
                let low = self.cell(direct, Cell::SignedToFloat, input, ScalarType::F64)?;
                self.jump(direct, join, &[low])?;
                let half = self.value(ScalarType::U64)?;
                self.emit(
                    upper,
                    Opcode::Numeric(Numeric::ShiftOne { input, out: half }),
                )?;
                let one = self.constant(upper, Constant::U64(1))?;
                let lowbit = self.alu(upper, BinaryOperation::And, input, one)?;
                let rounded = self.alu(upper, BinaryOperation::Or, half, lowbit)?;
                let converted = self.cell(upper, Cell::SignedToFloat, rounded, ScalarType::F64)?;
                let two = self.constant(upper, Constant::F64(0x4000_0000_0000_0000))?;
                let doubled = self.alu(upper, BinaryOperation::Multiply, converted, two)?;
                self.jump(upper, join, &[doubled])?;
                self.emit(join, Opcode::Numeric(Numeric::Boundary(recipe)))?;
                return Ok(join);
            }
            Conversion::TruncateFloat if to != ScalarType::U64 => {
                let wide = if to == ScalarType::U8 {
                    self.value(ScalarType::U64)?
                } else {
                    out
                };
                self.emit(
                    block,
                    Opcode::Numeric(Numeric::CheckedTruncate {
                        input,
                        out: wide,
                        source: input,
                        target: to,
                        domain: domain.ok_or(SelectedBuildError::TypeMismatch)?,
                    }),
                )?;
                if to == ScalarType::U8 {
                    self.emit(
                        block,
                        Opcode::Numeric(Numeric::Cell {
                            cell: Cell::Narrow,
                            input: wide,
                            out,
                        }),
                    )?;
                }
            }
            Conversion::TruncateFloat => {
                let domain = domain.ok_or(SelectedBuildError::TypeMismatch)?;
                let boundary = self.constant(block, Constant::F64(0x43e0_0000_0000_0000))?;
                let lower = self.cmp(block, Predicate::LessThan, false, input, boundary)?;
                let direct = self.block()?;
                let upper = self.block()?;
                let join = self.join(out)?;
                self.branch(block, lower, direct, upper)?;
                let low = self.value(ScalarType::U64)?;
                self.emit(
                    direct,
                    Opcode::Numeric(Numeric::CheckedTruncate {
                        input,
                        out: low,
                        source: input,
                        target: to,
                        domain: domain.clone(),
                    }),
                )?;
                self.jump(direct, join, &[low])?;
                let reduced = self.alu(upper, BinaryOperation::Subtract, input, boundary)?;
                let truncated = self.value(ScalarType::U64)?;
                self.emit(
                    upper,
                    Opcode::Numeric(Numeric::CheckedTruncate {
                        input: reduced,
                        out: truncated,
                        source: input,
                        target: to,
                        domain,
                    }),
                )?;
                let highbit = self.constant(upper, Constant::U64(1 << 63))?;
                let restored = self.alu(upper, BinaryOperation::Xor, truncated, highbit)?;
                self.jump(upper, join, &[restored])?;
                self.emit(join, Opcode::Numeric(Numeric::Boundary(recipe)))?;
                return Ok(join);
            }
            Conversion::PointerBits => unreachable!("pilot excludes pointer casts"),
        }
        self.emit(block, Opcode::Numeric(Numeric::Boundary(recipe)))?;
        Ok(block)
    }
}
