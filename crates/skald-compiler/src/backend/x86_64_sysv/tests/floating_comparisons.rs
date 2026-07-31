use super::*;

const PREDICATES: [MirComparisonPredicate; 6] = [
    MirComparisonPredicate::Equal,
    MirComparisonPredicate::NotEqual,
    MirComparisonPredicate::LessThan,
    MirComparisonPredicate::LessEqual,
    MirComparisonPredicate::GreaterThan,
    MirComparisonPredicate::GreaterEqual,
];

fn expected_comparison(predicate: MirComparisonPredicate, left_bits: u64, right_bits: u64) -> bool {
    let left = f64::from_bits(left_bits);
    let right = f64::from_bits(right_bits);
    if left.is_nan() || right.is_nan() {
        return predicate == MirComparisonPredicate::NotEqual;
    }
    match predicate {
        MirComparisonPredicate::Equal => left == right,
        MirComparisonPredicate::NotEqual => left != right,
        MirComparisonPredicate::LessThan => left < right,
        MirComparisonPredicate::LessEqual => left <= right,
        MirComparisonPredicate::GreaterThan => left > right,
        MirComparisonPredicate::GreaterEqual => left >= right,
    }
}

fn expected_truth_row(left_bits: u64, right_bits: u64) -> [bool; 6] {
    PREDICATES.map(|predicate| expected_comparison(predicate, left_bits, right_bits))
}

#[test]
fn ordered_floating_rows_obey_trichotomy_and_valid_predicate_duals() {
    let pairs = [
        ((-3.0_f64).to_bits(), 2.0_f64.to_bits()),
        (2.0_f64.to_bits(), 2.0_f64.to_bits()),
        (3.0_f64.to_bits(), 2.0_f64.to_bits()),
        (0.0_f64.to_bits(), (-0.0_f64).to_bits()),
        (f64::NEG_INFINITY.to_bits(), f64::INFINITY.to_bits()),
        (f64::INFINITY.to_bits(), f64::INFINITY.to_bits()),
    ];

    for (left, right) in pairs {
        let [equal, not_equal, less, less_equal, greater, greater_equal] =
            expected_truth_row(left, right);
        let reversed = expected_truth_row(right, left);

        assert_eq!(equal, !not_equal);
        assert_eq!(u8::from(less) + u8::from(equal) + u8::from(greater), 1);
        assert_eq!(less_equal, less || equal);
        assert_eq!(greater_equal, greater || equal);
        assert_eq!(less, reversed[4]);
        assert_eq!(less_equal, reversed[5]);
        assert_eq!(greater, reversed[2]);
        assert_eq!(greater_equal, reversed[3]);
    }
}

#[test]
fn emits_explicit_unordered_gating_for_every_floating_predicate() {
    let cases = [
        (
            MirComparisonPredicate::Equal,
            "sete al",
            "setnp cl",
            "and al, cl",
        ),
        (
            MirComparisonPredicate::NotEqual,
            "setne al",
            "setp cl",
            "or al, cl",
        ),
        (
            MirComparisonPredicate::LessThan,
            "setb al",
            "setnp cl",
            "and al, cl",
        ),
        (
            MirComparisonPredicate::LessEqual,
            "setbe al",
            "setnp cl",
            "and al, cl",
        ),
        (
            MirComparisonPredicate::GreaterThan,
            "seta al",
            "setnp cl",
            "and al, cl",
        ),
        (
            MirComparisonPredicate::GreaterEqual,
            "setae al",
            "setnp cl",
            "and al, cl",
        ),
    ];

    for (predicate, relation, parity, combine) in cases {
        let program = f64_comparison_program(
            predicate,
            1.0_f64.to_bits(),
            2.0_f64.to_bits(),
            expected_comparison(predicate, 1.0_f64.to_bits(), 2.0_f64.to_bits()),
        );
        let output = emit_assembly(Target::X86_64SysV, &program).unwrap();
        assert_eq!(output, emit_assembly(Target::X86_64SysV, &program).unwrap());
        assert!(output.contains("ucomisd xmm14, xmm15"));
        assert!(output.contains(relation));
        assert!(output.contains(parity));
        assert!(output.contains(combine));
        assert!(output.contains("movzx rax, al"));
        assert!(!output.contains("ska_rt_compare"));
        assert_system_assembler_accepts(&output);
    }
}

#[test]
fn executes_the_complete_unordered_floating_truth_table() {
    const QUIET_NAN_A: u64 = 0x7ff8_0000_0000_0001;
    const QUIET_NAN_B: u64 = 0xfff8_0000_0000_1234;
    const SIGNALING_NAN_A: u64 = 0x7ff0_0000_0000_0001;
    const SIGNALING_NAN_B: u64 = 0xfff0_0000_0000_4321;

    let pairs = [
        ((-3.0_f64).to_bits(), 2.0_f64.to_bits()),
        (2.0_f64.to_bits(), 2.0_f64.to_bits()),
        (3.0_f64.to_bits(), 2.0_f64.to_bits()),
        (0.0_f64.to_bits(), (-0.0_f64).to_bits()),
        ((-0.0_f64).to_bits(), 0.0_f64.to_bits()),
        (f64::NEG_INFINITY.to_bits(), 0.0_f64.to_bits()),
        (0.0_f64.to_bits(), f64::INFINITY.to_bits()),
        (f64::NEG_INFINITY.to_bits(), f64::NEG_INFINITY.to_bits()),
        (f64::INFINITY.to_bits(), f64::INFINITY.to_bits()),
        (f64::NEG_INFINITY.to_bits(), f64::INFINITY.to_bits()),
        (QUIET_NAN_A, 1.0_f64.to_bits()),
        (1.0_f64.to_bits(), QUIET_NAN_B),
        (SIGNALING_NAN_A, 1.0_f64.to_bits()),
        (1.0_f64.to_bits(), SIGNALING_NAN_B),
        (QUIET_NAN_A, QUIET_NAN_B),
        (SIGNALING_NAN_A, SIGNALING_NAN_B),
    ];

    for predicate in PREDICATES {
        for (left, right) in pairs {
            let expected = expected_comparison(predicate, left, right);
            let program = f64_comparison_program(predicate, left, right, expected);
            let output = emit_assembly(Target::X86_64SysV, &program).unwrap();
            assert_eq!(
                output,
                emit_assembly(Target::X86_64SysV, &program).unwrap(),
                "{} assembly changed for 0x{left:016x} and 0x{right:016x}",
                predicate.mnemonic(),
            );
            let result = run_native_assembly_output(&output);
            assert!(
                result.status.success(),
                "{} failed for 0x{left:016x} and 0x{right:016x}: status {:?}, stderr {:?}",
                predicate.mnemonic(),
                result.status.code(),
                String::from_utf8_lossy(&result.stderr),
            );
            assert!(result.stdout.is_empty());
            assert!(result.stderr.is_empty());
        }
    }
}
