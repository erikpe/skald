use super::*;
use crate::hir::{
    dump_hir, HirExpressionKind, HirLocalInitializer, HirPrimitiveCast, HirPrimitiveType,
    HirStatement,
};

const PRIMITIVE_TYPES: &[(HirPrimitiveType, MirPrimitiveType, &str)] = &[
    (HirPrimitiveType::I64, MirPrimitiveType::I64, "i64"),
    (HirPrimitiveType::U64, MirPrimitiveType::U64, "u64"),
    (HirPrimitiveType::U8, MirPrimitiveType::U8, "u8"),
    (HirPrimitiveType::F64, MirPrimitiveType::F64, "f64"),
    (HirPrimitiveType::Bool, MirPrimitiveType::Bool, "bool"),
];

#[test]
fn lowers_and_verifies_the_complete_source_enabled_matrix() {
    let mut implemented_pairs = 0;
    for &(_, source_type, source_name) in PRIMITIVE_TYPES {
        for &(_, target_type, target_name) in PRIMITIVE_TYPES {
            implemented_pairs += 1;
            let operand = match source_type {
                MirPrimitiveType::I64 => "-1",
                MirPrimitiveType::U64 => "18446744073709551615u",
                MirPrimitiveType::U8 => "255u8",
                MirPrimitiveType::F64 => "-0.0",
                MirPrimitiveType::Bool => "true",
            };
            let source = format!(
                "fn cast() -> {target_name} {{ return ({target_name}) {operand}; }} \
                 fn main() -> i64 {{ return 0; }}"
            );
            let mir = lower_text(&source);
            verify_mir(&mir).unwrap();
            let operation = MirPrimitiveCast::new(source_type, target_type);
            if operation.may_terminate() {
                let definition = mir.definitions.get(FunctionId::new(0)).unwrap();
                let relation = definition
                    .body
                    .blocks
                    .iter()
                    .find_map(|block| match block.terminator {
                        Some(MirTerminator::PrimitiveCastRangeCheck { check, .. }) => {
                            Some(check.relation)
                        }
                        _ => None,
                    })
                    .expect("checked source cast must lower to its range diamond");
                assert_eq!(relation.operation(), operation);
            } else {
                let cast = primitive_cast(&mir);
                assert_eq!(cast.0, operation);
                assert_eq!(cast.0.source_type(), source_type.value_type());
                assert_eq!(cast.0.result_type(), target_type.value_type());
                assert_eq!(cast.2, target_type.value_type());
                assert!(mir
                    .definitions
                    .get(FunctionId::new(0))
                    .unwrap()
                    .values
                    .iter()
                    .any(|value| value.id == cast.1 && value.ty == source_type.value_type()));
            }

            let dump = dump_mir(&mir);
            assert_eq!(dump, dump_mir(&mir));
            assert!(dump.contains(&format!("cast.{source_name}.{target_name}")));
        }
    }
    assert_eq!(implemented_pairs, 25);
}

#[test]
fn direct_hir_lowering_covers_all_twenty_two_pure_cast_cells() {
    let mut count = 0;
    for &(hir_source, mir_source, _) in PRIMITIVE_TYPES {
        for &(hir_target, mir_target, _) in PRIMITIVE_TYPES {
            let operation = HirPrimitiveCast::new(hir_source, hir_target);
            if operation.may_terminate() {
                continue;
            }
            count += 1;

            let mut hir = primitive_cast_hir();
            let definition = hir
                .definitions
                .get_mut_for_test(FunctionId::new(0))
                .unwrap();
            definition.locals[0].ty = hir_target.value_type();
            let HirStatement::Local(local) = &mut definition.body.statements[0] else {
                panic!("fixture must start with a local declaration");
            };
            let HirLocalInitializer::Value(expression) = &mut local.initializer else {
                panic!("fixture local must have a scalar initializer");
            };
            let HirExpressionKind::PrimitiveCast {
                operation: cast,
                operand,
            } = &mut expression.kind
            else {
                panic!("fixture initializer must be a primitive cast");
            };
            *cast = operation;
            operand.kind = hir_literal(hir_source);
            operand.ty = hir_source.value_type();
            expression.ty = hir_target.value_type();

            let mir = lower_hir(&hir);
            verify_mir(&mir).unwrap();
            let (cast, _, result_type) = primitive_cast(&mir);
            assert_eq!(cast, MirPrimitiveCast::new(mir_source, mir_target));
            assert_eq!(result_type, mir_target.value_type());
        }
    }
    assert_eq!(count, 22);
}

#[test]
fn directly_constructed_mir_verifies_all_twenty_two_pure_cast_cells() {
    let mut count = 0;
    for &(_, source, _) in PRIMITIVE_TYPES {
        for &(_, target, _) in PRIMITIVE_TYPES {
            let operation = MirPrimitiveCast::new(source, target);
            if operation.may_terminate() {
                continue;
            }
            count += 1;
            let mir = primitive_cast_mir(source, target);
            verify_mir(&mir).unwrap();

            let dump = dump_mir(&mir);
            assert_eq!(dump, dump_mir(&mir));
            assert!(dump.contains(&format!("cast.{}.{}", source.name(), target.name())));
        }
    }
    assert_eq!(count, 22);
}

#[test]
fn pure_cast_operand_is_lowered_once_and_adds_no_control_effect() {
    let mir = lower_text(
        "fn source() -> u64 { return 7u; }\n\
         fn cast() -> f64 { return (f64) source(); }\n\
         fn main() -> i64 { return 0; }\n",
    );
    verify_mir(&mir).unwrap();
    let function = mir.definitions.get(FunctionId::new(1)).unwrap();

    assert_eq!(
        function
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| matches!(instruction, MirInstruction::Call(_)))
            .count(),
        1
    );
    assert_eq!(
        function
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| matches!(
                instruction,
                MirInstruction::Assign(MirAssignment {
                    rvalue: MirRvalue {
                        kind: MirRvalueKind::PrimitiveCast { .. },
                        ..
                    },
                    ..
                })
            ))
            .count(),
        1
    );
    assert!(!function
        .storage
        .iter()
        .any(|storage| storage.kind == MirStorageKind::ScalarSpill));
}

#[test]
fn cast_around_checked_array_access_remains_block_local() {
    let mir = lower_text(
        "fn cast(values: u64[]) -> bool { return (bool) values[0]; }\n\
         fn main() -> i64 { return 0; }\n",
    );
    verify_mir(&mir).unwrap();
    let dump = dump_mir(&mir);
    assert_eq!(dump.matches("array-position-check").count(), 1);
    assert_eq!(dump.matches("cast.u64.bool").count(), 1);
    assert_eq!(dump, dump_mir(&mir));
}

fn primitive_cast_hir() -> crate::hir::HirProgram {
    type_check_source(concat!(
        "fn exercise() -> i64 { var value: u8 = (u8) 1u; return 0; }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap()
}

fn checked_primitive_cast_hir(target: HirPrimitiveType) -> crate::hir::HirProgram {
    type_check_source(format!(
        "fn exercise() -> unit {{ var value: {0} = ({0}) 1.5; }}\n\
         fn main() -> i64 {{ return 0; }}\n",
        target.name()
    ))
    .hir
    .unwrap()
}

fn checked_primitive_cast_mir(target: HirPrimitiveType) -> MirProgram {
    lower_hir(&checked_primitive_cast_hir(target))
}

#[test]
fn lowers_and_verifies_all_checked_primitive_cast_diamonds() {
    for &(hir_target, mir_target) in &[
        (HirPrimitiveType::I64, MirIntegerType::I64),
        (HirPrimitiveType::U64, MirIntegerType::U64),
        (HirPrimitiveType::U8, MirIntegerType::U8),
    ] {
        let hir = checked_primitive_cast_hir(hir_target);
        let hir_dump = dump_hir(&hir);
        assert_eq!(hir_dump, dump_hir(&hir));
        assert!(hir_dump.contains(&format!(
            "PrimitiveCast checked_f64_to_integer f64.{} failure=primitive-cast-out-of-range",
            mir_target.name()
        )));

        let mir = lower_hir(&hir);
        verify_mir(&mir).unwrap();
        let definition = mir.definitions.get(FunctionId::new(0)).unwrap();
        let (check, success, failure) = definition
            .body
            .blocks
            .iter()
            .find_map(|block| match block.terminator {
                Some(MirTerminator::PrimitiveCastRangeCheck {
                    check,
                    success_target,
                    failure_target,
                    ..
                }) => Some((check, success_target, failure_target)),
                _ => None,
            })
            .expect("checked cast must contain a range-check terminator");
        assert_eq!(check.relation.target, mir_target);
        assert_eq!(check.relation.source_type(), MirType::F64);
        assert_eq!(check.relation.result_type(), mir_target.operand_type());
        assert!(check.relation.requires_finite());
        assert_eq!(
            check.relation.rounding(),
            MirF64ToIntegerRounding::TowardZero
        );
        assert_eq!(
            check.relation.failure_reason(),
            MirTerminationReason::PrimitiveCastOutOfRange
        );
        assert!(matches!(
            definition.block(success).unwrap().instructions.as_slice(),
            [MirInstruction::Assign(_), MirInstruction::Assign(MirAssignment {
                rvalue: MirRvalue {
                    kind: MirRvalueKind::CheckedF64ToInteger { relation, .. },
                    ..
                },
                ..
            }), MirInstruction::Store(_)] if *relation == check.relation
        ));
        assert!(definition.block(failure).unwrap().instructions.is_empty());
        assert!(matches!(
            definition.block(failure).unwrap().terminator,
            Some(MirTerminator::Terminate {
                reason: MirTerminationReason::PrimitiveCastOutOfRange,
                ..
            })
        ));

        let dump = dump_mir(&mir);
        assert_eq!(dump, dump_mir(&mir));
        assert!(dump.contains(&format!(
            "primitive-cast-range-check f64.{}",
            mir_target.name()
        )));
        assert!(dump.contains(&format!(
            "checked-cast.f64.{} trunc=toward-zero",
            mir_target.name()
        )));
        assert!(dump.contains("terminate primitive-cast-out-of-range"));
    }
}

#[test]
fn checked_cast_is_control_affecting_and_spills_an_enclosing_eager_operand() {
    let hir = type_check_source(concat!(
        "fn source() -> f64 { return 2.5; }\n",
        "fn exercise() -> i64 { return 1 + (i64) source(); }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let dump = dump_mir(&mir);
    let check = dump.find("primitive-cast-range-check f64.i64").unwrap();
    let addition = dump.find("add.i64").unwrap();
    assert!(check < addition);
    let definition = mir.definitions.get(FunctionId::new(1)).unwrap();
    assert!(definition.storage.iter().any(|storage| {
        storage.kind == MirStorageKind::ScalarSpill
            && storage.ty == MirType::I64
            && storage.name.starts_with("spill")
    }));
    assert!(dump.find("add.i64").unwrap() < dump.rfind("return").unwrap());
}

#[test]
fn checked_cast_stays_on_the_selected_path_of_a_loop_condition() {
    let hir = type_check_source(concat!(
        "fn exercise() -> i64 { while (false && (bool) (i64) 1.5) {} return 0; }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let definition = mir.definitions.get(FunctionId::new(0)).unwrap();
    let logical = &definition.body.logical_expressions[0];
    let check_block = definition
        .body
        .blocks
        .iter()
        .find(|block| {
            matches!(
                block.terminator,
                Some(MirTerminator::PrimitiveCastRangeCheck { .. })
            )
        })
        .unwrap();
    assert_eq!(check_block.id, logical.right_entry);

    let dump = dump_mir(&mir);
    assert!(dump.contains("and condition"));
    assert!(dump.contains("primitive-cast-range-check f64.i64"));
}

#[test]
fn effectful_checked_cast_operands_are_evaluated_once_and_cleanup_after_the_join() {
    let hir = type_check_source(concat!(
        "class Trace {\n",
        "  value: f64;\n",
        "  init(value: f64) { self.value = value; }\n",
        "  fn read() -> f64 { return self.value; }\n",
        "  destroy {}\n",
        "}\n",
        "fn make(value: f64) -> shared Trace { return new Trace(value); }\n",
        "fn exercise() -> unit { var value: u64 = (u64) make(7.5)->read(); }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let definition = mir.definitions.get(FunctionId::new(1)).unwrap();
    assert_eq!(
        definition
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| matches!(instruction, MirInstruction::Call(_)))
            .count(),
        2,
        "one producer call and one method call must each execute once"
    );
    let (join, failure) = definition
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::PrimitiveCastRangeCheck {
                success_target,
                failure_target,
                ..
            }) => {
                let success = definition.block(success_target).unwrap();
                let Some(MirTerminator::Goto { target: join, .. }) = success.terminator else {
                    unreachable!()
                };
                Some((join, failure_target))
            }
            _ => None,
        })
        .unwrap();
    assert!(definition.block(failure).unwrap().instructions.is_empty());
    assert!(definition.body.blocks.iter().any(|block| {
        block.id.index() >= join.index()
            && block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, MirInstruction::SharedRelease(_)))
    }));
    let dump = dump_mir(&mir);
    assert!(dump.contains("primitive-cast-range-check f64.u64"));
    assert!(dump.contains("shared-release"));
    crate::backend::emit_assembly(crate::backend::Target::X86_64SysV, &mir)
        .expect("verified effectful checked-cast MIR must reach target selection");
}

#[test]
fn checked_cast_secures_a_checked_operand_before_its_own_range_check() {
    let hir = type_check_source(concat!(
        "fn exercise(values: f64[]) -> unit {\n",
        "  var value: u8 = (u8) values[0];\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let dump = dump_mir(&mir);
    let operand_check = dump.find("array-position-check").unwrap();
    let cast_check = dump.find("primitive-cast-range-check f64.u8").unwrap();
    assert!(operand_check < cast_check);
    crate::backend::emit_assembly(crate::backend::Target::X86_64SysV, &mir)
        .expect("nested checked operations must reach target selection in verified order");
}

#[test]
fn verified_checked_cast_mir_reaches_backend_selection() {
    let mir = checked_primitive_cast_mir(HirPrimitiveType::U8);
    verify_mir(&mir).unwrap();
    let assembly = crate::backend::emit_assembly(crate::backend::Target::X86_64SysV, &mir)
        .expect("verified checked-cast MIR must be executable");
    assert!(assembly.contains("cvttsd2si rax, xmm14"));
    assert!(assembly.contains("call ska_rt_panic"));
}

fn primitive_cast_mir(source: MirPrimitiveType, target: MirPrimitiveType) -> MirProgram {
    let mut mir = lower_text(concat!(
        "fn exercise() -> i64 { var value: u8 = (u8) 1u; return 0; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let function = mir
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let cast_index = function.body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                MirInstruction::Assign(MirAssignment {
                    rvalue: MirRvalue {
                        kind: MirRvalueKind::PrimitiveCast { .. },
                        ..
                    },
                    ..
                })
            )
        })
        .unwrap();
    let (operand, result) = match &mut function.body.blocks[0].instructions[cast_index] {
        MirInstruction::Assign(assignment) => {
            let MirRvalueKind::PrimitiveCast { operation, operand } = &mut assignment.rvalue.kind
            else {
                unreachable!()
            };
            *operation = MirPrimitiveCast::new(source, target);
            assignment.rvalue.ty = target.value_type();
            (*operand, assignment.result)
        }
        _ => unreachable!(),
    };
    let source_assignment = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) if assignment.result == operand => Some(assignment),
            _ => None,
        })
        .unwrap();
    source_assignment.rvalue.kind = mir_literal(source);
    source_assignment.rvalue.ty = source.value_type();
    function.values[operand.index()].ty = source.value_type();
    function.values[result.index()].ty = target.value_type();
    function.storage[0].ty = target.value_type();
    mir
}

fn primitive_cast(program: &MirProgram) -> (MirPrimitiveCast, ValueId, MirType) {
    program
        .definitions
        .get(FunctionId::new(0))
        .unwrap()
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(MirAssignment {
                rvalue:
                    MirRvalue {
                        kind: MirRvalueKind::PrimitiveCast { operation, operand },
                        ty,
                    },
                ..
            }) => Some((*operation, *operand, *ty)),
            _ => None,
        })
        .expect("fixture must contain a primitive-cast rvalue")
}

fn hir_literal(ty: HirPrimitiveType) -> HirExpressionKind {
    match ty {
        HirPrimitiveType::I64 => HirExpressionKind::I64(-1),
        HirPrimitiveType::U64 => HirExpressionKind::U64(u64::MAX),
        HirPrimitiveType::U8 => HirExpressionKind::U8(u8::MAX),
        HirPrimitiveType::F64 => HirExpressionKind::F64Bits(1.5f64.to_bits()),
        HirPrimitiveType::Bool => HirExpressionKind::Boolean(true),
    }
}

fn mir_literal(ty: MirPrimitiveType) -> MirRvalueKind {
    match ty {
        MirPrimitiveType::I64 => MirRvalueKind::ConstantI64(-1),
        MirPrimitiveType::U64 => MirRvalueKind::ConstantU64(u64::MAX),
        MirPrimitiveType::U8 => MirRvalueKind::ConstantU8(u8::MAX),
        MirPrimitiveType::F64 => MirRvalueKind::ConstantF64Bits(1.5f64.to_bits()),
        MirPrimitiveType::Bool => MirRvalueKind::ConstantBool(true),
    }
}

#[test]
fn semantic_classes_cover_the_frozen_matrix() {
    for &(source, target, expected) in &[
        (
            MirPrimitiveType::I64,
            MirPrimitiveType::I64,
            MirPrimitiveCastKind::Identity,
        ),
        (
            MirPrimitiveType::U64,
            MirPrimitiveType::U8,
            MirPrimitiveCastKind::IntegerBits,
        ),
        (
            MirPrimitiveType::F64,
            MirPrimitiveType::Bool,
            MirPrimitiveCastKind::ToBool,
        ),
        (
            MirPrimitiveType::U8,
            MirPrimitiveType::F64,
            MirPrimitiveCastKind::ToF64,
        ),
        (
            MirPrimitiveType::Bool,
            MirPrimitiveType::U64,
            MirPrimitiveCastKind::FromBool,
        ),
        (
            MirPrimitiveType::F64,
            MirPrimitiveType::I64,
            MirPrimitiveCastKind::CheckedF64ToInteger,
        ),
    ] {
        assert_eq!(MirPrimitiveCast::new(source, target).kind(), expected);
    }

    for (source, target) in [
        (MirPrimitiveType::F64, MirPrimitiveType::U64),
        (MirPrimitiveType::U64, MirPrimitiveType::F64),
    ] {
        let operation = MirPrimitiveCast::bit_reinterpretation(source, target);
        assert_eq!(operation.kind(), MirPrimitiveCastKind::BitReinterpretation);
        assert!(operation.is_semantically_consistent());
        assert!(!operation.may_terminate());
    }
}

#[test]
fn checked_cast_is_not_an_ordinary_mir_rvalue() {
    let mir = primitive_cast_mir(MirPrimitiveType::F64, MirPrimitiveType::I64);
    assert_eq!(
        verify_mir(&mir)
            .unwrap_err()
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>(),
        ["checked primitive cast requires explicit control flow"]
    );
}

fn checked_cast_block_indices(program: &MirProgram) -> (usize, usize, usize, usize) {
    let definition = program.definitions.get(FunctionId::new(0)).unwrap();
    checked_cast_block_indices_from_definition(definition)
}

fn checked_cast_verifier_errors(program: &MirProgram) -> String {
    let errors = verify_mir(program).unwrap_err().to_string();
    assert_eq!(errors, verify_mir(program).unwrap_err().to_string());
    errors
}

#[test]
fn verifier_rejects_checked_cast_type_relation_and_carrier_corruption() {
    let mut wrong_source = checked_primitive_cast_mir(HirPrimitiveType::I64);
    let definition = wrong_source
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let (check, _, _, _) = checked_cast_block_indices_from_definition(definition);
    let source = match definition.body.blocks[check].terminator.as_ref().unwrap() {
        MirTerminator::PrimitiveCastRangeCheck { check, .. } => check.source,
        _ => unreachable!(),
    };
    definition.storage[source.index()].ty = MirType::I64;
    assert!(checked_cast_verifier_errors(&wrong_source)
        .contains("primitive cast source carrier must be an exact `f64` scalar spill"));

    let mut wrong_result = checked_primitive_cast_mir(HirPrimitiveType::I64);
    let definition = wrong_result
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let (check, _, _, _) = checked_cast_block_indices_from_definition(definition);
    let result = match definition.body.blocks[check].terminator.as_ref().unwrap() {
        MirTerminator::PrimitiveCastRangeCheck { check, .. } => check.result,
        _ => unreachable!(),
    };
    definition.storage[result.index()].ty = MirType::F64;
    assert!(checked_cast_verifier_errors(&wrong_result)
        .contains("primitive cast result carrier must be an exact `i64` scalar spill"));

    let mut mismatched_relation = checked_primitive_cast_mir(HirPrimitiveType::I64);
    let (check, success, _, _) = checked_cast_block_indices(&mismatched_relation);
    let definition = mismatched_relation
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirTerminator::PrimitiveCastRangeCheck { check, .. } =
        definition.body.blocks[check].terminator.as_mut().unwrap()
    else {
        unreachable!()
    };
    check.relation.target = MirIntegerType::U64;
    let errors = checked_cast_verifier_errors(&mismatched_relation);
    assert!(errors.contains("primitive cast result carrier must be an exact `u64` scalar spill"));
    assert!(errors.contains("primitive cast success edge must load the secured source"));
    assert!(errors.contains("checked floating-to-integer conversion is not protected"));
    assert_eq!(success, 1);
}

#[test]
fn verifier_rejects_checked_cast_success_failure_and_dominance_corruption() {
    let mut wrong_conversion = checked_primitive_cast_mir(HirPrimitiveType::I64);
    let (_, success, _, _) = checked_cast_block_indices(&wrong_conversion);
    let definition = wrong_conversion
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let conversion = definition.body.blocks[success]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(
                    assignment.rvalue.kind,
                    MirRvalueKind::CheckedF64ToInteger { .. }
                ) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .unwrap();
    let MirRvalueKind::CheckedF64ToInteger { relation, .. } = &mut conversion.rvalue.kind else {
        unreachable!()
    };
    relation.target = MirIntegerType::U64;
    let errors = checked_cast_verifier_errors(&wrong_conversion);
    assert!(errors.contains("primitive cast success edge must load the secured source"));
    assert!(errors.contains("checked primitive cast result type mismatch"));
    assert!(errors.contains("checked floating-to-integer conversion is not protected"));

    let mut wrong_operand = checked_primitive_cast_mir(HirPrimitiveType::I64);
    let (_, success, _, _) = checked_cast_block_indices(&wrong_operand);
    let definition = wrong_operand
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let conversion = definition.body.blocks[success]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(
                    assignment.rvalue.kind,
                    MirRvalueKind::CheckedF64ToInteger { .. }
                ) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .unwrap();
    let MirRvalueKind::CheckedF64ToInteger { operand, .. } = &mut conversion.rvalue.kind else {
        unreachable!()
    };
    *operand = ValueId::new(definition.function, 0);
    assert!(checked_cast_verifier_errors(&wrong_operand)
        .contains("primitive cast success edge must load the secured source"));

    let mut wrong_join = checked_primitive_cast_mir(HirPrimitiveType::I64);
    let (check, _, _, join) = checked_cast_block_indices(&wrong_join);
    let definition = wrong_join
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let source = match definition.body.blocks[check].terminator.as_ref().unwrap() {
        MirTerminator::PrimitiveCastRangeCheck { check, .. } => check.source,
        _ => unreachable!(),
    };
    let MirInstruction::Assign(load) = &mut definition.body.blocks[join].instructions[0] else {
        unreachable!()
    };
    load.rvalue.kind = MirRvalueKind::Load(MirPlace::base(source));
    assert!(checked_cast_verifier_errors(&wrong_join)
        .contains("primitive cast join must begin by loading the checked result carrier"));

    let mut wrong_failure = checked_primitive_cast_mir(HirPrimitiveType::I64);
    let (_, _, failure, _) = checked_cast_block_indices(&wrong_failure);
    let definition = wrong_failure
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let span = definition.body.blocks[failure].span;
    definition.body.blocks[failure].terminator = Some(MirTerminator::Terminate {
        reason: MirTerminationReason::OptionalAccessFailure,
        span,
    });
    assert!(checked_cast_verifier_errors(&wrong_failure)
        .contains("primitive cast failure edge must directly terminate"));

    let mut alternate_success_predecessor = checked_primitive_cast_mir(HirPrimitiveType::I64);
    let (_, success, failure, _) = checked_cast_block_indices(&alternate_success_predecessor);
    let definition = alternate_success_predecessor
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let span = definition.body.blocks[failure].span;
    definition.body.blocks[failure].terminator = Some(MirTerminator::Goto {
        target: BlockId::new(definition.function, success),
        span,
    });
    let errors = checked_cast_verifier_errors(&alternate_success_predecessor);
    assert!(errors.contains("primitive cast success block must be dominated"));
    assert!(errors.contains("primitive cast failure edge must directly terminate"));

    let mut unprotected = checked_primitive_cast_mir(HirPrimitiveType::I64);
    let (check, _, _, join) = checked_cast_block_indices(&unprotected);
    let definition = unprotected
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirTerminator::PrimitiveCastRangeCheck { success_target, .. } =
        definition.body.blocks[check].terminator.as_mut().unwrap()
    else {
        unreachable!()
    };
    *success_target = BlockId::new(definition.function, join);
    let errors = checked_cast_verifier_errors(&unprotected);
    assert!(errors.contains("primitive cast success edge must load the secured source"));
    assert!(errors.contains("checked floating-to-integer conversion is not protected"));
}

fn checked_cast_block_indices_from_definition(
    definition: &MirFunctionDefinition,
) -> (usize, usize, usize, usize) {
    definition
        .body
        .blocks
        .iter()
        .enumerate()
        .find_map(|(check_index, block)| match block.terminator {
            Some(MirTerminator::PrimitiveCastRangeCheck {
                success_target,
                failure_target,
                ..
            }) => {
                let Some(MirTerminator::Goto { target: join, .. }) = definition
                    .block(success_target)
                    .and_then(|block| block.terminator.as_ref())
                else {
                    return None;
                };
                Some((
                    check_index,
                    success_target.index(),
                    failure_target.index(),
                    join.index(),
                ))
            }
            _ => None,
        })
        .unwrap()
}
