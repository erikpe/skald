use super::build::{MirBodyBuilder, MirBuildError};
use super::*;
use crate::{
    identity::{BindingId, ClassId, FieldId, FunctionId, InitializerId, LocalId, MethodId},
    test_support::lower_source_to_mir,
};

mod alias_lowering;
mod aliases;
mod object_fixtures;
mod objects;

fn lower_text(text: &str) -> MirProgram {
    lower_source_to_mir(text)
}

fn goto_join_mir() -> MirProgram {
    let mut mir = lower_text("fn main() -> i64 { var result: i64 = 0; return result; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let entry = &mut function.body.blocks[0];
    let join_id = BlockId::new(function.function, 1);
    let join_instructions = entry.instructions.split_off(2);
    let join_terminator = entry.terminator.take();
    entry.terminator = Some(MirTerminator::Goto {
        target: join_id,
        span: entry.span,
    });
    function.body.blocks.push(MirBasicBlock {
        id: join_id,
        instructions: join_instructions,
        terminator: join_terminator,
        span: function.span,
    });
    mir
}

fn diamond_mir() -> MirProgram {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let span = function.span;
    let original = function.body.blocks.pop().unwrap();
    let condition = ValueId::new(function.function, function.values.len());
    function.values.push(MirValue {
        id: condition,
        ty: MirType::Bool,
        span,
    });
    let false_value = ValueId::new(function.function, function.values.len());
    function.values.push(MirValue {
        id: false_value,
        ty: MirType::I64,
        span,
    });
    let entry = BlockId::new(function.function, 0);
    let true_block = BlockId::new(function.function, 1);
    let false_block = BlockId::new(function.function, 2);
    function.body.blocks = vec![
        MirBasicBlock {
            id: entry,
            instructions: vec![MirInstruction::Assign(MirAssignment {
                result: condition,
                rvalue: MirRvalue {
                    kind: MirRvalueKind::ConstantBool(true),
                    ty: MirType::Bool,
                },
                span,
            })],
            terminator: Some(MirTerminator::Branch {
                condition,
                true_target: true_block,
                false_target: false_block,
                span,
            }),
            span,
        },
        MirBasicBlock {
            id: true_block,
            instructions: original.instructions,
            terminator: original.terminator,
            span,
        },
        MirBasicBlock {
            id: false_block,
            instructions: vec![MirInstruction::Assign(MirAssignment {
                result: false_value,
                rvalue: MirRvalue {
                    kind: MirRvalueKind::ConstantI64(1),
                    ty: MirType::I64,
                },
                span,
            })],
            terminator: Some(MirTerminator::Return {
                value: Some(false_value),
                span,
            }),
            span,
        },
    ];
    mir
}

fn f64_arithmetic_mir() -> MirProgram {
    let mut mir = lower_text(
        "fn calculate() -> i64 { return -(1 + 2 * 3 - 4); } fn main() -> i64 { return 0; }",
    );
    mir.declarations.entries_mut_for_test()[0].return_type = MirType::F64;
    let function = mir
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    for value in &mut function.values {
        value.ty = MirType::F64;
    }
    for instruction in &mut function.body.blocks[0].instructions {
        let MirInstruction::Assign(assignment) = instruction else {
            continue;
        };
        assignment.rvalue.ty = MirType::F64;
        match &mut assignment.rvalue.kind {
            MirRvalueKind::ConstantI64(value) => {
                assignment.rvalue.kind = MirRvalueKind::ConstantF64Bits((*value as f64).to_bits());
            }
            MirRvalueKind::Unary { operation, .. } => {
                *operation = MirUnaryOperation::NegateF64;
            }
            MirRvalueKind::Binary { operation, .. } => {
                *operation = match operation {
                    MirBinaryOperation::AddI64 => MirBinaryOperation::AddF64,
                    MirBinaryOperation::SubtractI64 => MirBinaryOperation::SubtractF64,
                    MirBinaryOperation::MultiplyI64 => MirBinaryOperation::MultiplyF64,
                    _ => unreachable!("test source uses only integer arithmetic"),
                };
            }
            _ => unreachable!("test source lowers only arithmetic rvalues"),
        }
    }
    mir
}

mod builder;
mod control_flow;
mod dump;
mod inline_fields;
mod lowering;
mod verification;
