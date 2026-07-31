use super::*;

#[derive(Clone, Copy)]
enum ExpectedFloat {
    ExactBits(u64),
    NaN,
}

fn exact_bits_validator(expected: u64) -> String {
    format!(
        concat!(
            "\n.text\n",
            ".globl validate_f64\n",
            ".type validate_f64, @function\n",
            "validate_f64:\n",
            "    movq rax, xmm0\n",
            "    mov rcx, 0x{expected:016x}\n",
            "    cmp rax, rcx\n",
            "    setne al\n",
            "    movzx rax, al\n",
            "    ret\n",
            ".size validate_f64, .-validate_f64\n",
        ),
        expected = expected,
    )
}

fn nan_validator() -> &'static str {
    concat!(
        "\n.text\n",
        ".globl validate_f64\n",
        ".type validate_f64, @function\n",
        "validate_f64:\n",
        "    movq rax, xmm0\n",
        "    mov rcx, rax\n",
        "    mov rdx, 0x7ff0000000000000\n",
        "    and rcx, rdx\n",
        "    cmp rcx, rdx\n",
        "    jne .Lvalidate_f64_not_nan\n",
        "    mov rcx, rax\n",
        "    mov rdx, 0x000fffffffffffff\n",
        "    and rcx, rdx\n",
        "    cmp rcx, 0\n",
        "    sete al\n",
        "    movzx rax, al\n",
        "    ret\n",
        ".Lvalidate_f64_not_nan:\n",
        "    mov rax, 1\n",
        "    ret\n",
        ".size validate_f64, .-validate_f64\n",
    )
}

fn run_division(dividend: u64, divisor: u64, expected: ExpectedFloat) {
    let program = f64_division_program(dividend, divisor);
    let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assert_eq!(output, emit_assembly(Target::X86_64SysV, &program).unwrap());
    match expected {
        ExpectedFloat::ExactBits(bits) => output.push_str(&exact_bits_validator(bits)),
        ExpectedFloat::NaN => output.push_str(nan_validator()),
    }

    let result = run_native_assembly_output(&output);
    assert!(
        result.status.success(),
        "division 0x{dividend:016x} / 0x{divisor:016x} failed with status {:?}, stderr {:?}",
        result.status.code(),
        String::from_utf8_lossy(&result.stderr),
    );
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

#[test]
fn emits_verified_scalar_binary64_division() {
    let program = f64_division_program(6.0_f64.to_bits(), 2.0_f64.to_bits());
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();

    assert!(output.contains("divsd xmm14, xmm15"));
    assert!(!output.contains("idiv"));
    assert!(!output.contains("integer division by zero"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn executes_binary64_division_edge_cases_without_panicking() {
    use ExpectedFloat::{ExactBits, NaN};

    let cases = [
        (
            6.0_f64.to_bits(),
            2.0_f64.to_bits(),
            ExactBits(3.0_f64.to_bits()),
        ),
        (
            1.0_f64.to_bits(),
            3.0_f64.to_bits(),
            ExactBits(0x3fd5_5555_5555_5555),
        ),
        (
            1.0_f64.to_bits(),
            0.0_f64.to_bits(),
            ExactBits(f64::INFINITY.to_bits()),
        ),
        (
            1.0_f64.to_bits(),
            (-0.0_f64).to_bits(),
            ExactBits(f64::NEG_INFINITY.to_bits()),
        ),
        (
            (-1.0_f64).to_bits(),
            0.0_f64.to_bits(),
            ExactBits(f64::NEG_INFINITY.to_bits()),
        ),
        (
            (-1.0_f64).to_bits(),
            (-0.0_f64).to_bits(),
            ExactBits(f64::INFINITY.to_bits()),
        ),
        (
            0.0_f64.to_bits(),
            2.0_f64.to_bits(),
            ExactBits(0.0_f64.to_bits()),
        ),
        (
            (-0.0_f64).to_bits(),
            2.0_f64.to_bits(),
            ExactBits((-0.0_f64).to_bits()),
        ),
        (
            0.0_f64.to_bits(),
            (-2.0_f64).to_bits(),
            ExactBits((-0.0_f64).to_bits()),
        ),
        (
            (-0.0_f64).to_bits(),
            (-2.0_f64).to_bits(),
            ExactBits(0.0_f64.to_bits()),
        ),
        (
            0x0000_0000_0000_0001,
            2.0_f64.to_bits(),
            ExactBits(0.0_f64.to_bits()),
        ),
        (
            0x8000_0000_0000_0001,
            2.0_f64.to_bits(),
            ExactBits((-0.0_f64).to_bits()),
        ),
        (
            0x0010_0000_0000_0000,
            2.0_f64.to_bits(),
            ExactBits(0x0008_0000_0000_0000),
        ),
        (
            f64::MAX.to_bits(),
            0.5_f64.to_bits(),
            ExactBits(f64::INFINITY.to_bits()),
        ),
        (
            f64::INFINITY.to_bits(),
            2.0_f64.to_bits(),
            ExactBits(f64::INFINITY.to_bits()),
        ),
        (
            f64::NEG_INFINITY.to_bits(),
            2.0_f64.to_bits(),
            ExactBits(f64::NEG_INFINITY.to_bits()),
        ),
        (
            f64::INFINITY.to_bits(),
            (-2.0_f64).to_bits(),
            ExactBits(f64::NEG_INFINITY.to_bits()),
        ),
        (
            f64::NEG_INFINITY.to_bits(),
            (-2.0_f64).to_bits(),
            ExactBits(f64::INFINITY.to_bits()),
        ),
        (
            2.0_f64.to_bits(),
            f64::INFINITY.to_bits(),
            ExactBits(0.0_f64.to_bits()),
        ),
        (
            (-2.0_f64).to_bits(),
            f64::INFINITY.to_bits(),
            ExactBits((-0.0_f64).to_bits()),
        ),
        (
            2.0_f64.to_bits(),
            f64::NEG_INFINITY.to_bits(),
            ExactBits((-0.0_f64).to_bits()),
        ),
        (
            (-2.0_f64).to_bits(),
            f64::NEG_INFINITY.to_bits(),
            ExactBits(0.0_f64.to_bits()),
        ),
        (0.0_f64.to_bits(), 0.0_f64.to_bits(), NaN),
        (f64::INFINITY.to_bits(), f64::INFINITY.to_bits(), NaN),
        (0x7ff8_0000_0000_0042, 1.0_f64.to_bits(), NaN),
        (0x7ff0_0000_0000_0042, 1.0_f64.to_bits(), NaN),
        (1.0_f64.to_bits(), 0x7ff8_0000_0000_0042, NaN),
        (1.0_f64.to_bits(), 0x7ff0_0000_0000_0042, NaN),
    ];

    for (dividend, divisor, expected) in cases {
        run_division(dividend, divisor, expected);
    }
}
