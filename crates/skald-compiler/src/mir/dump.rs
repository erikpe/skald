//! Deterministic textual rendering of MIR.

use std::fmt::Write;

use crate::dump_format::{write_quoted, write_span};

use super::model::*;

pub fn dump_mir(program: &MirProgram) -> String {
    let mut output = String::new();
    output.push_str("MirProgram");
    write_span(&mut output, program.span);
    output.push('\n');
    let _ = writeln!(output, "  Entry {}", program.entry_function);
    output.push_str("  Classes\n");
    for class in program.classes.iter() {
        dump_class(&mut output, class);
    }
    output.push_str("  Declarations\n");
    for declaration in program.declarations.iter() {
        dump_declaration(&mut output, declaration);
    }
    output.push_str("  Definitions\n");
    for definition in program.definitions.iter() {
        dump_definition(&mut output, definition);
    }
    output
}

fn dump_class(output: &mut String, class: &MirClassDeclaration) {
    let _ = write!(output, "    Class {} ", class.id);
    write_quoted(output, &class.name);
    write_span(output, class.span);
    output.push('\n');
    for field in &class.fields {
        let _ = write!(output, "      Field {} ", field.id);
        write_quoted(output, &field.name);
        let _ = write!(output, " : {}", field.ty);
        write_span(output, field.span);
        output.push('\n');
    }
    for initializer in &class.initializers {
        let _ = write!(output, "      Initializer {}(", initializer.id);
        dump_types(output, &initializer.parameter_types);
        output.push(')');
        write_span(output, initializer.span);
        output.push('\n');
    }
    for method in &class.methods {
        let _ = write!(output, "      Method {} ", method.id);
        write_quoted(output, &method.name);
        let _ = write!(output, " {} (", method.receiver_access);
        dump_types(output, &method.parameter_types);
        let _ = write!(output, ") -> {}", method.return_type);
        write_span(output, method.span);
        output.push('\n');
    }
}

fn dump_types(output: &mut String, types: &[MirType]) {
    for (index, ty) in types.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        let _ = write!(output, "{ty}");
    }
}

fn dump_declaration(output: &mut String, declaration: &MirFunctionDeclaration) {
    let _ = write!(output, "    Declaration {} ", declaration.id);
    write_quoted(output, &declaration.name);
    match &declaration.linkage {
        MirFunctionLinkage::Internal => output.push_str(" internal"),
        MirFunctionLinkage::External { symbol } => {
            output.push_str(" external ");
            write_quoted(output, symbol);
        }
    }
    write_span(output, declaration.span);
    output.push('\n');
    output.push_str("      Signature (");
    for (index, parameter) in declaration.parameter_types.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        let _ = write!(output, "{parameter}");
    }
    let _ = writeln!(output, ") -> {}", declaration.return_type);
}

fn dump_definition(output: &mut String, function: &MirFunctionDefinition) {
    let _ = write!(output, "    Definition {}", function.function);
    write_span(output, function.span);
    output.push('\n');
    output.push_str("      Parameters");
    for parameter in &function.parameters {
        let _ = write!(output, " {parameter}");
    }
    output.push('\n');
    output.push_str("      Storage\n");
    for storage in &function.storage {
        let kind = match storage.kind {
            MirStorageKind::Parameter => "parameter",
            MirStorageKind::Local => "local",
        };
        let _ = write!(output, "        {} {kind} {} ", storage.id, storage.source);
        write_quoted(output, &storage.name);
        let _ = write!(output, " : {}", storage.ty);
        write_span(output, storage.span);
        output.push('\n');
    }
    output.push_str("      Values\n");
    for value in &function.values {
        let _ = write!(output, "        {} : {}", value.id, value.ty);
        write_span(output, value.span);
        output.push('\n');
    }
    let _ = writeln!(output, "      EntryBlock {}", function.body.entry);
    output.push_str("      Blocks\n");
    for block in &function.body.blocks {
        dump_block(output, block);
    }
}

fn dump_block(output: &mut String, block: &MirBasicBlock) {
    let _ = write!(output, "        {}", block.id);
    write_span(output, block.span);
    output.push('\n');
    for instruction in &block.instructions {
        output.push_str("          ");
        match instruction {
            MirInstruction::Assign(assignment) => {
                let _ = write!(output, "{} = ", assignment.result);
                dump_rvalue(output, &assignment.rvalue);
                write_span(output, assignment.span);
            }
            MirInstruction::Call(call) => {
                if let Some(result) = call.result {
                    let _ = write!(output, "{result} = ");
                }
                match call.target {
                    MirCallTarget::Direct(target) => {
                        let _ = write!(output, "call {target}");
                    }
                    MirCallTarget::Method(target) => {
                        let _ = write!(output, "call {target}");
                    }
                }
                if let Some(receiver) = &call.receiver {
                    output.push_str(" on ");
                    dump_place(output, receiver);
                }
                output.push('(');
                for (index, argument) in call.arguments.iter().enumerate() {
                    if index != 0 {
                        output.push_str(", ");
                    }
                    let _ = write!(output, "{argument}");
                }
                output.push(')');
                write_span(output, call.span);
            }
            MirInstruction::Initialize(initialize) => {
                output.push_str("initialize ");
                dump_place(output, &initialize.destination);
                let _ = write!(output, " with {}(", initialize.target);
                for (index, argument) in initialize.arguments.iter().enumerate() {
                    if index != 0 {
                        output.push_str(", ");
                    }
                    let _ = write!(output, "{argument}");
                }
                output.push(')');
                write_span(output, initialize.span);
            }
            MirInstruction::Store(store) => {
                output.push_str("store ");
                dump_place(output, &store.destination);
                let _ = write!(output, ", {}", store.value);
                write_span(output, store.span);
            }
        }
        output.push('\n');
    }
    output.push_str("          ");
    match &block.terminator {
        Some(MirTerminator::Return { value, span }) => {
            output.push_str("return");
            if let Some(value) = value {
                let _ = write!(output, " {value}");
            }
            write_span(output, *span);
        }
        Some(MirTerminator::Goto { target, span }) => {
            let _ = write!(output, "goto {target}");
            write_span(output, *span);
        }
        Some(MirTerminator::Branch {
            condition,
            true_target,
            false_target,
            span,
        }) => {
            let _ = write!(
                output,
                "branch {condition}, true {true_target}, false {false_target}"
            );
            write_span(output, *span);
        }
        None => output.push_str("<unterminated>"),
    }
    output.push('\n');
}

fn dump_rvalue(output: &mut String, rvalue: &MirRvalue) {
    match &rvalue.kind {
        MirRvalueKind::ConstantI64(value) => {
            let _ = write!(output, "const.i64 {value}");
        }
        MirRvalueKind::ConstantU64(value) => {
            let _ = write!(output, "const.u64 {value}");
        }
        MirRvalueKind::ConstantU8(value) => {
            let _ = write!(output, "const.u8 {value}");
        }
        MirRvalueKind::ConstantF64Bits(bits) => {
            let _ = write!(output, "const.f64 0x{bits:016x}");
        }
        MirRvalueKind::ConstantBool(value) => {
            let _ = write!(output, "const.bool {value}");
        }
        MirRvalueKind::Load(place) => {
            output.push_str("load ");
            dump_place(output, place);
        }
        MirRvalueKind::Unary { operation, operand } => {
            let operation = match operation {
                MirUnaryOperation::NegateI64 => "neg.i64",
                MirUnaryOperation::NegateF64 => "neg.f64",
            };
            let _ = write!(output, "{operation} {operand}");
        }
        MirRvalueKind::Binary {
            operation,
            left,
            right,
        } => {
            let operation = match operation {
                MirBinaryOperation::AddI64 => "add.i64",
                MirBinaryOperation::SubtractI64 => "sub.i64",
                MirBinaryOperation::MultiplyI64 => "mul.i64",
                MirBinaryOperation::AddU64 => "add.u64",
                MirBinaryOperation::SubtractU64 => "sub.u64",
                MirBinaryOperation::MultiplyU64 => "mul.u64",
                MirBinaryOperation::AddU8 => "add.u8",
                MirBinaryOperation::SubtractU8 => "sub.u8",
                MirBinaryOperation::MultiplyU8 => "mul.u8",
                MirBinaryOperation::AddF64 => "add.f64",
                MirBinaryOperation::SubtractF64 => "sub.f64",
                MirBinaryOperation::MultiplyF64 => "mul.f64",
            };
            let _ = write!(output, "{operation} {left}, {right}");
        }
    }
    let _ = write!(output, " : {}", rvalue.ty);
}

fn dump_place(output: &mut String, place: &MirPlace) {
    let _ = write!(output, "{}", place.base);
    for projection in &place.projections {
        match projection {
            MirPlaceProjection::Field(field) => {
                let _ = write!(output, ".field({field})");
            }
        }
    }
}
