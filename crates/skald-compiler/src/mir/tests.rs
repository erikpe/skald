use super::build::{MirBodyBuilder, MirBuildError};
use super::*;
use crate::{
    hir::HirProgram,
    lexer::lex,
    resolve::{resolve, FunctionId},
    source::SourceDatabase,
    syntax::parse,
    typeck::type_check,
};

fn hir_text(text: &str) -> HirProgram {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("test.ska", text);
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty());
    checked.hir.unwrap()
}

fn lower_text(text: &str) -> MirProgram {
    lower_hir(&hir_text(text))
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

#[test]
fn lowers_storage_values_arithmetic_and_return_explicitly() {
    let mir = lower_text("fn main() -> i64 { var result: i64 = 1; return result + 2; }");
    assert!(super::verify_mir(&mir).is_ok());
    let function = mir.definitions.get(mir.entry_function).unwrap();

    assert_eq!(function.storage.len(), 1);
    assert_eq!(function.storage[0].kind, MirStorageKind::Local);
    assert_eq!(function.values.len(), 4);
    let block = function.block(function.body.entry).unwrap();
    assert_eq!(block.instructions.len(), 5);
    assert!(matches!(
        block.instructions[0],
        MirInstruction::Assign(MirAssignment {
            rvalue: MirRvalue {
                kind: MirRvalueKind::ConstantI64(1),
                ..
            },
            ..
        })
    ));
    assert!(matches!(block.instructions[1], MirInstruction::Store(_)));
    assert!(matches!(
        block.instructions[4],
        MirInstruction::Assign(MirAssignment {
            rvalue: MirRvalue {
                kind: MirRvalueKind::Binary {
                    operation: MirBinaryOperation::AddI64,
                    ..
                },
                ..
            },
            ..
        })
    ));
    assert!(matches!(
        block.terminator,
        Some(MirTerminator::Return { .. })
    ));
}

#[test]
fn lowers_u64_constants_storage_arithmetic_calls_and_returns_explicitly() {
    let mir = lower_text(concat!(
        "fn add(left: u64, right: u64) -> u64 { return left + right; }\n",
        "fn main() -> i64 { var value: u64 = add(18446744073709551615u, 2u); return 0; }\n",
    ));
    verify_mir(&mir).unwrap();
    let dump = dump_mir(&mir);

    assert!(dump.contains("Signature (u64, u64) -> u64"));
    assert!(dump.contains("const.u64 18446744073709551615 : u64"));
    assert!(dump.contains("add.u64"));
    assert!(dump.contains("local f1:l0 \"value\" : u64"));
}

#[test]
fn verifier_rejects_u64_constant_and_operation_type_corruption() {
    let mut constant_mismatch =
        lower_text("fn value() -> u64 { return 1u; } fn main() -> i64 { return 0; }");
    let function = constant_mismatch
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected constant assignment");
    };
    assignment.rvalue.ty = MirType::I64;
    assert!(verify_mir(&constant_mismatch)
        .unwrap_err()
        .to_string()
        .contains("u64 constant is not `u64`"));

    let mut operation_mismatch =
        lower_text("fn add() -> u64 { return 1u + 2u; } fn main() -> i64 { return 0; }");
    let function = operation_mismatch
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[2] else {
        panic!("expected binary assignment");
    };
    let MirRvalueKind::Binary { operation, .. } = &mut assignment.rvalue.kind else {
        panic!("expected binary rvalue");
    };
    *operation = MirBinaryOperation::AddI64;
    let errors = verify_mir(&operation_mismatch).unwrap_err().to_string();
    assert!(errors.contains("binary operation result type mismatch"));
    assert!(errors.contains("arithmetic operand is not `i64`"));
}

#[test]
fn lowers_u8_constants_storage_arithmetic_calls_and_returns_explicitly() {
    let mir = lower_text(concat!(
        "fn add(left: u8, right: u8) -> u8 { return left + right; }\n",
        "fn main() -> i64 { var value: u8 = add(255u8, 2u8); return 0; }\n",
    ));
    verify_mir(&mir).unwrap();
    let dump = dump_mir(&mir);

    assert!(dump.contains("Signature (u8, u8) -> u8"));
    assert!(dump.contains("const.u8 255 : u8"));
    assert!(dump.contains("add.u8"));
    assert!(dump.contains("local f1:l0 \"value\" : u8"));
}

#[test]
fn verifier_rejects_u8_constant_and_operation_type_corruption() {
    let mut constant_mismatch =
        lower_text("fn value() -> u8 { return 1u8; } fn main() -> i64 { return 0; }");
    let function = constant_mismatch
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected constant assignment");
    };
    assignment.rvalue.ty = MirType::U64;
    assert!(verify_mir(&constant_mismatch)
        .unwrap_err()
        .to_string()
        .contains("u8 constant is not `u8`"));

    let mut operation_mismatch =
        lower_text("fn add() -> u8 { return 1u8 + 2u8; } fn main() -> i64 { return 0; }");
    let function = operation_mismatch
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[2] else {
        panic!("expected binary assignment");
    };
    let MirRvalueKind::Binary { operation, .. } = &mut assignment.rvalue.kind else {
        panic!("expected binary rvalue");
    };
    *operation = MirBinaryOperation::AddU64;
    let errors = verify_mir(&operation_mismatch).unwrap_err().to_string();
    assert!(errors.contains("binary operation result type mismatch"));
    assert!(errors.contains("arithmetic operand is not `u64`"));
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

#[test]
fn represents_f64_as_raw_bits_and_explicit_typed_operations() {
    let mir = f64_arithmetic_mir();
    verify_mir(&mir).unwrap();
    let dump = dump_mir(&mir);

    assert!(dump.contains("Signature () -> f64"));
    assert!(dump.contains("const.f64 0x3ff0000000000000 : f64"));
    assert!(dump.contains("mul.f64"));
    assert!(dump.contains("add.f64"));
    assert!(dump.contains("sub.f64"));
    assert!(dump.contains("neg.f64"));
}

#[test]
fn lowers_source_f64_constants_storage_arithmetic_calls_and_returns() {
    let mir = lower_text(concat!(
        "extern fn observe(value: f64) -> unit;\n",
        "fn calculate(value: f64) -> f64 { var result: f64 = -(value * 2.0 + 0.5); return result; }\n",
        "fn main() -> i64 { observe(calculate(1.5)); return 0; }\n",
    ));
    verify_mir(&mir).unwrap();
    let dump = dump_mir(&mir);

    assert!(dump.contains("Signature (f64) -> f64"));
    assert!(dump.contains("const.f64 0x4000000000000000 : f64"));
    assert!(dump.contains("mul.f64"));
    assert!(dump.contains("add.f64"));
    assert!(dump.contains("neg.f64"));
    assert!(dump.contains("local f1:l0 \"result\" : f64"));
}

#[test]
fn verifier_rejects_f64_constant_unary_and_binary_corruption() {
    let mut constant = f64_arithmetic_mir();
    let function = constant
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected constant assignment");
    };
    assignment.rvalue.ty = MirType::I64;
    assert!(verify_mir(&constant)
        .unwrap_err()
        .to_string()
        .contains("f64 constant is not `f64`"));

    let mut operations = f64_arithmetic_mir();
    let function = operations
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(binary) = &mut function.body.blocks[0].instructions[3] else {
        panic!("expected binary assignment");
    };
    binary.rvalue.ty = MirType::I64;
    let MirInstruction::Assign(unary) = &mut function.body.blocks[0].instructions[7] else {
        panic!("expected unary assignment");
    };
    unary.rvalue.ty = MirType::I64;
    let errors = verify_mir(&operations).unwrap_err().to_string();
    assert!(errors.contains("binary operation result type mismatch"));
    assert!(errors.contains("unary operation result type mismatch"));
}

#[test]
fn lowers_boolean_constants_storage_calls_and_returns_as_boolean_mir() {
    let mir = lower_text(concat!(
        "extern fn external_flag(value: bool) -> bool;\n",
        "fn identity(value: bool) -> bool { var result: bool = value; return result; }\n",
        "fn main() -> i64 { var flag: bool = identity(true); var external: bool = external_flag(flag); return 0; }\n",
    ));

    assert!(verify_mir(&mir).is_ok());
    let identity_declaration = mir.declarations.get(FunctionId::new(1)).unwrap();
    assert_eq!(identity_declaration.parameter_types, [MirType::Bool]);
    assert_eq!(identity_declaration.return_type, MirType::Bool);
    let identity = mir.definitions.get(FunctionId::new(1)).unwrap();
    assert_eq!(identity.storage[0].ty, MirType::Bool);
    let main = mir.definitions.get(mir.entry_function).unwrap();
    assert!(main.values.iter().any(|value| value.ty == MirType::Bool));
    assert!(main.body.blocks[0].instructions.iter().any(|instruction| {
        matches!(
            instruction,
            MirInstruction::Assign(MirAssignment {
                rvalue: MirRvalue {
                    kind: MirRvalueKind::ConstantBool(true),
                    ty: MirType::Bool,
                    ..
                },
                ..
            })
        )
    }));

    let dump = dump_mir(&mir);
    assert!(dump.contains("const.bool true : bool"));
    assert!(dump.contains("Signature (bool) -> bool"));
}

#[test]
fn lowers_exhaustive_conditionals_without_an_unreachable_join() {
    let mir = lower_text(concat!(
        "fn choose(first: bool, second: bool) -> i64 {\n",
        "  if (first) { return 1; }\n",
        "  elif (second) { return 2; }\n",
        "  else { return 3; }\n",
        "}\n",
        "fn main() -> i64 { return choose(false, true); }\n",
    ));

    assert!(verify_mir(&mir).is_ok());
    let choose = mir.definitions.get(FunctionId::new(0)).unwrap();
    assert_eq!(choose.body.blocks.len(), 5);
    assert!(matches!(
        choose.body.blocks[0].terminator,
        Some(MirTerminator::Branch {
            true_target,
            false_target,
            ..
        }) if true_target == choose.body.blocks[1].id
            && false_target == choose.body.blocks[2].id
    ));
    assert!(matches!(
        choose.body.blocks[2].terminator,
        Some(MirTerminator::Branch {
            true_target,
            false_target,
            ..
        }) if true_target == choose.body.blocks[3].id
            && false_target == choose.body.blocks[4].id
    ));
    for index in [1, 3, 4] {
        assert!(matches!(
            choose.body.blocks[index].terminator,
            Some(MirTerminator::Return { .. })
        ));
    }

    let dump = dump_mir(&mir);
    let control_flow: Vec<_> = dump
        .lines()
        .filter(|line| {
            line.contains("EntryBlock f0:b")
                || line.trim_start().starts_with("f0:b")
                || line.trim_start().starts_with("branch f0:")
                || line.trim_start().starts_with("return f0:")
        })
        .map(|line| line.split(" @").next().unwrap().trim())
        .collect();
    assert_eq!(
        control_flow,
        [
            "EntryBlock f0:b0",
            "f0:b0",
            "branch f0:v0, true f0:b1, false f0:b2",
            "f0:b1",
            "return f0:v1",
            "f0:b2",
            "branch f0:v2, true f0:b3, false f0:b4",
            "f0:b3",
            "return f0:v3",
            "f0:b4",
            "return f0:v4",
        ]
    );
}

#[test]
fn lowers_fallthrough_arms_through_storage_to_one_join() {
    let mir = lower_text(concat!(
        "fn main() -> i64 {\n",
        "  var result: i64 = 7;\n",
        "  if (true) {} else {}\n",
        "  return result;\n",
        "}\n",
    ));

    assert!(verify_mir(&mir).is_ok());
    let main = mir.definitions.get(mir.entry_function).unwrap();
    assert_eq!(main.body.blocks.len(), 4);
    let join = main.body.blocks[3].id;
    assert!(main.body.blocks[1..=2].iter().all(|block| {
        matches!(block.terminator, Some(MirTerminator::Goto { target, .. }) if target == join)
    }));
    assert!(matches!(
        main.body.blocks[3].terminator,
        Some(MirTerminator::Return { .. })
    ));
}

#[test]
fn lowers_condition_calls_in_source_order_on_the_false_continuation_chain() {
    let mir = lower_text(concat!(
        "fn first() -> bool { return false; }\n",
        "fn second() -> bool { return true; }\n",
        "fn main() -> i64 {\n",
        "  if (first()) { return 1; }\n",
        "  elif (second()) { return 2; }\n",
        "  else { return 3; }\n",
        "}\n",
    ));

    assert!(verify_mir(&mir).is_ok());
    let main = mir.definitions.get(mir.entry_function).unwrap();
    let targets: Vec<_> = main
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Call(MirCall {
                target: MirCallTarget::Direct(target),
                ..
            }) => Some(*target),
            _ => None,
        })
        .collect();
    assert_eq!(targets, [FunctionId::new(0), FunctionId::new(1)]);
    assert!(matches!(
        main.body.blocks[0].terminator,
        Some(MirTerminator::Branch { false_target, .. })
            if false_target == main.body.blocks[2].id
    ));
}

#[test]
fn verifier_rejects_a_boolean_constant_with_a_non_boolean_result() {
    let mut mir = lower_text("fn flag() -> bool { return true; } fn main() -> i64 { return 0; }");
    let flag = mir
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut flag.body.blocks[0].instructions[0] else {
        panic!("expected constant assignment");
    };
    assignment.rvalue.ty = MirType::I64;

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("boolean constant is not `bool`")));
}

#[test]
fn nested_call_arguments_lower_in_deterministic_left_to_right_order() {
    let mir = lower_text(concat!(
        "fn left() -> i64 { return 1; }\n",
        "fn right() -> i64 { return 2; }\n",
        "fn combine(a: i64, b: i64) -> i64 { return a + b; }\n",
        "fn main() -> i64 { return combine(left(), right()); }\n",
    ));
    let main = mir.definitions.get(mir.entry_function).unwrap();
    let block = main.block(main.body.entry).unwrap();
    let calls: Vec<_> = block
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            MirInstruction::Call(MirCall {
                target: MirCallTarget::Direct(function),
                ..
            }) => Some(*function),
            _ => None,
        })
        .collect();

    assert_eq!(
        calls.iter().map(|id| id.index()).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert!(dump_mir(&mir).contains("call f2("));
}

#[test]
fn lowers_unit_calls_and_returns_without_payload_values() {
    let mir = lower_text(concat!(
        "fn explicit(value: i64) -> unit { return; }\n",
        "fn implicit() -> unit {}\n",
        "fn main() -> i64 { explicit(7); implicit(); return 3; }\n",
    ));

    assert!(verify_mir(&mir).is_ok());
    for id in [FunctionId::new(0), FunctionId::new(1)] {
        let function = mir.definitions.get(id).unwrap();
        assert!(matches!(
            function.body.blocks[0].terminator,
            Some(MirTerminator::Return { value: None, .. })
        ));
        assert!(function
            .values
            .iter()
            .all(|value| value.ty != MirType::Unit));
    }
    let main = mir.definitions.get(mir.entry_function).unwrap();
    let calls: Vec<_> = main.body.blocks[0]
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .collect();
    assert_eq!(calls.len(), 2);
    assert!(calls.iter().all(|call| call.result.is_none()));
    let dump = dump_mir(&mir);
    assert_eq!(dump, dump_mir(&mir));
    assert!(dump.contains("call f0(f2:v0)"));
    assert!(dump.contains("return @"));
}

#[test]
fn lowering_discards_statements_after_an_unconditional_return() {
    let mir = lower_text("fn main() -> i64 { { return 1; } return 2; }");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    let block = main.block(main.body.entry).unwrap();

    assert_eq!(main.values.len(), 1);
    assert_eq!(block.instructions.len(), 1);
    assert!(matches!(
        block.terminator,
        Some(MirTerminator::Return { .. })
    ));
}

#[test]
fn mir_dump_is_deterministic() {
    let mir = lower_text("fn main() -> i64 { return 42; }");

    assert_eq!(
        super::dump_mir(&mir),
        concat!(
            "MirProgram @0..31\n",
            "  Entry f0\n",
            "  Declarations\n",
            "    Declaration f0 \"main\" internal @0..31\n",
            "      Signature () -> i64\n",
            "  Definitions\n",
            "    Definition f0 @0..31\n",
            "      Parameters\n",
            "      Storage\n",
            "      Values\n",
            "        f0:v0 : i64 @26..28\n",
            "      EntryBlock f0:b0\n",
            "      Blocks\n",
            "        f0:b0 @17..31\n",
            "          f0:v0 = const.i64 42 : i64 @26..28\n",
            "          return f0:v0 @19..29\n",
        )
    );
}

#[test]
fn body_builder_allocates_and_selects_blocks_in_stable_order() {
    let mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir.definitions.get(mir.entry_function).unwrap();
    let mut builder = MirBodyBuilder::new(function.function, function.span);
    let entry = builder.entry();
    let second = builder.allocate_block(function.span);
    let third = builder.allocate_block(function.span);

    assert_eq!(builder.current(), entry);
    assert_eq!(second.index(), 1);
    assert_eq!(third.index(), 2);
    builder.select_block(third).unwrap();
    assert_eq!(builder.current(), third);
    let body = builder.finish();
    assert_eq!(body.entry, entry);
    assert_eq!(
        body.blocks.iter().map(|block| block.id).collect::<Vec<_>>(),
        [entry, second, third]
    );
}

#[test]
fn body_builder_rejects_emission_and_duplicate_termination_after_a_terminator() {
    let mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir.definitions.get(mir.entry_function).unwrap();
    let mut builder = MirBodyBuilder::new(function.function, function.span);
    let entry = builder.entry();
    builder
        .terminate(MirTerminator::Return {
            value: None,
            span: function.span,
        })
        .unwrap();

    assert_eq!(
        builder
            .terminate(MirTerminator::Return {
                value: None,
                span: function.span,
            })
            .unwrap_err(),
        MirBuildError::BlockAlreadyTerminated(entry)
    );
    assert_eq!(
        builder
            .push_instruction(MirInstruction::Store(MirStore {
                storage: StorageId::new(function.function, 0),
                value: ValueId::new(function.function, 0),
                span: function.span,
            }))
            .unwrap_err(),
        MirBuildError::BlockAlreadyTerminated(entry)
    );
}

#[test]
fn body_builder_rejects_unknown_and_foreign_block_selection() {
    let mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir.definitions.get(mir.entry_function).unwrap();
    let mut builder = MirBodyBuilder::new(function.function, function.span);

    for unknown in [
        BlockId::new(function.function, 1),
        BlockId::new(FunctionId::new(99), 0),
    ] {
        assert_eq!(
            builder.select_block(unknown).unwrap_err(),
            MirBuildError::UnknownBlock(unknown)
        );
    }
}

#[test]
fn verifies_jumps_joins_diamonds_and_multiple_returns() {
    let join = goto_join_mir();
    let diamond = diamond_mir();

    assert!(verify_mir(&join).is_ok());
    assert!(verify_mir(&diamond).is_ok());
    assert!(dump_mir(&join).contains("goto f0:b1"));
    let join_function = join.definitions.get(join.entry_function).unwrap();
    assert_eq!(
        join_function.body.blocks[0]
            .terminator
            .as_ref()
            .unwrap()
            .successors()
            .collect::<Vec<_>>(),
        [join_function.body.blocks[1].id]
    );
    let function = diamond.definitions.get(diamond.entry_function).unwrap();
    let successors: Vec<_> = function.body.blocks[0]
        .terminator
        .as_ref()
        .unwrap()
        .successors()
        .collect();
    assert_eq!(
        successors,
        [function.body.blocks[1].id, function.body.blocks[2].id]
    );
    assert_eq!(
        function.body.blocks[1]
            .terminator
            .as_ref()
            .unwrap()
            .successors()
            .count(),
        0
    );
}

#[test]
fn control_flow_dump_is_exact_and_deterministic() {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    function.values[0].ty = MirType::Bool;
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected constant assignment");
    };
    assignment.rvalue = MirRvalue {
        kind: MirRvalueKind::ConstantBool(true),
        ty: MirType::Bool,
    };
    let condition = assignment.result;
    let block = function.body.blocks[0].id;
    function.body.blocks[0].terminator = Some(MirTerminator::Branch {
        condition,
        true_target: block,
        false_target: block,
        span: function.span,
    });

    assert!(verify_mir(&mir).is_ok());
    let expected = concat!(
        "MirProgram @0..30\n",
        "  Entry f0\n",
        "  Declarations\n",
        "    Declaration f0 \"main\" internal @0..30\n",
        "      Signature () -> i64\n",
        "  Definitions\n",
        "    Definition f0 @0..30\n",
        "      Parameters\n",
        "      Storage\n",
        "      Values\n",
        "        f0:v0 : bool @26..27\n",
        "      EntryBlock f0:b0\n",
        "      Blocks\n",
        "        f0:b0 @17..30\n",
        "          f0:v0 = const.bool true : bool @26..27\n",
        "          branch f0:v0, true f0:b0, false f0:b0 @0..30\n",
    );
    assert_eq!(dump_mir(&mir), expected);
    assert_eq!(dump_mir(&mir), dump_mir(&mir));
}

#[test]
fn verifier_rejects_missing_and_foreign_control_flow_targets() {
    let mut missing = goto_join_mir();
    let function = missing
        .definitions
        .get_mut_for_test(missing.entry_function)
        .unwrap();
    function.body.blocks[0].terminator = Some(MirTerminator::Goto {
        target: BlockId::new(function.function, 99),
        span: function.span,
    });
    let errors = verify_mir(&missing).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("target f0:b99 is not declared")));

    let mut foreign = goto_join_mir();
    let function = foreign
        .definitions
        .get_mut_for_test(foreign.entry_function)
        .unwrap();
    function.body.blocks[0].terminator = Some(MirTerminator::Goto {
        target: BlockId::new(FunctionId::new(99), 0),
        span: function.span,
    });
    let errors = verify_mir(&foreign).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("target f99:b0 is owned by another function")));
}

#[test]
fn verifier_rejects_invalid_entry_and_non_dense_block_ids() {
    let mut missing_entry = goto_join_mir();
    let function = missing_entry
        .definitions
        .get_mut_for_test(missing_entry.entry_function)
        .unwrap();
    function.body.entry = BlockId::new(function.function, 99);
    let errors = verify_mir(&missing_entry).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("entry block f0:b99 is not declared")));

    let mut foreign_entry = goto_join_mir();
    let function = foreign_entry
        .definitions
        .get_mut_for_test(foreign_entry.entry_function)
        .unwrap();
    function.body.entry = BlockId::new(FunctionId::new(99), 0);
    let errors = verify_mir(&foreign_entry).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("entry block f99:b0 is owned by another function")));

    let mut sparse = goto_join_mir();
    let function = sparse
        .definitions
        .get_mut_for_test(sparse.entry_function)
        .unwrap();
    function.body.blocks[1].id = BlockId::new(function.function, 2);
    let errors = verify_mir(&sparse).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("block table index 1 contains f0:b2")));
}

#[test]
fn verifier_requires_a_boolean_branch_condition() {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let condition = function.values[0].id;
    let target = function.body.entry;
    function.body.blocks[0].terminator = Some(MirTerminator::Branch {
        condition,
        true_target: target,
        false_target: target,
        span: function.span,
    });

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("branch condition is not `bool`")));
}

#[test]
fn verifier_rejects_transient_values_used_across_block_boundaries() {
    let mut mir = goto_join_mir();
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let entry_value = function.values[0].id;
    let join = &mut function.body.blocks[1];
    join.terminator = Some(MirTerminator::Return {
        value: Some(entry_value),
        span: join.span,
    });

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("used before it is defined in this block")));
}

#[test]
fn verifier_checks_unreachable_blocks() {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let function_id = function.function;
    function.body.blocks.push(MirBasicBlock {
        id: BlockId::new(function_id, 1),
        instructions: Vec::new(),
        terminator: None,
        span: function.span,
    });

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors.iter().any(|error| {
        error.block == Some(BlockId::new(function_id, 1)) && error.message.contains("no terminator")
    }));
}

#[test]
fn verifier_rejects_unterminated_blocks() {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    mir.definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .terminator = None;

    let errors = super::verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("no terminator")));
}

#[test]
fn verifier_rejects_use_before_definition() {
    let mut mir = lower_text("fn main() -> i64 { return 1 + 2; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    function.body.blocks[0].instructions.swap(0, 2);

    let errors = super::verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("used before it is defined")));
}

#[test]
fn verifier_rejects_a_value_defined_in_terms_of_itself() {
    let mut mir = lower_text("fn main() -> i64 { return 1; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected assignment");
    };
    assignment.rvalue.kind = MirRvalueKind::Unary {
        operation: MirUnaryOperation::NegateI64,
        operand: assignment.result,
    };

    let errors = super::verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("used before it is defined")));
}

#[test]
fn verifier_rejects_call_signature_mismatches() {
    let mut mir = lower_text(concat!(
        "fn one(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { return one(1); }\n",
    ));
    let main = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let call = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(&mut call.arguments),
            _ => None,
        })
        .unwrap();
    call.clear();

    let errors = super::verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("has 0 arguments but requires 1")));
}

#[test]
fn verifier_rejects_ids_owned_by_another_function() {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let foreign = FunctionId::new(99);
    mir.definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap()
        .values[0]
        .id = ValueId::new(foreign, 0);

    let errors = super::verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("owned by another function")));
}

#[test]
fn verifier_accepts_an_external_declaration_without_a_definition() {
    let mir = lower_text(concat!(
        "extern fn foreign(value: i64) -> i64;\n",
        "fn main() -> i64 { return foreign(7); }\n",
    ));
    let foreign = FunctionId::new(0);

    assert!(verify_mir(&mir).is_ok());
    assert!(mir.declarations.get(foreign).is_some());
    assert!(mir.definitions.get(foreign).is_none());
    assert!(dump_mir(&mir).contains("Declaration f0 \"foreign\" external \"foreign\""));
}

#[test]
fn verifier_rejects_invalid_external_symbol_metadata() {
    let mut mir = lower_text(concat!(
        "extern fn foreign(value: i64) -> i64;\n",
        "fn main() -> i64 { return foreign(7); }\n",
    ));
    let foreign = FunctionId::new(0);
    mir.declarations.entries_mut_for_test()[foreign.index()].linkage =
        MirFunctionLinkage::External {
            symbol: "wrong-symbol".to_owned(),
        };

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("external symbol must be the declaration's exact source identifier")));
}

#[test]
fn verifier_checks_external_call_signature_and_result_presence() {
    let mut mir = lower_text(concat!(
        "extern fn foreign(value: i64) -> i64;\n",
        "fn main() -> i64 { return foreign(7); }\n",
    ));
    let main = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let call = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    call.arguments.clear();
    call.result = None;

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("has 0 arguments but requires 1")));
    assert!(errors
        .iter()
        .any(|error| error.message.contains("value-returning call has no result")));
}

#[test]
fn verifier_rejects_an_internal_declaration_without_a_definition() {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    mir.definitions.remove_for_test(mir.entry_function);

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("entry function f0 has no definition")));
    assert!(errors.iter().any(|error| error
        .message
        .contains("internal function has no definition")));
}

#[test]
fn verifier_rejects_an_unknown_call_target() {
    let mut mir = lower_text(concat!(
        "fn helper() -> i64 { return 1; }\n",
        "fn main() -> i64 { return helper(); }\n",
    ));
    let main = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let call = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    call.target = MirCallTarget::Direct(FunctionId::new(99));

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("call target f99 is not declared")));
}

#[test]
fn verifier_rejects_a_missing_value_call_result() {
    let mut mir = lower_text(concat!(
        "fn helper() -> i64 { return 1; }\n",
        "fn main() -> i64 { return helper(); }\n",
    ));
    let main = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let call = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    call.result = None;

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("value-returning call has no result")));
}

#[test]
fn verifier_rejects_a_result_on_a_unit_call() {
    let mut mir = lower_text(concat!(
        "fn notify() -> unit {}\n",
        "fn main() -> i64 { notify(); return 0; }\n",
    ));
    let main = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let result = ValueId::new(main.function, main.values.len());
    main.values.push(MirValue {
        id: result,
        ty: MirType::I64,
        span: main.span,
    });
    let call = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    call.result = Some(result);

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("unit-returning call must not have a result")));
}

#[test]
fn verifier_rejects_return_operand_presence_mismatches() {
    let mut unit_with_value = lower_text(concat!(
        "fn helper() -> i64 { return 1; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    unit_with_value.declarations.entries_mut_for_test()[0].return_type = MirType::Unit;
    let errors = verify_mir(&unit_with_value).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("unit function return must not have an operand")));

    let mut value_without_operand = lower_text("fn main() -> i64 { return 0; }");
    let main = value_without_operand
        .definitions
        .get_mut_for_test(value_without_operand.entry_function)
        .unwrap();
    let Some(MirTerminator::Return { value, .. }) = &mut main.body.blocks[0].terminator else {
        panic!("expected return terminator");
    };
    *value = None;
    let errors = verify_mir(&value_without_operand).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("value-returning function return has no operand")));
}

#[test]
fn verifier_rejects_definition_signature_mismatches() {
    let mut mir = lower_text(concat!(
        "fn helper(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { return helper(1); }\n",
    ));
    mir.definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap()
        .parameters
        .clear();

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("definition has 0 parameters but declaration requires 1")));
}
