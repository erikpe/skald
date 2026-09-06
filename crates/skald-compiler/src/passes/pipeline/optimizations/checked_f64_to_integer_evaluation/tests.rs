use crate::mir::{MirF64ToIntegerRange, MirIntegerType, MirTerminationReason};

use super::*;

fn evaluate(target: MirIntegerType, bits: u64) -> CheckedF64ToIntegerEvaluation {
    evaluate_f64_to_integer(
        MirF64ToIntegerRange { target },
        PrimitiveConstant::F64Bits(bits),
    )
}

#[test]
fn evaluates_exact_successes_for_every_integer_target() {
    for (target, bits, expected) in [
        (
            MirIntegerType::I64,
            0xc022_6666_6666_6666,
            PrimitiveConstant::I64(-9),
        ),
        (
            MirIntegerType::I64,
            0xc3e0_0000_0000_0000,
            PrimitiveConstant::I64(i64::MIN),
        ),
        (
            MirIntegerType::U64,
            0xbfef_ffff_ffff_ffff,
            PrimitiveConstant::U64(0),
        ),
        (
            MirIntegerType::U64,
            0x43ef_ffff_ffff_ffff,
            PrimitiveConstant::U64(18_446_744_073_709_549_568),
        ),
        (
            MirIntegerType::U8,
            0x406f_ffff_ffff_ffff,
            PrimitiveConstant::U8(255),
        ),
    ] {
        assert_eq!(
            evaluate(target, bits),
            CheckedF64ToIntegerEvaluation::Success(expected),
            "target={target:?} bits={bits:#018x}"
        );
    }
}

#[test]
fn reports_nan_infinity_and_finite_range_failures_without_a_result() {
    for (target, bits) in [
        (MirIntegerType::I64, 0x7ff8_0000_0000_0042),
        (MirIntegerType::I64, 0x7ff0_0000_0000_0000),
        (MirIntegerType::I64, 0x43e0_0000_0000_0000),
        (MirIntegerType::U64, 0xbff0_0000_0000_0000),
        (MirIntegerType::U64, 0x43f0_0000_0000_0000),
        (MirIntegerType::U8, 0x4070_0000_0000_0000),
    ] {
        assert_eq!(
            evaluate(target, bits),
            CheckedF64ToIntegerEvaluation::Failure(MirTerminationReason::PrimitiveCastOutOfRange),
            "target={target:?} bits={bits:#018x}"
        );
    }
}

#[test]
fn rejects_non_floating_source_facts() {
    for source in [
        PrimitiveConstant::I64(1),
        PrimitiveConstant::U64(1),
        PrimitiveConstant::U8(1),
        PrimitiveConstant::Bool(true),
    ] {
        assert_eq!(
            evaluate_f64_to_integer(
                MirF64ToIntegerRange {
                    target: MirIntegerType::I64,
                },
                source,
            ),
            CheckedF64ToIntegerEvaluation::Unsupported
        );
    }
}
