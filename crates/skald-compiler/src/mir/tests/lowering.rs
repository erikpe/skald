use super::*;

#[test]
fn lowers_storage_values_arithmetic_and_return_explicitly() {
    let mir = lower_text("fn main() -> i64 { var result: i64 = 1; return result + 2; }");
    assert!(super::verify_mir(&mir).is_ok());
    let function = mir.definitions.get(mir.entry_function).unwrap();

    assert_eq!(function.storage.len(), 1);
    assert_eq!(function.storage[0].kind, MirStorageKind::Local);
    assert_eq!(function.values.len(), 4);
    let block = function.block(function.body.entry).unwrap();
    assert_eq!(block.instructions.len(), 7);
    assert!(matches!(
        block.instructions[0],
        MirInstruction::StorageLive(MirStorageLive { .. })
    ));
    assert!(matches!(
        block.instructions[1],
        MirInstruction::Assign(MirAssignment {
            rvalue: MirRvalue {
                kind: MirRvalueKind::ConstantI64(1),
                ..
            },
            ..
        })
    ));
    assert!(matches!(block.instructions[2], MirInstruction::Store(_)));
    assert!(matches!(
        block.instructions[5],
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
        block.instructions[6],
        MirInstruction::StorageDead(MirStorageDead { .. })
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
fn lowers_boolean_constants_storage_calls_and_returns_as_boolean_mir() {
    let mir = lower_text(concat!(
        "extern fn external_flag(value: bool) -> bool;\n",
        "fn identity(value: bool) -> bool { var result: bool = value; return result; }\n",
        "fn main() -> i64 { var flag: bool = identity(true); var external: bool = external_flag(flag); return 0; }\n",
    ));

    assert!(verify_mir(&mir).is_ok());
    let identity_declaration = mir.declarations.get(FunctionId::new(1)).unwrap();
    assert_eq!(
        identity_declaration.parameters,
        [MirParameter::value(MirType::Bool)]
    );
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
    assert!(dump.contains("call f0(value(f2:v0))"));
    assert!(dump.contains("return @"));
}
