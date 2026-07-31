use super::*;
use crate::hir::{
    HirExpressionKind, HirLocalInitializer, HirPrimitiveCast, HirPrimitiveType, HirStatement,
};

const PRIMITIVE_TYPES: &[(HirPrimitiveType, MirPrimitiveType, &str)] = &[
    (HirPrimitiveType::I64, MirPrimitiveType::I64, "i64"),
    (HirPrimitiveType::U64, MirPrimitiveType::U64, "u64"),
    (HirPrimitiveType::U8, MirPrimitiveType::U8, "u8"),
    (HirPrimitiveType::F64, MirPrimitiveType::F64, "f64"),
    (HirPrimitiveType::Bool, MirPrimitiveType::Bool, "bool"),
];

#[test]
fn lowers_and_verifies_the_complete_source_enabled_pure_matrix() {
    let mut implemented_pairs = 0;
    for &(_, source_type, source_name) in PRIMITIVE_TYPES {
        for &(_, target_type, target_name) in PRIMITIVE_TYPES {
            if MirPrimitiveCast::new(source_type, target_type).may_terminate() {
                continue;
            }
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
            let cast = primitive_cast(&mir);

            assert_eq!(cast.0, MirPrimitiveCast::new(source_type, target_type));
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

            let dump = dump_mir(&mir);
            assert_eq!(dump, dump_mir(&mir));
            assert!(dump.contains(&format!("cast.{source_name}.{target_name}")));
        }
    }
    assert_eq!(implemented_pairs, 22);
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
