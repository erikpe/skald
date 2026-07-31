use super::*;
use crate::mir::{MirIntegerDivisionKind, MirIntegerDivisionOperation};

const fn operation(
    kind: MirIntegerDivisionKind,
    operand: MirIntegerType,
) -> MirIntegerDivisionOperation {
    MirIntegerDivisionOperation { kind, operand }
}

fn floor_div_rem(dividend: i64, divisor: i64) -> (i64, i64) {
    if dividend == i64::MIN && divisor == -1 {
        return (i64::MIN, 0);
    }
    let mut quotient = dividend / divisor;
    let mut remainder = dividend % divisor;
    if remainder != 0 && remainder.is_negative() != divisor.is_negative() {
        quotient -= 1;
        remainder += divisor;
    }
    (quotient, remainder)
}

fn division_assembly(
    operation: MirIntegerDivisionOperation,
    dividend: u64,
    divisor: u64,
    expected: u64,
) -> String {
    emit_assembly(
        Target::X86_64SysV,
        &fixture_checked_integer_division_program(operation, dividend, divisor, expected),
    )
    .unwrap()
}

#[test]
fn native_signed_division_and_remainder_follow_floor_semantics_without_traps() {
    let cases = [
        (7_i64, 3_i64, 2_i64, 1_i64),
        (-7, 3, -3, 2),
        (7, -3, -3, -2),
        (-7, -3, 2, -1),
        (-6, 3, -2, 0),
        (0, 1, 0, 0),
        (i64::MIN, -1, i64::MIN, 0),
        (i64::MIN, 1, i64::MIN, 0),
        (i64::MAX, -1, -i64::MAX, 0),
        (i64::MIN, i64::MAX, -2, i64::MAX - 1),
    ];

    for (dividend, divisor, quotient, remainder) in cases {
        assert_eq!(
            quotient.wrapping_mul(divisor).wrapping_add(remainder),
            dividend
        );
        for (kind, expected) in [
            (MirIntegerDivisionKind::Quotient, quotient),
            (MirIntegerDivisionKind::Remainder, remainder),
        ] {
            let assembly = division_assembly(
                operation(kind, MirIntegerType::I64),
                dividend as u64,
                divisor as u64,
                expected as u64,
            );
            assert_eq!(
                run_native_assembly(&assembly).code(),
                Some(0),
                "{kind:?}: {dividend} / {divisor}\n{assembly}"
            );
        }
    }
}

#[test]
fn native_signed_property_samples_match_the_floor_model() {
    let dividends = [i64::MIN, i64::MIN + 1, -257, -1, 0, i64::MAX];
    let divisors = [i64::MIN, -257, -3, -1, 1, i64::MAX];
    let mut observations = Vec::new();

    for dividend in dividends {
        for divisor in divisors {
            let (quotient, remainder) = floor_div_rem(dividend, divisor);
            assert_eq!(
                quotient.wrapping_mul(divisor).wrapping_add(remainder),
                dividend,
                "identity for {dividend} / {divisor}"
            );
            assert!(remainder == 0 || remainder.is_negative() == divisor.is_negative());
            assert!(remainder.unsigned_abs() < divisor.unsigned_abs());
            observations.push(format!(
                "({dividend} / {divisor}) == {quotient} && ({dividend} % {divisor}) == {remainder}"
            ));
        }
    }

    let checks = observations
        .iter()
        .map(|observation| format!("if (!({observation})) {{ return 91; }}"))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("fn main() -> i64 {{ {checks} return 0; }}");
    assert_eq!(run_native_assembly(&assembly(&source)).code(), Some(0));
}

#[test]
fn native_unsigned_and_byte_division_cover_high_bits_and_canonical_results() {
    let u64_cases = [
        (0, 1, 0, 0),
        (1_u64 << 63, 2, 1_u64 << 62, 0),
        (u64::MAX, 1, u64::MAX, 0),
        (u64::MAX, 2, u64::MAX / 2, 1),
        (u64::MAX, u64::MAX, 1, 0),
    ];
    for (dividend, divisor, quotient, remainder) in u64_cases {
        for (kind, expected) in [
            (MirIntegerDivisionKind::Quotient, quotient),
            (MirIntegerDivisionKind::Remainder, remainder),
        ] {
            let assembly = division_assembly(
                operation(kind, MirIntegerType::U64),
                dividend,
                divisor,
                expected,
            );
            assert_eq!(run_native_assembly(&assembly).code(), Some(0));
        }
    }

    for (dividend, divisor, quotient, remainder) in [
        (0, 1, 0, 0),
        (255, 1, 255, 0),
        (255, 2, 127, 1),
        (255, 255, 1, 0),
    ] {
        for (kind, expected) in [
            (MirIntegerDivisionKind::Quotient, quotient),
            (MirIntegerDivisionKind::Remainder, remainder),
        ] {
            let assembly = division_assembly(
                operation(kind, MirIntegerType::U8),
                dividend,
                divisor,
                expected,
            );
            assert_eq!(run_native_assembly(&assembly).code(), Some(0));
            let divide = assembly.find("div rcx").unwrap();
            assert!(assembly[divide..].contains("movzx rax, al"));
        }
    }
}

#[test]
fn assembly_guards_zero_and_signed_overflow_before_dividing_and_corrects_after_idiv() {
    let signed = division_assembly(
        operation(MirIntegerDivisionKind::Remainder, MirIntegerType::I64),
        (-7_i64) as u64,
        3,
        2,
    );
    assert_eq!(
        signed,
        division_assembly(
            operation(MirIntegerDivisionKind::Remainder, MirIntegerType::I64),
            (-7_i64) as u64,
            3,
            2,
        )
    );
    let function = function_assembly(&signed, ".Lska.fn.main.main.f0");
    let zero_test = function.find("test rax, rax").unwrap();
    let min_guard = function.find("mov r11, 0x8000000000000000").unwrap();
    let minus_one_guard = function.find("mov r11, 0xffffffffffffffff").unwrap();
    let sign_extend = function.find("cqo").unwrap();
    let divide = function.find("idiv rcx").unwrap();
    let remainder_test = function[divide..].find("test rdx, rdx").unwrap() + divide;
    let sign_compare = function[divide..].find("xor r11, rcx").unwrap() + divide;
    let quotient_correction = function[divide..].find("sub rax, r10").unwrap() + divide;
    let remainder_correction = function[divide..].find("add rdx, rcx").unwrap() + divide;
    assert!(zero_test < min_guard);
    assert!(min_guard < minus_one_guard && minus_one_guard < sign_extend);
    assert!(sign_extend < divide && divide < remainder_test);
    assert!(remainder_test < sign_compare && sign_compare < quotient_correction);
    assert!(quotient_correction < remainder_correction);
    assert_system_assembler_accepts(&signed);

    let unsigned = division_assembly(
        operation(MirIntegerDivisionKind::Quotient, MirIntegerType::U64),
        u64::MAX,
        2,
        u64::MAX / 2,
    );
    let function = function_assembly(&unsigned, ".Lska.fn.main.main.f0");
    let zero_high = function.find("xor rdx, rdx").unwrap();
    let divide = function.find("div rcx").unwrap();
    assert!(zero_high < divide);
    assert!(!function.contains("idiv rcx"));
    assert_system_assembler_accepts(&unsigned);
}

#[test]
fn zero_divisors_report_the_exact_operation_specific_panic() {
    for operand in [MirIntegerType::I64, MirIntegerType::U64, MirIntegerType::U8] {
        for (kind, message, symbol) in [
            (
                MirIntegerDivisionKind::Quotient,
                "integer division by zero",
                ".Lska_panic_message_10",
            ),
            (
                MirIntegerDivisionKind::Remainder,
                "integer remainder by zero",
                ".Lska_panic_message_11",
            ),
        ] {
            let mut assembly = division_assembly(operation(kind, operand), 7, 0, 0);
            let function = function_assembly(&assembly, ".Lska.fn.main.main.f0");
            let zero_test = function.find("test rax, rax").unwrap();
            let failure_jump = function[zero_test..].find("jmp ").unwrap() + zero_test;
            assert!(!function[..failure_jump].contains("div rcx"));
            assert!(assembly.contains(symbol));
            assert!(assembly.contains(&format!(".ascii \"{message}\"")));
            assembly.push_str(native_panic_reporter());
            let result = run_native_assembly_output(&assembly);
            assert_eq!(result.status.code(), Some(1));
            assert!(result.stdout.is_empty());
            assert_eq!(result.stderr, format!("panic: {message}\n").as_bytes());
        }
    }
}

#[test]
fn literal_and_call_produced_zero_divisors_take_the_same_checked_failure_edge() {
    for source in [
        "fn main() -> i64 { return 7 / 0; }",
        concat!(
            "fn produce_zero() -> i64 { return 0; }\n",
            "fn main() -> i64 { return 7 / produce_zero(); }\n",
        ),
    ] {
        let mut assembly = assembly(source);
        assert!(!assembly.contains("integer-divisor-check"));
        assert!(assembly.contains("test rax, rax"));
        assert!(assembly.contains(".ascii \"integer division by zero\""));
        assembly.push_str(native_panic_reporter());
        let result = run_native_assembly_output(&assembly);
        assert_eq!(result.status.code(), Some(1));
        assert_eq!(result.stderr, b"panic: integer division by zero\n");
    }
}

#[test]
fn backend_rejects_division_that_loses_its_verified_check_shape() {
    let mut program = fixture_checked_integer_division_program(
        operation(MirIntegerDivisionKind::Quotient, MirIntegerType::I64),
        7,
        3,
        2,
    );
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    definition.body.blocks[0].terminator = Some(MirTerminator::Goto {
        target: BlockId::new(definition.function, 1),
        span: definition.span,
    });

    let error = emit_assembly(Target::X86_64SysV, &program).unwrap_err();
    assert!(error
        .message()
        .contains("operation is not protected by its matching divisor check"));
}
