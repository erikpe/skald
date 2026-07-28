use super::*;

const INTEGER_TYPES: &[(MirIntegerType, &str, &str)] = &[
    (MirIntegerType::I64, "i64", "-1"),
    (MirIntegerType::U64, "u64", "18446744073709551615u"),
    (MirIntegerType::U8, "u8", "255u8"),
];

#[test]
fn lowers_and_verifies_the_complete_integer_cast_matrix() {
    for &(source_type, source_name, operand) in INTEGER_TYPES {
        for &(target_type, target_name, _) in INTEGER_TYPES {
            let source = format!(
                "fn cast() -> {target_name} {{ return ({target_name}) {operand}; }} \
                 fn main() -> i64 {{ return 0; }}"
            );
            let mir = lower_text(&source);
            verify_mir(&mir).unwrap();
            let cast = mir
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
                                kind: MirRvalueKind::IntegerCast { operation, operand },
                                ty,
                            },
                        ..
                    }) => Some((*operation, *operand, *ty)),
                    _ => None,
                })
                .expect("cast source must lower to an integer-cast rvalue");

            assert_eq!(
                cast.0,
                MirIntegerCast {
                    source: source_type,
                    target: target_type,
                }
            );
            assert_eq!(cast.0.source_type(), source_type.operand_type());
            assert_eq!(cast.0.result_type(), target_type.operand_type());
            assert_eq!(cast.2, target_type.operand_type());
            assert!(mir
                .definitions
                .get(FunctionId::new(0))
                .unwrap()
                .values
                .iter()
                .any(|value| value.id == cast.1 && value.ty == source_type.operand_type()));

            let dump = dump_mir(&mir);
            assert_eq!(dump, dump_mir(&mir));
            assert!(dump.contains(&format!("cast.{source_name}.{target_name}")));
        }
    }
}

#[test]
fn cast_operand_is_lowered_once_and_cast_adds_no_control_effect() {
    let mir = lower_text(
        "fn source() -> u64 { return 7u; }\n\
         fn cast() -> u8 { return (u8) source(); }\n\
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
                        kind: MirRvalueKind::IntegerCast { .. },
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
        "fn cast(values: u64[]) -> u8 { return (u8) values[0]; }\n\
         fn main() -> i64 { return 0; }\n",
    );
    verify_mir(&mir).unwrap();
    let dump = dump_mir(&mir);
    assert_eq!(dump.matches("array-position-check").count(), 1);
    assert_eq!(dump.matches("cast.u64.u8").count(), 1);
    assert_eq!(dump, dump_mir(&mir));
}
