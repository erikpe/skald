//! Deterministic textual rendering of MIR.

use std::fmt::Write;

use crate::source::Span;

use super::model::*;

pub fn dump_mir(program: &MirProgram) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "MirProgram @{}..{}",
        program.span.range().start(),
        program.span.range().end()
    );
    let _ = writeln!(output, "  Entry {}", program.entry_function);
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

fn dump_declaration(output: &mut String, declaration: &MirFunctionDeclaration) {
    let _ = writeln!(
        output,
        "    Declaration {} \"{}\" {} @{}..{}",
        declaration.id,
        escape(&declaration.name),
        match &declaration.linkage {
            MirFunctionLinkage::Internal => "internal".to_owned(),
            MirFunctionLinkage::External { symbol } => {
                format!("external \"{}\"", escape(symbol))
            }
        },
        declaration.span.range().start(),
        declaration.span.range().end()
    );
    output.push_str("      Signature (");
    for (index, parameter) in declaration.parameter_types.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        output.push_str(parameter.name());
    }
    let _ = writeln!(output, ") -> {}", declaration.return_type.name());
}

fn dump_definition(output: &mut String, function: &MirFunctionDefinition) {
    let _ = writeln!(
        output,
        "    Definition {} @{}..{}",
        function.function,
        function.span.range().start(),
        function.span.range().end()
    );
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
        let _ = writeln!(
            output,
            "        {} {kind} {} \"{}\" : {} @{}..{}",
            storage.id,
            storage.source,
            escape(&storage.name),
            storage.ty.name(),
            storage.span.range().start(),
            storage.span.range().end()
        );
    }
    output.push_str("      Values\n");
    for value in &function.values {
        let _ = writeln!(
            output,
            "        {} : {} @{}..{}",
            value.id,
            value.ty.name(),
            value.span.range().start(),
            value.span.range().end()
        );
    }
    let _ = writeln!(output, "      EntryBlock {}", function.body.entry);
    output.push_str("      Blocks\n");
    for block in &function.body.blocks {
        dump_block(output, block);
    }
}

fn dump_block(output: &mut String, block: &MirBasicBlock) {
    let _ = writeln!(
        output,
        "        {} @{}..{}",
        block.id,
        block.span.range().start(),
        block.span.range().end()
    );
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
                let MirCallTarget::Direct(target) = call.target;
                let _ = write!(output, "call {target}(");
                for (index, argument) in call.arguments.iter().enumerate() {
                    if index != 0 {
                        output.push_str(", ");
                    }
                    let _ = write!(output, "{argument}");
                }
                output.push(')');
                write_span(output, call.span);
            }
            MirInstruction::Store(store) => {
                let _ = write!(output, "store {}, {}", store.storage, store.value);
                write_span(output, store.span);
            }
        }
        output.push('\n');
    }
    output.push_str("          ");
    match &block.terminator {
        Some(MirTerminator::Return { value, span }) => {
            let _ = write!(output, "return {value}");
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
        MirRvalueKind::Load(storage) => {
            let _ = write!(output, "load {storage}");
        }
        MirRvalueKind::Unary { operation, operand } => {
            let operation = match operation {
                MirUnaryOperation::NegateI64 => "neg.i64",
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
            };
            let _ = write!(output, "{operation} {left}, {right}");
        }
    }
    let _ = write!(output, " : {}", rvalue.ty.name());
}

fn write_span(output: &mut String, span: Span) {
    let _ = write!(output, " @{}..{}", span.range().start(), span.range().end());
}

fn escape(text: &str) -> String {
    text.chars().flat_map(char::escape_default).collect()
}
