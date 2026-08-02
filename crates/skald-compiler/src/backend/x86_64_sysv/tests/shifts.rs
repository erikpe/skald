use super::*;
use crate::mir::{MirShiftDirection, MirShiftOperation};

fn operation(direction: MirShiftDirection, left: MirIntegerType) -> MirShiftOperation {
    MirShiftOperation { direction, left }
}

fn shift_assembly(operation: MirShiftOperation, left: u64, count: u64, expected: u64) -> String {
    emit_assembly(
        Target::X86_64SysV,
        &fixture_checked_shift_program(operation, left, count, expected),
    )
    .unwrap()
}

#[test]
fn selects_explicit_checks_and_exact_x86_shift_flavors() {
    for (operation, mnemonic) in [
        (
            operation(MirShiftDirection::Left, MirIntegerType::I64),
            "shl",
        ),
        (
            operation(MirShiftDirection::Right, MirIntegerType::I64),
            "sar",
        ),
        (
            operation(MirShiftDirection::Left, MirIntegerType::U64),
            "shl",
        ),
        (
            operation(MirShiftDirection::Right, MirIntegerType::U64),
            "shr",
        ),
        (
            operation(MirShiftDirection::Left, MirIntegerType::U8),
            "shl",
        ),
        (
            operation(MirShiftDirection::Right, MirIntegerType::U8),
            "shr",
        ),
    ] {
        let output = shift_assembly(operation, 0x81, 1, 0);
        assert_eq!(output, shift_assembly(operation, 0x81, 1, 0));
        let function = function_assembly(&output, ".Lska.fn.main.main.f0");
        let compare = function.find("cmp rax, r11").unwrap();
        let valid_jump = function[compare..].find("jb ").unwrap() + compare;
        let count_register = function.find("mov rcx, qword ptr [rbp").unwrap();
        let shift = function.find(&format!("{mnemonic} rax, cl")).unwrap();
        assert!(compare < valid_jump && valid_jump < count_register && count_register < shift);
        assert_eq!(
            function[..shift].matches("mov rcx, qword ptr [rbp").count(),
            1
        );
        assert!(!function[..valid_jump].contains("rcx"));
        assert!(!function.contains("and rcx"));
        assert!(!output.contains("ska_rt_shift"));
        assert!(output.contains("call ska_rt_abi_v7"));
        assert_system_assembler_accepts(&output);

        if operation.left == MirIntegerType::U8 {
            let lines: Vec<_> = function.lines().map(str::trim).collect();
            let index = lines
                .iter()
                .position(|line| *line == format!("{mnemonic} rax, cl"))
                .unwrap();
            assert_eq!(lines[index + 1], "movzx rax, al");
        }
    }
}

#[test]
fn native_shifts_cover_zero_edges_signedness_discard_and_byte_canonicalization() {
    let cases = [
        (
            operation(MirShiftDirection::Left, MirIntegerType::I64),
            7,
            0,
            7,
        ),
        (
            operation(MirShiftDirection::Left, MirIntegerType::I64),
            1,
            63,
            1 << 63,
        ),
        (
            operation(MirShiftDirection::Left, MirIntegerType::I64),
            1 << 63,
            1,
            0,
        ),
        (
            operation(MirShiftDirection::Right, MirIntegerType::I64),
            (-8_i64) as u64,
            1,
            (-4_i64) as u64,
        ),
        (
            operation(MirShiftDirection::Right, MirIntegerType::U64),
            1 << 63,
            63,
            1,
        ),
        (
            operation(MirShiftDirection::Left, MirIntegerType::U8),
            0x81,
            1,
            2,
        ),
        (
            operation(MirShiftDirection::Right, MirIntegerType::U8),
            0x81,
            7,
            1,
        ),
    ];
    for (operation, left, count, expected) in cases {
        let output = shift_assembly(operation, left, count, expected);
        assert_eq!(
            run_native_assembly(&output).code(),
            Some(0),
            "{operation:?} count={count}"
        );
    }
}

#[test]
fn invalid_native_counts_report_the_exact_frozen_failure_before_shifting() {
    for (integer, count) in [
        (MirIntegerType::I64, 64),
        (MirIntegerType::U64, 65),
        (MirIntegerType::U64, u64::MAX),
        (MirIntegerType::U8, 8),
        (MirIntegerType::U8, 9),
        (MirIntegerType::U8, u64::MAX),
    ] {
        let operation = operation(MirShiftDirection::Left, integer);
        let mut output = shift_assembly(operation, 1, count, 0);
        assert!(output.contains(".Lska_panic_message_9"));
        assert!(output.contains(".ascii \"shift count out of range\""));
        output.push_str(native_panic_reporter());
        let result = run_native_assembly_output(&output);
        assert_eq!(result.status.code(), Some(1), "{integer:?} count={count}");
        assert!(result.stdout.is_empty());
        assert_eq!(result.stderr, b"panic: shift count out of range\n");
    }
}
