//! Deterministic GNU assembler text emission using Intel syntax.

use std::fmt::Write;

use super::machine::{AssemblyProgram, FloatOperand, Instruction, Operand};

pub(super) fn emit(program: &AssemblyProgram) -> String {
    let mut output = String::from(".intel_syntax noprefix\n.text\n");
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
    if !program.static_slots.is_empty() {
        output.push_str("\n.bss\n");
        for slot in &program.static_slots {
            writeln!(output, ".p2align {}", slot.alignment_power).unwrap();
            writeln!(output, ".type {}, @object", slot.symbol).unwrap();
            writeln!(output, "{}:", slot.symbol).unwrap();
            writeln!(output, "    .zero {}", slot.size).unwrap();
            writeln!(output, ".size {}, {}", slot.symbol, slot.size).unwrap();
        }
    }
    if !program.literal_backings.is_empty()
        || !program.dispatch_tables.is_empty()
        || !program.panic_messages.is_empty()
    {
        output.push_str("\n.section .data.rel.ro.local,\"aw\",@progbits\n");
        for backing in &program.literal_backings {
            output.push_str(".p2align 3\n");
            writeln!(output, ".type {}, @object", backing.symbol).unwrap();
            writeln!(output, "{}:", backing.symbol).unwrap();
            output.push_str("    .quad 0xffffffffffffffff\n");
            writeln!(output, "    .quad {}", backing.metadata_symbol).unwrap();
            writeln!(output, "    .quad {}", backing.bytes.len()).unwrap();
            if !backing.bytes.is_empty() {
                emit_ascii_bytes(&mut output, &backing.bytes);
            }
            writeln!(output, ".size {}, .-{}", backing.symbol, backing.symbol).unwrap();
        }
        for table in &program.dispatch_tables {
            output.push_str(".p2align 3\n");
            writeln!(output, ".type {}, @object", table.symbol).unwrap();
            writeln!(output, "{}:", table.symbol).unwrap();
            for entry in &table.entries {
                match entry {
                    Some(symbol) => writeln!(output, "    .quad {symbol}").unwrap(),
                    None => output.push_str("    .quad 0\n"),
                }
            }
            // Every class metadata symbol occupies storage, even without
            // dispatch slots, so its address is a unique dynamic identity.
            output.push_str("    .quad 0\n");
            writeln!(output, ".size {}, .-{}", table.symbol, table.symbol).unwrap();
        }
        for message in &program.panic_messages {
            output.push_str(".p2align 0\n");
            writeln!(output, ".type {}, @object", message.symbol).unwrap();
            writeln!(output, "{}:", message.symbol).unwrap();
            emit_ascii_bytes(&mut output, message.bytes);
            writeln!(output, ".size {}, .-{}", message.symbol, message.symbol).unwrap();
        }
    }
    output.push_str("\n.section .note.GNU-stack,\"\",@progbits\n");
    output
}

fn emit_ascii_bytes(output: &mut String, bytes: &[u8]) {
    output.push_str("    .ascii \"");
    for byte in bytes {
        match byte {
            b'"' => output.push_str("\\\""),
            b'\\' => output.push_str("\\\\"),
            b'\n' => output.push_str("\\n"),
            b'\r' => output.push_str("\\r"),
            b'\t' => output.push_str("\\t"),
            0x20..=0x7e => output.push(char::from(*byte)),
            _ => write!(output, "\\{byte:03o}").unwrap(),
        }
    }
    output.push_str("\"\n");
}

fn emit_instruction(output: &mut String, instruction: &Instruction) {
    match instruction {
        Instruction::Label(label) => write!(output, "{}:", label.name()).unwrap(),
        Instruction::Push(register) => write!(output, "push {}", register.name()).unwrap(),
        Instruction::Move {
            source,
            destination,
        } => write!(
            output,
            "mov {}, {}",
            display_operand(*destination, MemorySize::Qword),
            display_operand(*source, MemorySize::Qword)
        )
        .unwrap(),
        Instruction::MoveByte {
            source,
            destination,
        } => write!(
            output,
            "mov {}, {}",
            display_operand(*destination, MemorySize::Byte),
            source.name()
        )
        .unwrap(),
        Instruction::LoadEffectiveAddress {
            source,
            destination,
        } => write!(
            output,
            "lea {}, {}",
            destination.name(),
            display_address_operand(*source)
        )
        .unwrap(),
        Instruction::LoadSymbolAddress {
            symbol,
            destination,
        } => write!(output, "lea {}, [rip + {symbol}]", destination.name()).unwrap(),
        Instruction::MoveImmediate64 { bits, destination } => {
            if *bits <= i64::MAX as u64 {
                write!(output, "mov {}, {bits}", destination.name()).unwrap()
            } else {
                write!(output, "mov {}, 0x{bits:016x}", destination.name()).unwrap()
            }
        }
        Instruction::MoveBitsToFloat {
            source,
            destination,
        } => write!(output, "movq {}, {}", destination.name(), source.name()).unwrap(),
        Instruction::MoveFloatBitsToInteger {
            source,
            destination,
        } => write!(output, "movq {}, {}", destination.name(), source.name()).unwrap(),
        Instruction::ConvertSignedIntegerToFloat64 {
            source,
            destination,
        } => write!(output, "cvtsi2sd {}, {}", destination.name(), source.name()).unwrap(),
        Instruction::ConvertFloat64ToSignedInteger {
            source,
            destination,
        } => write!(
            output,
            "cvttsd2si {}, {}",
            destination.name(),
            source.name()
        )
        .unwrap(),
        Instruction::MoveFloat64 {
            source,
            destination,
        } => write!(
            output,
            "movsd {}, {}",
            display_float_operand(*destination),
            display_float_operand(*source)
        )
        .unwrap(),
        Instruction::ZeroExtendByte {
            source,
            destination,
        } => write!(output, "movzx {}, {}", destination.name(), source.name()).unwrap(),
        Instruction::LoadZeroExtendByte {
            source,
            destination,
        } => write!(
            output,
            "movzx {}, {}",
            destination.name(),
            display_operand(*source, MemorySize::Byte)
        )
        .unwrap(),
        Instruction::Add {
            source,
            destination,
        } => write!(output, "add {}, {}", destination.name(), source.name()).unwrap(),
        Instruction::Subtract {
            source,
            destination,
        } => write!(output, "sub {}, {}", destination.name(), source.name()).unwrap(),
        Instruction::Multiply {
            source,
            destination,
        } => write!(output, "imul {}, {}", destination.name(), source.name()).unwrap(),
        Instruction::SignExtendDividend => output.push_str("cqo"),
        Instruction::IntegerDivide { flavor, divisor } => {
            write!(output, "{} {}", flavor.mnemonic(), divisor.name()).unwrap()
        }
        Instruction::Negate(register) => write!(output, "neg {}", register.name()).unwrap(),
        Instruction::BitwiseNot(register) => write!(output, "not {}", register.name()).unwrap(),
        Instruction::Bitwise {
            operation,
            source,
            destination,
        } => write!(
            output,
            "{} {}, {}",
            operation.mnemonic(),
            destination.name(),
            source.name()
        )
        .unwrap(),
        Instruction::ByteBitwise {
            operation,
            source,
            destination,
        } => write!(
            output,
            "{} {}, {}",
            operation.mnemonic(),
            destination.name(),
            source.name()
        )
        .unwrap(),
        Instruction::Shift {
            operation,
            destination,
        } => write!(
            output,
            "{} {}, {}",
            operation.mnemonic(),
            destination.name(),
            super::machine::ByteRegister::Cl.name()
        )
        .unwrap(),
        Instruction::AddFloat64 {
            source,
            destination,
        } => write!(output, "addsd {}, {}", destination.name(), source.name()).unwrap(),
        Instruction::SubtractFloat64 {
            source,
            destination,
        } => write!(output, "subsd {}, {}", destination.name(), source.name()).unwrap(),
        Instruction::MultiplyFloat64 {
            source,
            destination,
        } => write!(output, "mulsd {}, {}", destination.name(), source.name()).unwrap(),
        Instruction::DivideFloat64 {
            source,
            destination,
        } => write!(output, "divsd {}, {}", destination.name(), source.name()).unwrap(),
        Instruction::XorFloat128 {
            source,
            destination,
        } => write!(output, "xorpd {}, {}", destination.name(), source.name()).unwrap(),
        Instruction::Test(register) => {
            write!(output, "test {}, {}", register.name(), register.name()).unwrap()
        }
        Instruction::Compare {
            source,
            destination,
        } => write!(output, "cmp {}, {}", destination.name(), source.name()).unwrap(),
        Instruction::CompareFloat64 {
            source,
            destination,
        } => write!(output, "ucomisd {}, {}", destination.name(), source.name()).unwrap(),
        Instruction::SetCondition {
            condition,
            destination,
        } => write!(output, "set{} {}", condition.mnemonic(), destination.name()).unwrap(),
        Instruction::ReserveStack(bytes) => write!(output, "sub rsp, {bytes}").unwrap(),
        Instruction::ReleaseStack(bytes) => write!(output, "add rsp, {bytes}").unwrap(),
        Instruction::Call(symbol) => write!(output, "call {symbol}").unwrap(),
        Instruction::CallIndirect(register) => write!(output, "call {}", register.name()).unwrap(),
        Instruction::Jump(label) => write!(output, "jmp {}", label.name()).unwrap(),
        Instruction::JumpIfNotZero(label) => write!(output, "jne {}", label.name()).unwrap(),
        Instruction::JumpIfEqual(label) => write!(output, "je {}", label.name()).unwrap(),
        Instruction::JumpIfNotSign(label) => write!(output, "jns {}", label.name()).unwrap(),
        Instruction::JumpIfBelow(label) => write!(output, "jb {}", label.name()).unwrap(),
        Instruction::JumpIfAbove(label) => write!(output, "ja {}", label.name()).unwrap(),
        Instruction::JumpIf { condition, target } => {
            write!(output, "j{} {}", condition.mnemonic(), target.name()).unwrap()
        }
        Instruction::Trap => output.push_str("ud2"),
        Instruction::Leave => output.push_str("leave"),
        Instruction::Return => output.push_str("ret"),
    }
}

#[derive(Clone, Copy)]
enum MemorySize {
    Byte,
    Qword,
}

impl MemorySize {
    const fn qualifier(self) -> &'static str {
        match self {
            Self::Byte => "byte ptr",
            Self::Qword => "qword ptr",
        }
    }
}

fn display_float_operand(operand: FloatOperand) -> String {
    match operand {
        FloatOperand::Register(register) => register.name().to_owned(),
        FloatOperand::Memory { base, displacement } => {
            display_memory(base, displacement, Some(MemorySize::Qword))
        }
        FloatOperand::IndexedMemory {
            base,
            index,
            scale,
            displacement,
        } => display_indexed_memory(base, index, scale, displacement, Some(MemorySize::Qword)),
    }
}

fn display_operand(operand: Operand, memory_size: MemorySize) -> String {
    match operand {
        Operand::Register(register) => register.name().to_owned(),
        Operand::Memory { base, displacement } => {
            display_memory(base, displacement, Some(memory_size))
        }
        Operand::IndexedMemory {
            base,
            index,
            scale,
            displacement,
        } => display_indexed_memory(base, index, scale, displacement, Some(memory_size)),
    }
}

fn display_address_operand(operand: Operand) -> String {
    match operand {
        Operand::Register(register) => register.name().to_owned(),
        Operand::Memory { base, displacement } => display_memory(base, displacement, None),
        Operand::IndexedMemory {
            base,
            index,
            scale,
            displacement,
        } => display_indexed_memory(base, index, scale, displacement, None),
    }
}

fn display_indexed_memory(
    base: super::machine::Register,
    index: super::machine::Register,
    scale: u8,
    displacement: i32,
    size: Option<MemorySize>,
) -> String {
    let suffix = if displacement == 0 {
        String::new()
    } else if displacement > 0 {
        format!(" + {displacement}")
    } else {
        format!(" - {}", displacement.unsigned_abs())
    };
    let address = format!("[{} + {}*{scale}{suffix}]", base.name(), index.name());
    if let Some(size) = size {
        format!("{} {address}", size.qualifier())
    } else {
        address
    }
}

fn display_memory(
    base: super::machine::Register,
    displacement: i32,
    size: Option<MemorySize>,
) -> String {
    let address = if displacement == 0 {
        format!("[{}]", base.name())
    } else if displacement > 0 {
        format!("[{} + {displacement}]", base.name())
    } else {
        format!("[{} - {}]", base.name(), displacement.unsigned_abs())
    };
    if let Some(size) = size {
        format!("{} {address}", size.qualifier())
    } else {
        address
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::x86_64_sysv::machine::{ByteRegister, ConditionCode};

    #[test]
    fn emits_every_condition_code() {
        for (condition, expected) in [
            (ConditionCode::Equal, "sete al"),
            (ConditionCode::NotEqual, "setne al"),
            (ConditionCode::Parity, "setp al"),
            (ConditionCode::NotParity, "setnp al"),
            (ConditionCode::SignedLess, "setl al"),
            (ConditionCode::SignedLessEqual, "setle al"),
            (ConditionCode::SignedGreater, "setg al"),
            (ConditionCode::SignedGreaterEqual, "setge al"),
            (ConditionCode::UnsignedBelow, "setb al"),
            (ConditionCode::UnsignedBelowEqual, "setbe al"),
            (ConditionCode::UnsignedAbove, "seta al"),
            (ConditionCode::UnsignedAboveEqual, "setae al"),
        ] {
            let mut output = String::new();
            emit_instruction(
                &mut output,
                &Instruction::SetCondition {
                    condition,
                    destination: ByteRegister::Al,
                },
            );
            assert_eq!(output, expected);

            let mut output = String::new();
            emit_instruction(
                &mut output,
                &Instruction::JumpIf {
                    condition,
                    target: super::super::machine::Label::new("target".to_owned()),
                },
            );
            assert_eq!(
                output,
                expected.replace("set", "j").replace(" al", " target")
            );
        }
    }
}
