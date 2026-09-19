//! CFG frame state and original callee bits: finite meet, then strict replay.
use super::super::{super::Gpr, model::*};
use super::{PhysicalError, Reason};
use crate::backend::plan::LirCallableId;
use std::collections::{BTreeMap, VecDeque};
#[derive(Clone, Debug, Eq, PartialEq)]
struct State {
    sp: i64,
    fp: Option<i64>,
    original_fp: bool,
    header: bool,
    registers: [Option<Gpr>; 16],
    simd: [Option<Gpr>; 16],
    memory: BTreeMap<i64, Gpr>,
}
impl State {
    fn seed() -> Self {
        let mut registers = [None; 16];
        for r in Gpr::ALL {
            if r.preserved() && !r.reserved() {
                registers[r as usize] = Some(r)
            }
        }
        Self {
            sp: 0,
            fp: None,
            original_fp: true,
            header: false,
            registers,
            simd: [None; 16],
            memory: BTreeMap::new(),
        }
    }
    fn meet(&mut self, other: &Self) -> Result<bool, Reason> {
        if (self.sp, self.fp, self.original_fp, self.header)
            != (other.sp, other.fp, other.original_fp, other.header)
        {
            return Err(Reason::StackJoin);
        }
        let before = self.clone();
        for (a, b) in self.registers.iter_mut().zip(other.registers) {
            if *a != b {
                *a = None
            }
        }
        for (a, b) in self.simd.iter_mut().zip(other.simd) {
            if *a != b {
                *a = None
            }
        }
        self.memory
            .retain(|key, value| other.memory.get(key) == Some(value));
        Ok(*self != before)
    }
    fn address(&self, operand: Operand) -> Result<Option<i64>, Reason> {
        if let Operand::Memory { base, displacement } = operand {
            let base = match base {
                Gpr::Rbp => self.fp.ok_or(Reason::StackState)?,
                Gpr::Rsp => self.sp,
                _ => return Ok(None),
            };
            return base
                .checked_add(i64::from(displacement))
                .map(Some)
                .ok_or(Reason::Capacity);
        }
        Ok(None)
    }
    fn origin(&self, o: Operand, bits: u16) -> Result<Option<Gpr>, Reason> {
        if bits != 64 {
            return Ok(None);
        }
        match o {
            Operand::Register(Register::Gpr(r)) => Ok(self.registers[r as usize]),
            Operand::Register(Register::Xmm(i)) => {
                self.simd.get(i as usize).copied().ok_or(Reason::Encoding)
            }
            _ => Ok(self.address(o)?.and_then(|a| self.memory.get(&a).copied())),
        }
    }
    fn write(&mut self, r: Register, origin: Option<Gpr>) -> Result<(), Reason> {
        if let Register::Gpr(r) = r {
            if r.reserved() {
                return Err(Reason::StackState);
            }
            self.registers[r as usize] = origin;
        } else if let Register::Xmm(i) = r {
            *self.simd.get_mut(i as usize).ok_or(Reason::Encoding)? = origin;
        }
        Ok(())
    }
    fn movement(&mut self, bits: u16, source: Operand, destination: Operand) -> Result<(), Reason> {
        let fp = Operand::Register(Register::Gpr(Gpr::Rbp));
        let sp = Operand::Register(Register::Gpr(Gpr::Rsp));
        if bits == 64 && source == sp && destination == fp {
            if self.sp != -8 || !self.header || !self.original_fp {
                return Err(Reason::StackState);
            }
            self.fp = Some(self.sp);
            self.original_fp = false;
            return Ok(());
        }
        if bits == 64 && source == fp && destination == sp {
            self.sp = self.fp.ok_or(Reason::StackState)?;
            return Ok(());
        }
        let origin = self.origin(source, bits)?;
        // Reads must also have established FP even when they are not 64-bit copies.
        self.address(source)?;
        match destination {
            Operand::Register(r) => self.write(r, origin)?,
            Operand::Memory { .. } => {
                if let Some(address) = self.address(destination)? {
                    let end = address
                        .checked_add(i64::from(bits / 8))
                        .ok_or(Reason::Capacity)?;
                    if address < 8 && end > 0 {
                        return Err(Reason::StackState);
                    } // incoming return address
                    if address < 0 && end > -8 {
                        self.header = false;
                    }
                    self.memory
                        .retain(|&key, _| key + 8 <= address || key >= end);
                    if let Some(origin) = origin {
                        self.memory.insert(address, origin);
                    }
                }
            }
            Operand::Immediate(_) => return Err(Reason::Encoding),
        }
        Ok(())
    }
}
pub(super) fn check(
    draft: &PhysicalDraft<'_, '_, '_, '_>,
    key: LirCallableId,
) -> Result<(), PhysicalError> {
    let fail = |block: usize, reason| PhysicalError::new(key, Some(block), None, None, reason);
    if draft.entry.0 >= draft.blocks.len() {
        return Err(fail(draft.entry.0, Reason::Topology));
    }
    let instructions = draft
        .blocks
        .iter()
        .flat_map(|b| &b.groups)
        .map(|g| g.instructions.len())
        .try_fold(0usize, usize::checked_add)
        .ok_or_else(|| fail(draft.entry.0, Reason::Capacity))?;
    let bound = draft
        .blocks
        .len()
        .checked_mul(
            instructions
                .checked_add(33)
                .ok_or_else(|| fail(draft.entry.0, Reason::Capacity))?,
        )
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| fail(draft.entry.0, Reason::Capacity))?;
    let mut states = vec![None; draft.blocks.len()];
    states[draft.entry.0] = Some(State::seed());
    let mut pending = VecDeque::from([draft.entry.0]);
    let mut queued = vec![false; draft.blocks.len()];
    queued[draft.entry.0] = true;
    let mut iterations = 0;
    while let Some(block) = pending.pop_front() {
        queued[block] = false;
        iterations += 1;
        if iterations > bound {
            return Err(fail(block, Reason::Capacity));
        }
        let edges = run(
            &draft.blocks[block],
            states[block].clone().unwrap(),
            key,
            false,
        )?;
        for (target, state) in edges {
            let changed = if let Some(current) = &mut states[target.0] {
                current.meet(&state).map_err(|r| fail(target.0, r))?
            } else {
                states[target.0] = Some(state);
                true
            };
            if changed && !queued[target.0] {
                queued[target.0] = true;
                pending.push_back(target.0);
            }
        }
    }
    for (block, state) in states.into_iter().enumerate() {
        if let Some(state) = state {
            run(&draft.blocks[block], state, key, true)?;
        }
    }
    Ok(())
}
fn run(
    block: &Block,
    mut state: State,
    key: LirCallableId,
    strict: bool,
) -> Result<Vec<(BlockId, State)>, PhysicalError> {
    let mut edges = vec![];
    let mut terminated = false;
    for (g, group) in block.groups.iter().enumerate() {
        for (i, instruction) in group.instructions.iter().enumerate() {
            let fail = |r| PhysicalError::new(key, Some(block.id.0), Some(g), Some(i), r);
            if terminated {
                return Err(fail(Reason::Topology));
            }
            let result = match *instruction {
                Instruction::Push(Gpr::Rbp) => {
                    if state.sp == 0 && state.original_fp && !state.header {
                        state.sp = -8;
                        state.header = true;
                        Ok(())
                    } else {
                        Err(Reason::StackState)
                    }
                }
                Instruction::Pop(Gpr::Rbp) => {
                    if state.sp == -8 && state.header {
                        state.sp = 0;
                        state.fp = None;
                        state.original_fp = true;
                        state.header = false;
                        Ok(())
                    } else {
                        Err(Reason::StackState)
                    }
                }
                Instruction::Push(_) | Instruction::Pop(_) => Err(Reason::StackState),
                Instruction::StackSubtract(bytes) => {
                    if state.fp == Some(-8) && state.header {
                        state.sp = state
                            .sp
                            .checked_sub(i64::from(bytes))
                            .ok_or_else(|| fail(Reason::Capacity))?;
                        Ok(())
                    } else {
                        Err(Reason::StackState)
                    }
                }
                Instruction::Move {
                    bits,
                    source,
                    destination,
                    ..
                } => state.movement(bits, source, destination),
                Instruction::Alu {
                    source,
                    destination,
                    ..
                }
                | Instruction::Lea {
                    source,
                    destination,
                } => {
                    state.address(source).map_err(fail)?;
                    state.write(destination, None)
                }
                Instruction::Unary { destination, .. }
                | Instruction::Set { destination, .. }
                | Instruction::Convert { destination, .. }
                | Instruction::RipAddress { destination, .. }
                | Instruction::TlsBase { destination }
                | Instruction::TlsOffset { destination, .. } => state.write(destination, None),
                Instruction::Compare { left, right, .. } => {
                    state.address(left).map_err(fail)?;
                    state.address(right).map_err(fail)?;
                    Ok(())
                }
                Instruction::DividendSignExtend => {
                    state.registers[Gpr::Rdx as usize] = None;
                    Ok(())
                }
                Instruction::Divide { .. } => {
                    state.registers[Gpr::Rax as usize] = None;
                    state.registers[Gpr::Rdx as usize] = None;
                    Ok(())
                }
                Instruction::Shift { destination, .. } => state.write(destination, None),
                Instruction::Call(_) => {
                    if (state.sp + 8).rem_euclid(16) != 0 {
                        Err(Reason::Alignment)
                    } else {
                        state.simd.fill(None);
                        for r in Gpr::ALL {
                            if !r.preserved() {
                                state.registers[r as usize] = None;
                            }
                        }
                        Ok(())
                    }
                }
                Instruction::JumpIf { destination, .. } => {
                    edges.push((destination, state.clone()));
                    Ok(())
                }
                Instruction::Jump(target) => {
                    edges.push((target, state.clone()));
                    terminated = true;
                    Ok(())
                }
                Instruction::HardTrap => {
                    terminated = true;
                    Ok(())
                }
                Instruction::Return => {
                    terminated = true;
                    if state.sp != 0 || state.fp.is_some() || !state.original_fp || state.header {
                        Err(Reason::StackState)
                    } else if strict
                        && Gpr::ALL.into_iter().any(|r| {
                            r.preserved() && !r.reserved() && state.registers[r as usize] != Some(r)
                        })
                    {
                        Err(Reason::Preservation)
                    } else {
                        Ok(())
                    }
                }
            };
            result.map_err(fail)?;
        }
    }
    if !terminated {
        return Err(PhysicalError::new(
            key,
            Some(block.id.0),
            None,
            None,
            Reason::Topology,
        ));
    }
    Ok(edges)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn returning_calls_destroy_original_bits_carried_in_caller_simd_resources() {
        let mut seed = State::seed();
        seed.sp = -24;
        seed.fp = Some(-8);
        seed.header = true;
        seed.original_fp = false;
        let movement = |kind, source, destination| Instruction::Move {
            kind,
            bits: 64,
            source,
            destination,
        };
        let rbx = Operand::Register(Register::Gpr(Gpr::Rbx));
        let xmm = Operand::Register(Register::Xmm(0));
        let mut code = vec![
            movement(MoveKind::Bits, rbx, xmm),
            movement(MoveKind::Integer, Operand::Immediate(0), rbx),
            movement(MoveKind::Bits, xmm, rbx),
            movement(
                MoveKind::Integer,
                Operand::Register(Register::Gpr(Gpr::Rbp)),
                Operand::Register(Register::Gpr(Gpr::Rsp)),
            ),
            Instruction::Pop(Gpr::Rbp),
            Instruction::Return,
        ];
        let block = |code| Block {
            id: BlockId(0),
            origin: BlockOrigin::Entry,
            groups: vec![Group {
                origin: Origin::Prologue,
                instructions: code,
                dependencies: vec![],
            }],
        };
        assert!(run(
            &block(code.clone()),
            seed.clone(),
            LirCallableId::Entry,
            true
        )
        .is_ok());
        code.insert(
            2,
            Instruction::Call(CallTarget::Indirect(Register::Gpr(Gpr::Rax))),
        );
        assert_eq!(
            run(&block(code), seed, LirCallableId::Entry, true)
                .err()
                .unwrap()
                .reason,
            Reason::Preservation
        );
    }
}
