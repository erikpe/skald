use super::build::{MirBodyBuilder, MirBuildError};
use super::test_fixtures::{
    add_body_storage_lifetimes as fixture_add_body_storage_lifetimes, assign as fixture_assign,
    block as fixture_block, checked_shift_program as fixture_checked_shift_program,
    conditional_full_expression_cleanup_program as fixture_conditional_cleanup_program,
    empty_member_definition as fixture_empty_member_definition, storage as fixture_storage,
    storage_dead as fixture_storage_dead, storage_live as fixture_storage_live,
    store as fixture_store, value as fixture_value,
};
use super::*;
use crate::{
    identity::{
        BindingId, ClassId, CopyConstructorId, FieldId, FunctionId, InitializerId, LocalId,
        MethodId,
    },
    test_support::{lower_source_to_mir, type_check_source},
};

mod alias_fixtures;
mod alias_lowering;
mod aliases;
mod arrays;
mod bitwise;
mod comparisons;
mod conditional_cleanup;
mod copy;
mod cycles;
mod integer_casts;
mod interface_dispatch;
mod interface_fixtures;
mod logical_boundaries;
mod logical_expressions;
mod logical_fixtures;
mod logical_object_lifetimes;
mod logical_shared_array_lifetimes;
mod logical_stress;
mod logical_verification;
mod loop_hardening;
mod object_fixtures;
mod object_results;
mod object_temporaries;
mod objects;
mod optional_values;
mod path_conditions;
mod primitive_binding_assignment;
mod robustness;
mod shared;
mod shifts;
mod static_inheritance;
mod static_methods;
mod storage_lifetimes;
mod strings;
mod type_operation_fixtures;
mod type_operations;
mod value_parameters;
mod virtual_dispatch;
mod virtual_fixtures;
mod while_loops;

fn lower_text(text: &str) -> MirProgram {
    lower_source_to_mir(text)
}

fn static_inheritance_mir() -> MirProgram {
    lower_text(concat!(
        "class Base { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Mid extends Base { init(value: i64) { super(value); } }\n",
        "class Derived extends Mid { init(value: i64) { super(value); } }\n",
        "fn inspect(ref base: Base, ref any: Obj) -> unit {}\n",
        "fn consume(value: Base) -> unit {}\n",
        "fn relay(ref value: Derived) -> unit { inspect(value, value); }\n",
        "fn main() -> i64 {\n",
        "  var value: Derived = Derived(7);\n",
        "  relay(value);\n",
        "  consume(value);\n",
        "  return value.value;\n",
        "}\n",
    ))
}

fn goto_join_mir() -> MirProgram {
    let mut mir = lower_text("fn main() -> i64 { var result: i64 = 0; return result; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let entry = &mut function.body.blocks[0];
    let join_id = BlockId::new(function.function, 1);
    let join_start = entry
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                MirInstruction::Assign(MirAssignment {
                    rvalue: MirRvalue {
                        kind: MirRvalueKind::Load(_),
                        ..
                    },
                    ..
                })
            )
        })
        .expect("fixture return must load the local in the join block");
    let join_instructions = entry.instructions.split_off(join_start);
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
    function
        .values
        .push(fixture_value(condition, MirType::Bool, span));
    let false_value = ValueId::new(function.function, function.values.len());
    function
        .values
        .push(fixture_value(false_value, MirType::I64, span));
    let entry = BlockId::new(function.function, 0);
    let true_block = BlockId::new(function.function, 1);
    let false_block = BlockId::new(function.function, 2);
    function.body.blocks = vec![
        fixture_block(
            entry,
            vec![fixture_assign(
                condition,
                MirRvalueKind::ConstantBool(true),
                MirType::Bool,
                span,
            )],
            Some(MirTerminator::Branch {
                condition,
                true_target: true_block,
                false_target: false_block,
                span,
            }),
            span,
        ),
        fixture_block(true_block, original.instructions, original.terminator, span),
        fixture_block(
            false_block,
            vec![fixture_assign(
                false_value,
                MirRvalueKind::ConstantI64(1),
                MirType::I64,
                span,
            )],
            Some(MirTerminator::Return {
                value: Some(false_value),
                span,
            }),
            span,
        ),
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
mod destruction;
mod dump;
mod inline_fields;
mod lowering;
mod verification;
