//! Deterministic GNU assembler text emission.

use std::fmt::Write;

use super::machine::{AssemblyProgram, FloatOperand, Instruction, Operand};

pub(super) fn emit(program: &AssemblyProgram) -> String {
    let mut output = String::from(".text\n");
    for (index, function) in program.functions.iter().enumerate() {
        if index != 0 {
            output.push('\n');
        }
        output.push_str(".p2align 4\n");
        if function.exported {
            writeln!(output, ".globl {}", function.symbol).unwrap();
        }
        writeln!(output, ".type {}, @function", function.symbol).unwrap();
        writeln!(output, "{}:", function.symbol).unwrap();
        for instruction in &function.instructions {
            if !matches!(instruction, Instruction::Label(_)) {
                output.push_str("    ");
            }
            emit_instruction(&mut output, instruction);
            output.push('\n');
        }
        writeln!(output, ".size {}, .-{}", function.symbol, function.symbol).unwrap();
    }
    output.push_str("\n.section .note.GNU-stack,\"\",@progbits\n");
    output
}

fn emit_instruction(output: &mut String, instruction: &Instruction) {
    match instruction {
        Instruction::Label(label) => write!(output, "{}:", label.name()).unwrap(),
        Instruction::Push(register) => write!(output, "pushq {}", register.name()).unwrap(),
        Instruction::Move {
            source,
            destination,
        } => write!(
            output,
            "movq {}, {}",
            display_operand(*source),
            display_operand(*destination)
        )
        .unwrap(),
        Instruction::MoveByte {
            source,
            destination,
        } => write!(
            output,
            "movb {}, {}",
            source.name(),
            display_operand(*destination)
        )
        .unwrap(),
        Instruction::LoadEffectiveAddress {
            source,
            destination,
        } => write!(
            output,
            "leaq {}, {}",
            display_operand(*source),
            destination.name()
        )
        .unwrap(),
        Instruction::MoveImmediate64 { bits, destination } => {
            if *bits <= i64::MAX as u64 {
                write!(output, "movabsq ${bits}, {}", destination.name()).unwrap()
            } else {
                write!(output, "movabsq $0x{bits:016x}, {}", destination.name()).unwrap()
            }
        }
        Instruction::MoveBitsToFloat {
            source,
            destination,
        } => write!(output, "movq {}, {}", source.name(), destination.name()).unwrap(),
        Instruction::MoveFloat64 {
            source,
            destination,
        } => write!(
            output,
            "movsd {}, {}",
            display_float_operand(*source),
            display_float_operand(*destination)
        )
        .unwrap(),
        Instruction::ZeroExtendByte {
            source,
            destination,
        } => write!(output, "movzbq {}, {}", source.name(), destination.name()).unwrap(),
        Instruction::LoadZeroExtendByte {
            source,
            destination,
        } => write!(
            output,
            "movzbq {}, {}",
            display_operand(*source),
            destination.name()
        )
        .unwrap(),
        Instruction::Add {
            source,
            destination,
        } => write!(output, "addq {}, {}", source.name(), destination.name()).unwrap(),
        Instruction::Subtract {
            source,
            destination,
        } => write!(output, "subq {}, {}", source.name(), destination.name()).unwrap(),
        Instruction::Multiply {
            source,
            destination,
        } => write!(output, "imulq {}, {}", source.name(), destination.name()).unwrap(),
        Instruction::Negate(register) => write!(output, "negq {}", register.name()).unwrap(),
        Instruction::AddFloat64 {
            source,
            destination,
        } => write!(output, "addsd {}, {}", source.name(), destination.name()).unwrap(),
        Instruction::SubtractFloat64 {
            source,
            destination,
        } => write!(output, "subsd {}, {}", source.name(), destination.name()).unwrap(),
        Instruction::MultiplyFloat64 {
            source,
            destination,
        } => write!(output, "mulsd {}, {}", source.name(), destination.name()).unwrap(),
        Instruction::XorFloat128 {
            source,
            destination,
        } => write!(output, "xorpd {}, {}", source.name(), destination.name()).unwrap(),
        Instruction::Test(register) => {
            write!(output, "testq {}, {}", register.name(), register.name()).unwrap()
        }
        Instruction::ReserveStack(bytes) => write!(output, "subq ${bytes}, %rsp").unwrap(),
        Instruction::ReleaseStack(bytes) => write!(output, "addq ${bytes}, %rsp").unwrap(),
        Instruction::Call(symbol) => write!(output, "call {symbol}").unwrap(),
        Instruction::Jump(label) => write!(output, "jmp {}", label.name()).unwrap(),
        Instruction::JumpIfNotZero(label) => write!(output, "jne {}", label.name()).unwrap(),
        Instruction::Leave => output.push_str("leave"),
        Instruction::Return => output.push_str("ret"),
    }
}

fn display_float_operand(operand: FloatOperand) -> String {
    match operand {
        FloatOperand::Register(register) => register.name().to_owned(),
        FloatOperand::Memory { base, displacement } => display_memory(base, displacement),
    }
}

fn display_operand(operand: Operand) -> String {
    match operand {
        Operand::Register(register) => register.name().to_owned(),
        Operand::Memory { base, displacement } => display_memory(base, displacement),
    }
}

fn display_memory(base: super::machine::Register, displacement: i32) -> String {
    if displacement == 0 {
        format!("({})", base.name())
    } else {
        format!("{displacement}({})", base.name())
    }
}
