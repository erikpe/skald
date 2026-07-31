use super::*;

const CAST_FUNCTION: &str = ".Lska.fn.main.cast.f0";

#[test]
fn legality_accepts_all_ten_identity_and_boolean_boundary_cells() {
    for (source, target) in executable_cases() {
        emit_assembly(Target::X86_64SysV, &primitive_cast_program(source, target))
            .unwrap_or_else(|error| panic!("{source:?} -> {target:?}: {error}"));
    }
}

#[test]
fn selector_uses_explicit_reusable_scalar_operations() {
    let f64_identity = cast_function(
        PrimitiveValue::F64Bits(0x7ff8_1234_5678_9abc),
        MirPrimitiveType::F64,
    );
    assert!(f64_identity.contains("movsd xmm14, qword ptr [rbp"));
    assert!(f64_identity.contains("movsd qword ptr [rbp"));

    let bool_identity = cast_function(PrimitiveValue::Bool(true), MirPrimitiveType::Bool);
    assert!(!bool_identity.contains("cmp "));
    assert!(!bool_identity.contains("set"));

    for source in [
        PrimitiveValue::I64(-1),
        PrimitiveValue::U64(u64::MAX),
        PrimitiveValue::U8(u8::MAX),
    ] {
        let function = cast_function(source, MirPrimitiveType::Bool);
        assert!(function.contains("test rax, rax"));
        assert!(function.contains("setne al"));
        assert!(function.contains("movzx rax, al"));
    }

    let f64_to_bool = cast_function(
        PrimitiveValue::F64Bits(f64::NAN.to_bits()),
        MirPrimitiveType::Bool,
    );
    for instruction in [
        "xorpd xmm15, xmm15",
        "ucomisd xmm14, xmm15",
        "setne al",
        "setp cl",
        "or al, cl",
        "movzx rax, al",
    ] {
        assert!(f64_to_bool.contains(instruction), "missing `{instruction}`");
    }

    for target in [
        MirPrimitiveType::I64,
        MirPrimitiveType::U64,
        MirPrimitiveType::U8,
    ] {
        let function = cast_function(PrimitiveValue::Bool(true), target);
        assert!(!function.contains("cmp "));
        assert!(!function.contains("set"));
        assert!(!function.contains("cvtsi2sd"));
    }

    let bool_to_f64 = cast_function(PrimitiveValue::Bool(true), MirPrimitiveType::F64);
    assert!(bool_to_f64.contains("cvtsi2sd xmm14, rax"));

    for (source, target) in executable_cases() {
        let function = cast_function(source, target);
        assert!(!function.contains("call "), "{source:?} -> {target:?}");
        assert!(!function.contains("ud2"), "{source:?} -> {target:?}");
    }
}

#[test]
fn emitted_identity_and_boolean_boundary_casts_are_deterministic_and_assemble() {
    for (source, target) in executable_cases() {
        let program = primitive_cast_program(source, target);
        let output = emit_assembly(Target::X86_64SysV, &program).unwrap();
        assert_eq!(output, emit_assembly(Target::X86_64SysV, &program).unwrap());
        assert_system_assembler_accepts(&output);
        assert!(!output.contains("ska_rt_primitive_cast"));
    }
}

#[test]
fn f64_identity_preserves_every_tested_binary64_bit() {
    for bits in [
        0,
        1_u64 << 63,
        1,
        (1_u64 << 63) | 1,
        1.5_f64.to_bits(),
        (-1.5_f64).to_bits(),
        f64::INFINITY.to_bits(),
        f64::NEG_INFINITY.to_bits(),
        0x7ff8_1234_5678_9abc,
        0xfff0_0000_0000_0001,
    ] {
        assert_native_cast(PrimitiveValue::F64Bits(bits), MirPrimitiveType::F64, bits);
    }
}

#[test]
fn numeric_to_bool_uses_zero_truthiness_and_treats_every_nan_as_true() {
    for (source, expected) in [
        (PrimitiveValue::I64(0), false),
        (PrimitiveValue::I64(1), true),
        (PrimitiveValue::I64(-1), true),
        (PrimitiveValue::I64(i64::MIN), true),
        (PrimitiveValue::I64(i64::MAX), true),
        (PrimitiveValue::U64(0), false),
        (PrimitiveValue::U64(1), true),
        (PrimitiveValue::U64(1_u64 << 63), true),
        (PrimitiveValue::U64(u64::MAX), true),
        (PrimitiveValue::U8(0), false),
        (PrimitiveValue::U8(1), true),
        (PrimitiveValue::U8(u8::MAX), true),
    ] {
        assert_native_cast(source, MirPrimitiveType::Bool, u64::from(expected));
    }

    for (bits, expected) in [
        (0, false),
        (1_u64 << 63, false),
        (1, true),
        ((1_u64 << 63) | 1, true),
        (1.5_f64.to_bits(), true),
        ((-1.5_f64).to_bits(), true),
        (f64::INFINITY.to_bits(), true),
        (f64::NEG_INFINITY.to_bits(), true),
        (0x7ff8_1234_5678_9abc, true),
        (0x7ff0_0000_0000_0001, true),
        (0xfff8_0000_0000_0042, true),
    ] {
        assert_native_cast(
            PrimitiveValue::F64Bits(bits),
            MirPrimitiveType::Bool,
            u64::from(expected),
        );
    }
}

#[test]
fn bool_identity_and_numeric_results_are_canonical_zero_or_one() {
    for value in [false, true] {
        let expected = u64::from(value);
        assert_native_cast(
            PrimitiveValue::Bool(value),
            MirPrimitiveType::Bool,
            expected,
        );
        for target in [
            MirPrimitiveType::I64,
            MirPrimitiveType::U64,
            MirPrimitiveType::U8,
        ] {
            assert_native_cast(PrimitiveValue::Bool(value), target, expected);
        }
        assert_native_cast(
            PrimitiveValue::Bool(value),
            MirPrimitiveType::F64,
            (expected as f64).to_bits(),
        );
    }
}

#[test]
fn cast_results_feed_boolean_control_flow_and_exact_numeric_comparisons() {
    let mut to_bool = lower_text("fn main() -> i64 { if (1u == 1u) { return 0; } return 1; }\n");
    let function = to_bool
        .definitions
        .get_mut_for_test(to_bool.entry_function)
        .unwrap();
    let assignment = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(
                    assignment.rvalue.kind,
                    MirRvalueKind::PrimitiveComparison { .. }
                ) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .unwrap();
    let left = match &assignment.rvalue.kind {
        MirRvalueKind::PrimitiveComparison { left, .. } => *left,
        _ => unreachable!(),
    };
    assignment.rvalue.kind = MirRvalueKind::PrimitiveCast {
        operation: MirPrimitiveCast::new(MirPrimitiveType::U64, MirPrimitiveType::Bool),
        operand: left,
    };
    verify_mir(&to_bool).unwrap();
    assert_eq!(
        run_native_assembly(
            &emit_assembly(Target::X86_64SysV, &to_bool).expect("u64-to-bool must lower")
        )
        .code(),
        Some(0)
    );

    let mut from_bool =
        lower_text("fn main() -> i64 { if ((u8) 1u == 1u8) { return 0; } return 1; }\n");
    let function = from_bool
        .definitions
        .get_mut_for_test(from_bool.entry_function)
        .unwrap();
    let (operand, operation) = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) => match &mut assignment.rvalue.kind {
                MirRvalueKind::PrimitiveCast { operation, operand } => Some((*operand, operation)),
                _ => None,
            },
            _ => None,
        })
        .unwrap();
    *operation = MirPrimitiveCast::new(MirPrimitiveType::Bool, MirPrimitiveType::U8);
    let source = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) if assignment.result == operand => Some(assignment),
            _ => None,
        })
        .unwrap();
    source.rvalue.kind = MirRvalueKind::ConstantBool(true);
    source.rvalue.ty = MirType::Bool;
    function.values[operand.index()].ty = MirType::Bool;
    verify_mir(&from_bool).unwrap();
    assert_eq!(
        run_native_assembly(
            &emit_assembly(Target::X86_64SysV, &from_bool).expect("bool-to-u8 must lower")
        )
        .code(),
        Some(0)
    );
}

fn executable_cases() -> [(PrimitiveValue, MirPrimitiveType); 10] {
    [
        (PrimitiveValue::F64Bits(0), MirPrimitiveType::F64),
        (PrimitiveValue::Bool(false), MirPrimitiveType::Bool),
        (PrimitiveValue::I64(0), MirPrimitiveType::Bool),
        (PrimitiveValue::U64(0), MirPrimitiveType::Bool),
        (PrimitiveValue::U8(0), MirPrimitiveType::Bool),
        (PrimitiveValue::F64Bits(0), MirPrimitiveType::Bool),
        (PrimitiveValue::Bool(false), MirPrimitiveType::I64),
        (PrimitiveValue::Bool(false), MirPrimitiveType::U64),
        (PrimitiveValue::Bool(false), MirPrimitiveType::U8),
        (PrimitiveValue::Bool(false), MirPrimitiveType::F64),
    ]
}

fn cast_function(source: PrimitiveValue, target: MirPrimitiveType) -> String {
    let output =
        emit_assembly(Target::X86_64SysV, &primitive_cast_program(source, target)).unwrap();
    function_assembly(&output, CAST_FUNCTION).to_owned()
}

fn assert_native_cast(source: PrimitiveValue, target: MirPrimitiveType, expected_bits: u64) {
    let program = primitive_cast_program(source, target);
    let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    output.push_str(&validator(target, expected_bits));
    let result = run_native_assembly_output(&output);
    assert!(
        result.status.success(),
        "{source:?} -> {target:?} expected 0x{expected_bits:016x}, status {:?}, stderr {}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

fn validator(target: MirPrimitiveType, expected_bits: u64) -> String {
    let load_actual = if target == MirPrimitiveType::F64 {
        "    movq rdi, xmm0\n"
    } else {
        ""
    };
    format!(
        concat!(
            "\n.text\n",
            ".globl validate_primitive_cast\n",
            ".type validate_primitive_cast, @function\n",
            "validate_primitive_cast:\n",
            "{load_actual}",
            "    mov rcx, 0x{expected_bits:016x}\n",
            "    cmp rdi, rcx\n",
            "    setne al\n",
            "    movzx rax, al\n",
            "    ret\n",
            ".size validate_primitive_cast, .-validate_primitive_cast\n",
        ),
        load_actual = load_actual,
        expected_bits = expected_bits,
    )
}
