use super::*;
impl<'p> Recipes<'_, 'p> {
    pub fn divide(
        &mut self,
        block: Block<'p>,
        ty: ScalarType,
        result: DivisionResult,
        operands: [ValueRef; 3],
        domain: Domain,
    ) -> Result<Block<'p>> {
        let [a, b, out] = operands;
        let signed = ty == ScalarType::I64;
        let join = self.join(out)?;
        let (ordinary, overflow) = if signed {
            let min = self.constant(block, Constant::I64(i64::MIN))?;
            let minus_one = self.constant(block, Constant::I64(-1))?;
            let is_min = self.cmp(block, Predicate::Equal, true, a, min)?;
            let is_minus_one = self.cmp(block, Predicate::Equal, true, b, minus_one)?;
            let condition = self.alu(block, BinaryOperation::And, is_min, is_minus_one)?;
            let special = self.block()?;
            let ordinary = self.block()?;
            self.branch(block, condition, special, ordinary)?;
            let answer = if result == DivisionResult::Quotient {
                min
            } else {
                self.constant(special, Constant::I64(0))?
            };
            self.jump(special, join, &[answer])?;
            (ordinary, Some(block.id()))
        } else {
            (block, None)
        };
        let low = if ty == ScalarType::U8 {
            self.cell(ordinary, Cell::ZeroExtend, a, ScalarType::U64)?
        } else {
            a
        };
        let divisor = if ty == ScalarType::U8 {
            self.cell(ordinary, Cell::ZeroExtend, b, ScalarType::U64)?
        } else {
            b
        };
        let high = self.value(ScalarType::U64)?;
        self.emit(
            ordinary,
            Opcode::Numeric(Numeric::Dividend { signed, low, high }),
        )?;
        let quotient = self.value(ScalarType::U64)?;
        let remainder = self.value(ScalarType::U64)?;
        self.emit(
            ordinary,
            Opcode::Numeric(Numeric::Divide {
                signed,
                low,
                high,
                divisor,
                quotient,
                remainder,
                domain: Box::new(domain),
                overflow,
            }),
        )?;
        let raw = if result == DivisionResult::Quotient {
            quotient
        } else {
            remainder
        };
        if signed {
            let zero = self.constant(ordinary, Constant::I64(0))?;
            let nonzero = self.cmp(ordinary, Predicate::NotEqual, true, remainder, zero)?;
            let signs = self.alu(ordinary, BinaryOperation::Xor, a, b)?;
            let different = self.cmp(ordinary, Predicate::LessThan, true, signs, zero)?;
            let adjust = self.alu(ordinary, BinaryOperation::And, nonzero, different)?;
            let correction = self.block()?;
            let unchanged = self.block()?;
            self.branch(ordinary, adjust, correction, unchanged)?;
            self.jump(unchanged, join, &[raw])?;
            let corrected = if result == DivisionResult::Quotient {
                let one = self.constant(correction, Constant::I64(1))?;
                self.alu(correction, BinaryOperation::Subtract, quotient, one)?
            } else {
                self.alu(correction, BinaryOperation::Add, remainder, b)?
            };
            self.jump(correction, join, &[corrected])?;
        } else {
            let answer = if ty == ScalarType::U8 {
                self.cell(ordinary, Cell::Narrow, raw, ScalarType::U8)?
            } else {
                raw
            };
            self.jump(ordinary, join, &[answer])?;
        }
        self.emit(
            join,
            Opcode::Numeric(Numeric::Boundary(Recipe::Division {
                ty,
                result,
                dividend: a,
                divisor: b,
                out,
            })),
        )?;
        Ok(join)
    }
}
