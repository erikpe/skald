//! Deterministic GNU assembler text emission.

use std::fmt::Write;

use super::machine::{AssemblyProgram, Instruction, Operand};

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
            output.push_str("    ");
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
        Instruction::MoveImmediate64 { value, destination } => {
            write!(output, "movabsq ${value}, {}", destination.name()).unwrap()
        }
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
        Instruction::ReserveStack(bytes) => write!(output, "subq ${bytes}, %rsp").unwrap(),
        Instruction::ReleaseStack(bytes) => write!(output, "addq ${bytes}, %rsp").unwrap(),
        Instruction::Call(symbol) => write!(output, "call {symbol}").unwrap(),
        Instruction::Leave => output.push_str("leave"),
        Instruction::Return => output.push_str("ret"),
    }
}

fn display_operand(operand: Operand) -> String {
    match operand {
        Operand::Register(register) => register.name().to_owned(),
        Operand::Memory { base, displacement } => {
            if displacement == 0 {
                format!("({})", base.name())
            } else {
                format!("{displacement}({})", base.name())
            }
        }
    }
}
