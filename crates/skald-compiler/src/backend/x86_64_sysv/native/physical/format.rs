//! Instruction leaf formatting only. This module cannot publish a callable or program.
use super::model::*;
use crate::backend::plan::ArtifactId;
use std::fmt::{self, Write};

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn format_instruction(
    out: &mut dyn Write,
    instruction: &Instruction,
    mut symbol: impl FnMut(ArtifactId) -> String,
    mut block_label: impl FnMut(BlockId) -> String,
) -> fmt::Result {
    let reg = |r, b| register(r, b);
    let operand = |o, b| operand(o, b);
    match instruction {
        Instruction::Move {
            kind,
            bits,
            source,
            destination,
        } => write!(
            out,
            "{} {}, {}",
            match kind {
                MoveKind::Integer => "mov",
                MoveKind::Float => "movsd",
                MoveKind::Bits => "movq",
            },
            operand(*destination, *bits)?,
            operand(*source, *bits)?
        ),
        Instruction::Alu {
            op,
            float,
            bits,
            source,
            destination,
        } => write!(
            out,
            "{} {}, {}",
            match (op, float) {
                (Alu::Add, true) => "addsd",
                (Alu::Subtract, true) => "subsd",
                (Alu::Multiply, true) => "mulsd",
                (Alu::DivideFloat, true) => "divsd",
                (Alu::Add, false) => "add",
                (Alu::Subtract, false) => "sub",
                (Alu::Multiply, false) => "imul",
                (Alu::And, false) => "and",
                (Alu::Or, false) => "or",
                (Alu::Xor, false) => "xor",
                _ => return Err(fmt::Error),
            },
            reg(*destination, *bits)?,
            operand(*source, *bits)?
        ),
        Instruction::Unary {
            op,
            bits,
            destination,
        } => write!(
            out,
            "{} {}",
            match op {
                Unary::Negate => "neg",
                Unary::Complement => "not",
            },
            reg(*destination, *bits)?
        ),
        Instruction::Compare {
            float,
            bits,
            left,
            right,
        } => write!(
            out,
            "{} {}, {}",
            if *float { "ucomisd" } else { "cmp" },
            operand(*left, *bits)?,
            operand(*right, *bits)?
        ),
        Instruction::Set {
            condition,
            destination,
        } => write!(
            out,
            "set{} {}",
            condition_name(*condition),
            reg(*destination, 8)?
        ),
        Instruction::Convert {
            op,
            input_bits,
            output_bits,
            source,
            destination,
        } => write!(
            out,
            "{} {}, {}",
            match op {
                Convert::ZeroExtend => "movzx",
                Convert::SignedToFloat => "cvtsi2sd",
                Convert::TruncateFloat => "cvttsd2si",
            },
            reg(*destination, *output_bits)?,
            reg(*source, *input_bits)?
        ),
        Instruction::DividendSignExtend => out.write_str("cqo"),
        Instruction::Divide { signed, divisor } => write!(
            out,
            "{} {}",
            if *signed { "idiv" } else { "div" },
            reg(*divisor, 64)?
        ),
        Instruction::Shift {
            op,
            bits,
            destination,
            variable,
        } => write!(
            out,
            "{} {}, {}",
            match op {
                Shift::Left => "shl",
                Shift::ArithmeticRight => "sar",
                Shift::LogicalRight => "shr",
            },
            reg(*destination, *bits)?,
            if *variable { "cl" } else { "1" }
        ),
        Instruction::Lea {
            source,
            destination,
        } => write!(
            out,
            "lea {}, {}",
            reg(*destination, 64)?,
            operand(*source, 0)?
        ),
        Instruction::RipAddress {
            artifact,
            destination,
        } => write!(
            out,
            "lea {}, [rip+{}]",
            reg(*destination, 64)?,
            symbol(*artifact)
        ),
        Instruction::TlsBase { destination } => {
            write!(out, "mov {}, QWORD PTR fs:0", reg(*destination, 64)?)
        }
        Instruction::TlsOffset {
            artifact,
            destination,
        } => write!(
            out,
            "lea {}, [{}+{}@tpoff]",
            reg(*destination, 64)?,
            reg(*destination, 64)?,
            symbol(*artifact)
        ),
        Instruction::Call(target) => match target {
            CallTarget::Direct(artifact) => write!(out, "call {}", symbol(*artifact)),
            CallTarget::Indirect(r) => write!(out, "call {}", reg(*r, 64)?),
        },
        Instruction::Jump(id) => write!(out, "jmp {}", block_label(*id)),
        Instruction::JumpIf {
            condition,
            destination,
        } => write!(
            out,
            "j{} {}",
            condition_name(*condition),
            block_label(*destination)
        ),
        Instruction::Push(r) => write!(out, "push {}", reg(Register::Gpr(*r), 64)?),
        Instruction::Pop(r) => write!(out, "pop {}", reg(Register::Gpr(*r), 64)?),
        Instruction::StackSubtract(bytes) => write!(out, "sub rsp, {bytes}"),
        Instruction::Return => out.write_str("ret"),
        Instruction::HardTrap => out.write_str("ud2"),
    }
}
fn operand(o: Operand, bits: u16) -> Result<String, fmt::Error> {
    Ok(match o {
        Operand::Register(r) => register(r, bits)?,
        Operand::Immediate(value) => value.to_string(),
        Operand::Memory { base, displacement } => format!(
            "{}[{}{:+}]",
            match bits {
                0 => "",
                8 => "BYTE PTR ",
                16 => "WORD PTR ",
                32 => "DWORD PTR ",
                64 => "QWORD PTR ",
                _ => return Err(fmt::Error),
            },
            register(Register::Gpr(base), 64)?,
            displacement
        ),
    })
}
fn register(r: Register, bits: u16) -> Result<String, fmt::Error> {
    const NAMES: [[&str; 16]; 4] = [
        [
            "al", "cl", "dl", "bl", "spl", "bpl", "sil", "dil", "r8b", "r9b", "r10b", "r11b",
            "r12b", "r13b", "r14b", "r15b",
        ],
        [
            "ax", "cx", "dx", "bx", "sp", "bp", "si", "di", "r8w", "r9w", "r10w", "r11w", "r12w",
            "r13w", "r14w", "r15w",
        ],
        [
            "eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi", "r8d", "r9d", "r10d", "r11d",
            "r12d", "r13d", "r14d", "r15d",
        ],
        [
            "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11",
            "r12", "r13", "r14", "r15",
        ],
    ];
    match r {
        Register::Gpr(r) => Ok(NAMES[match bits {
            8 => 0,
            16 => 1,
            32 => 2,
            64 => 3,
            _ => return Err(fmt::Error),
        }][r as usize]
            .to_owned()),
        Register::Xmm(index) if index < 16 && matches!(bits, 64 | 128) => Ok(format!("xmm{index}")),
        _ => Err(fmt::Error),
    }
}
fn condition_name(c: Condition) -> &'static str {
    match c {
        Condition::Equal => "e",
        Condition::NotEqual => "ne",
        Condition::Less => "l",
        Condition::LessEqual => "le",
        Condition::Greater => "g",
        Condition::GreaterEqual => "ge",
        Condition::Below => "b",
        Condition::BelowEqual => "be",
        Condition::Above => "a",
        Condition::AboveEqual => "ae",
        Condition::Parity => "p",
        Condition::NotParity => "np",
    }
}
