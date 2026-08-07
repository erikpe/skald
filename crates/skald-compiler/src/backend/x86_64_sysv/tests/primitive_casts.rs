use super::primitive_cast_oracle::{
    checked_cast_samples, expected_checked_cast, expected_pure_cast, pure_cast_samples,
    signed_integer_to_f64_bits, unsigned_integer_to_f64_bits,
};
use super::*;
use crate::test_support::TemporaryFile;
use std::{
    io::Write,
    process::{Command, Stdio},
};

const CAST_FUNCTION: &str = ".Lska.fn.main.cast.f0";
const U8_TO_F64_SAMPLES: [u8; 31] = [
    0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 21, 31, 32, 33, 42, 63, 64, 65, 85, 106, 127, 128, 129,
    149, 170, 213, 234, 254, 255,
];

#[test]
fn legality_accepts_all_ten_identity_and_boolean_boundary_cells() {
    for (source, target) in executable_cases() {
        emit_assembly(Target::X86_64SysV, &primitive_cast_program(source, target))
            .unwrap_or_else(|error| panic!("{source:?} -> {target:?}: {error}"));
    }
}

#[test]
fn legality_accepts_all_three_integer_to_f64_cells() {
    for source in [
        PrimitiveValue::I64(-1),
        PrimitiveValue::U64(u64::MAX),
        PrimitiveValue::U8(u8::MAX),
    ] {
        emit_assembly(
            Target::X86_64SysV,
            &primitive_cast_program(source, MirPrimitiveType::F64),
        )
        .unwrap_or_else(|error| panic!("{source:?} -> f64: {error}"));
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
fn selector_distinguishes_signed_byte_and_full_domain_unsigned_conversion() {
    for source in [PrimitiveValue::I64(i64::MIN), PrimitiveValue::U8(u8::MAX)] {
        let function = cast_function(source, MirPrimitiveType::F64);
        assert!(function.contains("cvtsi2sd xmm14, rax"));
        assert!(!function.contains("jns "));
        assert!(!function.contains("shr "));
        assert!(!function.contains("addsd "));
        assert!(!function.contains("call "));
    }

    let function = cast_function(PrimitiveValue::U64(u64::MAX), MirPrimitiveType::F64);
    for instruction in [
        "test rax, rax",
        "jns .Lska.fn.main.cast.f0.primitive_cast_0_0_u64_signed_domain",
        "shr rax, cl",
        "and rdx, rcx",
        "or rax, rdx",
        "cvtsi2sd xmm14, rax",
        "addsd xmm14, xmm14",
        "jmp .Lska.fn.main.cast.f0.primitive_cast_0_1_u64_result_ready",
    ] {
        assert!(function.contains(instruction), "missing `{instruction}`");
    }
    assert!(!function.contains("call "));
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
fn emitted_integer_to_f64_casts_are_deterministic_and_assemble_without_helpers() {
    for source in [
        PrimitiveValue::I64(i64::MIN),
        PrimitiveValue::U64(u64::MAX),
        PrimitiveValue::U8(u8::MAX),
    ] {
        let program = primitive_cast_program(source, MirPrimitiveType::F64);
        let output = emit_assembly(Target::X86_64SysV, &program).unwrap();
        assert_eq!(output, emit_assembly(Target::X86_64SysV, &program).unwrap());
        assert_system_assembler_accepts(&output);
        assert!(!output.contains("ska_rt_primitive_cast"));
        assert!(!function_assembly(&output, CAST_FUNCTION).contains("call "));
    }
}

#[test]
fn checked_float_to_integer_selection_is_guarded_deterministic_and_inline() {
    for target in [MirIntegerType::I64, MirIntegerType::U64, MirIntegerType::U8] {
        let program = fixture_checked_primitive_cast_program(0.5_f64.to_bits(), target, 0);
        let assembly = emit_assembly(Target::X86_64SysV, &program).unwrap();
        assert_eq!(
            assembly,
            emit_assembly(Target::X86_64SysV, &program).unwrap()
        );
        assert_system_assembler_accepts(&assembly);
        assert!(!assembly.contains("ska_rt_primitive_cast"));

        let function = function_assembly(&assembly, ".Lska.fn.main.main.f0");
        let check_block = cast_block_assembly(function, 0);
        let success_block = cast_block_assembly(function, 1);
        let failure_block = cast_block_assembly(function, 2);
        let check = function.find("ucomisd xmm14, xmm14").unwrap();
        let unordered_failure = function.find("jp .Lska.fn.main.main.f0.block_2").unwrap();
        let upper_failure = function.find("jae .Lska.fn.main.main.f0.block_2").unwrap();
        let conversion = function.find("cvttsd2si rax, xmm14").unwrap();
        assert!(check < unordered_failure);
        assert!(unordered_failure < upper_failure);
        assert!(upper_failure < conversion);
        assert!(function[conversion..].contains("mov qword ptr [rbp"));
        assert!(!check_block.contains("cvttsd2si"));
        assert!(success_block.contains("cvttsd2si rax, xmm14"));
        assert!(success_block.contains("mov qword ptr [rbp"));
        assert!(failure_block.contains("call ska_rt_panic"));
        assert!(!failure_block.contains("cvttsd2si"));
        assert!(!failure_block.contains("jmp "));
        assert_eq!(
            function.matches("cvttsd2si rax, xmm14").count(),
            if target == MirIntegerType::U64 { 2 } else { 1 }
        );
    }
}

#[test]
fn checked_float_to_integer_successes_cover_every_target_boundary() {
    for (bits, target, expected) in [
        (
            (-9_223_372_036_854_775_808.0_f64).to_bits(),
            MirIntegerType::I64,
            i64::MIN as u64,
        ),
        (
            0xc3df_ffff_ffff_ffff,
            MirIntegerType::I64,
            (i64::MIN + 1024) as u64,
        ),
        (
            (-12_345.75_f64).to_bits(),
            MirIntegerType::I64,
            (-12_345_i64) as u64,
        ),
        ((-1.0_f64).to_bits(), MirIntegerType::I64, (-1_i64) as u64),
        ((-0.5_f64).to_bits(), MirIntegerType::I64, 0),
        (1, MirIntegerType::I64, 0),
        ((-0.0_f64).to_bits(), MirIntegerType::I64, 0),
        (0.0_f64.to_bits(), MirIntegerType::I64, 0),
        (12_345.75_f64.to_bits(), MirIntegerType::I64, 12_345),
        (
            0x43df_ffff_ffff_ffff,
            MirIntegerType::I64,
            (i64::MAX - 1023) as u64,
        ),
        ((-0.5_f64).to_bits(), MirIntegerType::U64, 0),
        (0xbfef_ffff_ffff_ffff, MirIntegerType::U64, 0),
        (0x8000_0000_0000_0001, MirIntegerType::U64, 0),
        ((-0.0_f64).to_bits(), MirIntegerType::U64, 0),
        (0.0_f64.to_bits(), MirIntegerType::U64, 0),
        (0x43e0_0000_0000_0000, MirIntegerType::U64, 1_u64 << 63),
        (
            0x43e0_0000_0000_0001,
            MirIntegerType::U64,
            (1_u64 << 63) + 2048,
        ),
        (0x43ef_ffff_ffff_ffff, MirIntegerType::U64, u64::MAX - 2047),
        ((-0.5_f64).to_bits(), MirIntegerType::U8, 0),
        (0xbfef_ffff_ffff_ffff, MirIntegerType::U8, 0),
        (0.0_f64.to_bits(), MirIntegerType::U8, 0),
        (255.0_f64.to_bits(), MirIntegerType::U8, 255),
        (255.9_f64.to_bits(), MirIntegerType::U8, 255),
        (0x406f_ffff_ffff_ffff, MirIntegerType::U8, 255),
    ] {
        assert_native_checked_cast(bits, target, expected);
    }
}

#[test]
fn checked_float_to_integer_failures_report_the_exact_frozen_message() {
    for (bits, target) in [
        (0xc3e0_0000_0000_0001, MirIntegerType::I64),
        (0x43e0_0000_0000_0000, MirIntegerType::I64),
        ((-1.0_f64).to_bits(), MirIntegerType::U64),
        (0xbff0_0000_0000_0001, MirIntegerType::U64),
        ((-1.0_f64).to_bits(), MirIntegerType::U8),
        (0xbff0_0000_0000_0001, MirIntegerType::U8),
        (256.0_f64.to_bits(), MirIntegerType::U8),
        (0x43f0_0000_0000_0000, MirIntegerType::U64),
        (f64::INFINITY.to_bits(), MirIntegerType::I64),
        (f64::NEG_INFINITY.to_bits(), MirIntegerType::U64),
        (0x7ff8_1234_5678_9abc, MirIntegerType::U8),
        (0x7ff0_0000_0000_0001, MirIntegerType::I64),
        (0xfff8_0000_0000_0042, MirIntegerType::U64),
        (0xfff0_0000_0000_0001, MirIntegerType::U8),
    ] {
        assert_native_checked_cast_failure(bits, target);
    }
}

#[test]
fn pure_casts_to_i64_match_the_independent_oracle() {
    assert_pure_cast_samples(MirPrimitiveType::I64);
}

#[test]
fn pure_casts_to_u64_match_the_independent_oracle() {
    assert_pure_cast_samples(MirPrimitiveType::U64);
}

#[test]
fn pure_casts_to_u8_match_the_independent_oracle() {
    assert_pure_cast_samples(MirPrimitiveType::U8);
}

#[test]
fn pure_casts_to_f64_match_the_independent_oracle() {
    assert_pure_cast_samples(MirPrimitiveType::F64);
}

#[test]
fn pure_casts_to_bool_match_the_independent_oracle() {
    assert_pure_cast_samples(MirPrimitiveType::Bool);
}

#[test]
fn checked_casts_to_i64_match_the_independent_post_truncation_oracle() {
    assert_checked_cast_samples(MirIntegerType::I64);
}

#[test]
fn checked_casts_to_u64_match_the_independent_post_truncation_oracle() {
    assert_checked_cast_samples(MirIntegerType::U64);
}

#[test]
fn checked_casts_to_u8_match_the_independent_post_truncation_oracle() {
    assert_checked_cast_samples(MirIntegerType::U8);
}

#[test]
fn literal_and_dynamically_produced_checked_casts_have_identical_behavior() {
    for (target, value, expected_status) in
        [("i64", "7.9", 7), ("u64", "42.9", 42), ("u8", "255.9", 255)]
    {
        let direct = run_checked_source(&format!(
            "fn main() -> i64 {{ return (i64) ({target}) {value}; }}\n"
        ));
        let dynamic = run_checked_source(&format!(
            "fn source() -> f64 {{ return {value}; }}\n\
             fn main() -> i64 {{ return (i64) ({target}) source(); }}\n"
        ));
        assert_eq!(direct.status.code(), Some(expected_status));
        assert_eq!(dynamic.status.code(), Some(expected_status));
        assert_eq!(direct.stdout, dynamic.stdout);
        assert_eq!(direct.stderr, dynamic.stderr);
    }

    for (target, value) in [
        ("i64", "9223372036854775808.0"),
        ("u64", "-1.0"),
        ("u8", "256.0"),
    ] {
        let direct = run_checked_source(&format!(
            "fn main() -> i64 {{ return (i64) ({target}) {value}; }}\n"
        ));
        let dynamic = run_checked_source(&format!(
            "fn source() -> f64 {{ return {value}; }}\n\
             fn main() -> i64 {{ return (i64) ({target}) source(); }}\n"
        ));
        assert_eq!(direct.status.code(), Some(1));
        assert_eq!(dynamic.status.code(), Some(1));
        assert_eq!(direct.stdout, dynamic.stdout);
        assert_eq!(direct.stderr, dynamic.stderr);
        assert_eq!(direct.stderr, b"panic: floating-point cast out of range\n");
    }
}

#[test]
fn complete_cast_object_uses_only_the_existing_runtime_abi_surface() {
    let source = concat!(
        "fn bits(value: i64) -> u8 { return (u8) value; }\n",
        "fn truth(value: f64) -> bool { return (bool) value; }\n",
        "fn number(value: bool) -> f64 { return (f64) value; }\n",
        "fn rounded(value: u64) -> f64 { return (f64) value; }\n",
        "fn signed(value: f64) -> i64 { return (i64) value; }\n",
        "fn unsigned(value: f64) -> u64 { return (u64) value; }\n",
        "fn byte(value: f64) -> u8 { return (u8) value; }\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let output = assembly(source);
    let undefined = undefined_object_symbols(&output);

    assert!(undefined
        .lines()
        .any(|line| line.ends_with(" ska_rt_abi_v8")));
    assert!(undefined
        .lines()
        .any(|line| line.ends_with(" ska_rt_panic")));
    assert!(!undefined.lines().any(|line| line.contains("cast")));

    let header = include_str!("../../../../../../runtime/include/skald_runtime.h");
    assert!(header.contains("#define SKALD_RUNTIME_ABI_MARKER ska_rt_abi_v8"));
    assert!(header.contains("_Noreturn void ska_rt_panic(const uint8_t* bytes, uint64_t length);"));
    assert!(!header.contains("primitive_cast"));
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

#[test]
fn signed_integer_to_f64_matches_an_exact_integer_oracle() {
    for value in [
        i64::MIN,
        i64::MIN + 1,
        -(1_i64 << 62) - 1,
        -(1_i64 << 62),
        -(1_i64 << 53) - 3,
        -(1_i64 << 53) - 2,
        -(1_i64 << 53) - 1,
        -(1_i64 << 53),
        -(1_i64 << 53) + 1,
        -257,
        -256,
        -255,
        -2,
        -1,
        0,
        1,
        2,
        255,
        256,
        257,
        (1_i64 << 52) - 1,
        1_i64 << 52,
        (1_i64 << 53) - 1,
        1_i64 << 53,
        (1_i64 << 53) + 1,
        (1_i64 << 53) + 2,
        (1_i64 << 53) + 3,
        (1_i64 << 54) + 2,
        (1_i64 << 54) + 6,
        i64::MAX - 1,
        i64::MAX,
    ] {
        assert_native_cast(
            PrimitiveValue::I64(value),
            MirPrimitiveType::F64,
            signed_integer_to_f64_bits(value),
        );
    }
}

#[test]
fn representative_u8_values_convert_exactly_to_f64() {
    for value in U8_TO_F64_SAMPLES {
        assert_native_cast(
            PrimitiveValue::U8(value),
            MirPrimitiveType::F64,
            unsigned_integer_to_f64_bits(u64::from(value)),
        );
    }
}

#[test]
fn full_domain_u64_to_f64_matches_an_exact_integer_oracle() {
    for value in [
        0,
        1,
        2,
        255,
        256,
        257,
        (1_u64 << 52) - 1,
        1_u64 << 52,
        (1_u64 << 53) - 1,
        1_u64 << 53,
        (1_u64 << 53) + 1,
        (1_u64 << 53) + 2,
        (1_u64 << 53) + 3,
        (1_u64 << 54) + 1,
        (1_u64 << 54) + 2,
        (1_u64 << 54) + 3,
        (1_u64 << 54) + 5,
        (1_u64 << 54) + 6,
        (1_u64 << 54) + 7,
        (1_u64 << 62) - 1,
        1_u64 << 62,
        (1_u64 << 63) - 2,
        (1_u64 << 63) - 1,
        1_u64 << 63,
        (1_u64 << 63) + 1,
        (1_u64 << 63) + 2,
        u64::MAX - 2,
        u64::MAX - 1,
        u64::MAX,
    ] {
        assert_native_cast(
            PrimitiveValue::U64(value),
            MirPrimitiveType::F64,
            unsigned_integer_to_f64_bits(value),
        );
    }
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

fn assert_pure_cast_samples(target: MirPrimitiveType) {
    for source in pure_cast_samples() {
        let Some(expected) = expected_pure_cast(source, target) else {
            continue;
        };
        assert_native_cast(source, target, expected);
    }
}

fn assert_checked_cast_samples(target: MirIntegerType) {
    for bits in checked_cast_samples() {
        match expected_checked_cast(bits, target) {
            Some(expected) => assert_native_checked_cast(bits, target, expected),
            None => assert_native_checked_cast_failure(bits, target),
        }
    }
}

fn cast_function(source: PrimitiveValue, target: MirPrimitiveType) -> String {
    let output =
        emit_assembly(Target::X86_64SysV, &primitive_cast_program(source, target)).unwrap();
    function_assembly(&output, CAST_FUNCTION).to_owned()
}

fn cast_block_assembly(function: &str, index: usize) -> &str {
    let marker = format!(".Lska.fn.main.main.f0.block_{index}:");
    let start = function
        .find(&marker)
        .expect("checked-cast block is emitted");
    let remaining = &function[start..];
    remaining
        .find("\n.Lska.fn.main.main.f0.block_")
        .map(|end| &remaining[..end])
        .unwrap_or(remaining)
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

fn assert_native_checked_cast(bits: u64, target: MirIntegerType, expected_bits: u64) {
    let program = fixture_checked_primitive_cast_program(bits, target, expected_bits);
    let mut assembly = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assembly.push_str(native_panic_reporter());
    let result = run_native_assembly_output(&assembly);
    assert_eq!(
        result.status.code(),
        Some(0),
        "f64 bits 0x{bits:016x} -> {target:?}, stderr {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

fn assert_native_checked_cast_failure(bits: u64, target: MirIntegerType) {
    let program = fixture_checked_primitive_cast_program(bits, target, 0);
    let mut assembly = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assembly.push_str(native_panic_reporter());
    let result = run_native_assembly_output(&assembly);
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(result.stderr, b"panic: floating-point cast out of range\n");
}

fn run_checked_source(source: &str) -> std::process::Output {
    let mut output = assembly(source);
    output.push_str(native_panic_reporter());
    run_native_assembly_output(&output)
}

fn undefined_object_symbols(assembly: &str) -> String {
    let object = TemporaryFile::new("primitive-cast-object").unwrap();
    let mut assembler = Command::new("cc")
        .args(["-x", "assembler", "-c", "-o"])
        .arg(object.path())
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("primitive-cast object inspection requires the Linux `cc` toolchain");
    assembler
        .stdin
        .take()
        .unwrap()
        .write_all(assembly.as_bytes())
        .unwrap();
    let assembled = assembler.wait_with_output().unwrap();
    assert!(
        assembled.status.success(),
        "assembler rejected primitive-cast object:\n{}",
        String::from_utf8_lossy(&assembled.stderr)
    );

    let symbols = Command::new("nm")
        .arg("--undefined-only")
        .arg(object.path())
        .output()
        .expect("primitive-cast object inspection requires `nm`");
    assert!(symbols.status.success());
    String::from_utf8(symbols.stdout).unwrap()
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
