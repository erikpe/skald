use super::oracle;
use crate::{
    backend::{
        effects::Effect,
        failure::FailureMessage,
        lir::*,
        lowering::{lower_next, lower_program},
        plan::*,
        planning::plan_program,
        BackendInput,
    },
    test_support::lower_source_to_complete_final_mir_with_sources,
};

#[test]
fn numeric_boundary_cases_execute_the_verified_lowered_graph() {
    use Constant::*;
    use FailureMessage::*;
    let cases = [
        ("i64", "-7 / 3", Ok(I64(-3))),
        ("i64", "-7 % 3", Ok(I64(2))),
        ("i64", "7 / -3", Ok(I64(-3))),
        ("i64", "7 % -3", Ok(I64(-2))),
        ("i64", "-9223372036854775808 / -1", Ok(I64(i64::MIN))),
        ("i64", "-9223372036854775808 % -1", Ok(I64(0))),
        ("i64", "7 / 0", Err(IntegerDivisionByZero)),
        ("i64", "7 % 0", Err(IntegerRemainderByZero)),
        ("u64", "18446744073709551615u / 2u", Ok(U64(u64::MAX / 2))),
        ("u8", "255u8 % 2u8", Ok(U8(1))),
        ("u64", "1u / 0u", Err(IntegerDivisionByZero)),
        ("u8", "1u8 % 0u8", Err(IntegerRemainderByZero)),
        ("i64", "-8 >> 2u", Ok(I64(-2))),
        ("u64", "18446744073709551615u >> 63u", Ok(U64(1))),
        ("u8", "128u8 >> 7u", Ok(U8(1))),
        ("u8", "255u8 << 1u", Ok(U8(254))),
        ("u8", "1u8 << 8u", Err(ShiftCountOutOfRange)),
        ("i64", "1 << 64u", Err(ShiftCountOutOfRange)),
        ("u64", "1u << 256u", Err(ShiftCountOutOfRange)),
        (
            "u8",
            "1u8 << 18446744073709551615u",
            Err(ShiftCountOutOfRange),
        ),
        ("i64", "(i64) -0.5", Ok(I64(0))),
        ("u64", "(u64) -0.5", Ok(U64(0))),
        ("u8", "(u8) 255.9", Ok(U8(255))),
        ("u8", "(u8) 256.0", Err(PrimitiveCastOutOfRange)),
        ("u64", "(u64) -1.0", Err(PrimitiveCastOutOfRange)),
        (
            "i64",
            "(i64) 9223372036854775808.0",
            Err(PrimitiveCastOutOfRange),
        ),
        (
            "u64",
            "(u64) 18446744073709551616.0",
            Err(PrimitiveCastOutOfRange),
        ),
    ];
    for (ty, expression, expected) in cases {
        let source = format!(
            "fn kernel() -> {ty} {{ return {expression}; }} fn main() -> i64 {{ return 0; }}"
        );
        let fixture = lower_source_to_complete_final_mir_with_sources("numeric.ska", &source);
        let planned = plan_program(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
        let mut worklist = ProgramBuilder::new(planned.plan().view());
        let body = lower_next(&planned, &mut worklist).unwrap().unwrap();
        assert_eq!(oracle::run(&body), expected, "{expression}");
        assert!(body.receipt().matches(&body));
    }
}

#[test]
fn complete_primitive_cast_matrix_publishes_the_expected_conversion_cells() {
    let types = [
        ScalarType::I64,
        ScalarType::U64,
        ScalarType::U8,
        ScalarType::F64,
        ScalarType::Bool,
    ];
    let names = ["i64", "u64", "u8", "f64", "bool"];
    let expected = [
        [
            Conversion::Identity,
            Conversion::IntegerBits,
            Conversion::IntegerBits,
            Conversion::ToFloat,
            Conversion::ToBoolean,
        ],
        [
            Conversion::IntegerBits,
            Conversion::Identity,
            Conversion::IntegerBits,
            Conversion::ToFloat,
            Conversion::ToBoolean,
        ],
        [
            Conversion::IntegerBits,
            Conversion::IntegerBits,
            Conversion::Identity,
            Conversion::ToFloat,
            Conversion::ToBoolean,
        ],
        [
            Conversion::TruncateFloat,
            Conversion::TruncateFloat,
            Conversion::TruncateFloat,
            Conversion::Identity,
            Conversion::ToBoolean,
        ],
        [
            Conversion::FromBoolean,
            Conversion::FromBoolean,
            Conversion::FromBoolean,
            Conversion::ToFloat,
            Conversion::Identity,
        ],
    ];
    for (from, source) in names.iter().enumerate() {
        for (to, target) in names.iter().enumerate() {
            let source = format!("fn convert(value: {source}) -> {target} {{ return ({target}) value; }} fn main() -> i64 {{ return 0; }}");
            let fixture = lower_source_to_complete_final_mir_with_sources("casts.ska", &source);
            let planned = plan_program(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
            let mut worklist = ProgramBuilder::new(planned.plan().view());
            let body = lower_next(&planned, &mut worklist).unwrap().unwrap();
            let conversions = body
                .draft()
                .blocks()
                .flat_map(|(_, b)| &b.instructions)
                .filter_map(|i| match i.operation {
                    Operation::Convert {
                        conversion,
                        value,
                        target,
                        ref evidence,
                    } => Some((
                        conversion,
                        body.draft()
                            .values()
                            .find(|(id, _)| *id == value)
                            .unwrap()
                            .1
                            .ty,
                        target,
                        evidence.is_some(),
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                conversions,
                vec![(
                    expected[from][to],
                    types[from],
                    types[to],
                    from == 3 && to < 3
                )],
                "{source}"
            );
        }
    }
}

#[test]
fn numeric_failures_preserve_exact_reporting_data_effects_and_source_origins() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "failure.ska",
        "fn kernel(a: i64, b: i64) -> i64 { return a / b; } fn main() -> i64 { return 0; }",
    );
    let planned = plan_program(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    let mut worklist = ProgramBuilder::new(planned.plan().view());
    let body = lower_next(&planned, &mut worklist).unwrap().unwrap();
    let (_, block) = body
        .draft()
        .blocks()
        .find(|(_, b)| matches!(b.terminator, Some(Terminator::ReportFailure { .. })))
        .unwrap();
    let Some(Terminator::ReportFailure { call, reason }) = &block.terminator else {
        unreachable!()
    };
    assert_eq!(*reason, FailureMessage::IntegerDivisionByZero);
    assert_eq!(
        call.target,
        CallTarget::Direct(ArtifactId::Runtime(RuntimeService::Panic))
    );
    assert!(
        matches!(call.attribution, CallAttribution::SourceOperation { location: None, origin } if origin == fixture.mir.program().definitions.iter().next().unwrap().body.blocks.iter().find_map(|b| match b.terminator { Some(crate::mir::MirTerminator::Terminate { span, .. }) => Some(span), _ => None }).unwrap())
    );
    assert!(block
        .terminal_effects
        .as_ref()
        .unwrap()
        .contains(Effect::Report));
    assert!(body
        .receipt()
        .references()
        .contains(&ArtifactId::Data(DataKey::FailureMessage(*reason))));
    assert!(body
        .receipt()
        .references()
        .contains(&ArtifactId::Runtime(RuntimeService::Panic)));
}

#[test]
fn final_publication_rejects_forged_and_bypassed_numeric_evidence() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "forged.ska",
        "fn divide(a: i64, b: i64) -> i64 { return a / b; } fn main() -> i64 { return 0; }",
    );
    let planned = plan_program(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    let key = ProgramBuilder::new(planned.plan().view()).next().unwrap();
    for (bypass, forged) in [(false, false), (true, false), (false, true)] {
        let mut builder = DraftBuilder::new(planned.plan().view().callable(key).unwrap()).unwrap();
        let blocks = (0..3)
            .map(|_| builder.reserve_block().unwrap())
            .collect::<Vec<_>>();
        for block in &blocks {
            builder.define_block(*block, &[]).unwrap();
        }
        builder.set_entry(blocks[0]).unwrap();
        let inputs = builder.inputs().collect::<Vec<_>>();
        let edge = |target| Edge {
            target,
            arguments: vec![],
        };
        builder
            .terminate(
                blocks[0],
                Terminator::ScalarCheck {
                    relation: ScalarCheck::NonZeroDivisor {
                        ty: ScalarType::I64,
                        divisor: inputs[1],
                    },
                    success: edge(blocks[1]),
                    failure: edge(blocks[2]),
                },
            )
            .unwrap();
        let value = builder
            .append(
                blocks[1],
                Operation::Divide {
                    result: DivisionResult::Quotient,
                    dividend: inputs[0],
                    divisor: if forged { inputs[0] } else { inputs[1] },
                    evidence: ScalarDomainEvidence::SuccessCheck(blocks[0]),
                },
            )
            .unwrap()[0];
        builder
            .terminate(blocks[1], Terminator::Return(vec![value]))
            .unwrap();
        builder
            .terminate(
                blocks[2],
                if bypass {
                    Terminator::Jump(edge(blocks[1]))
                } else {
                    Terminator::Return(vec![inputs[0]])
                },
            )
            .unwrap();
        let result = verify_callable(builder.finish());
        if bypass || forged {
            assert!(result
                .err()
                .unwrap()
                .iter()
                .any(|f| f.reason == VerificationReason::GuardProtection));
        } else {
            assert!(result.is_ok());
        }
    }
}

#[test]
fn checked_float_ranges_reject_nan_infinities_and_upper_boundaries() {
    use Constant::*;
    let invalid = Err(FailureMessage::PrimitiveCastOutOfRange);
    let samples = [
        (0x7ff8_0000_0000_0042, [invalid, invalid, invalid]),
        (0x7ff0_0000_0000_0001, [invalid, invalid, invalid]),
        (f64::INFINITY.to_bits(), [invalid, invalid, invalid]),
        (f64::NEG_INFINITY.to_bits(), [invalid, invalid, invalid]),
        ((-0.0_f64).to_bits(), [Ok(I64(0)), Ok(U64(0)), Ok(U8(0))]),
        ((-0.5_f64).to_bits(), [Ok(I64(0)), Ok(U64(0)), Ok(U8(0))]),
        ((-1.0_f64).to_bits(), [Ok(I64(-1)), invalid, invalid]),
        (
            255.9_f64.to_bits(),
            [Ok(I64(255)), Ok(U64(255)), Ok(U8(255))],
        ),
        (256.0_f64.to_bits(), [Ok(I64(256)), Ok(U64(256)), invalid]),
        (
            (-9_223_372_036_854_775_808.0_f64).to_bits(),
            [Ok(I64(i64::MIN)), invalid, invalid],
        ),
        (
            0x43df_ffff_ffff_ffff,
            [
                Ok(I64(9_223_372_036_854_774_784)),
                Ok(U64(9_223_372_036_854_774_784)),
                invalid,
            ],
        ),
        (
            0x43e0_0000_0000_0000,
            [invalid, Ok(U64(9_223_372_036_854_775_808)), invalid],
        ),
        (
            0x43ef_ffff_ffff_ffff,
            [invalid, Ok(U64(18_446_744_073_709_549_568)), invalid],
        ),
        (0x43f0_0000_0000_0000, [invalid, invalid, invalid]),
    ];
    for (target, ty) in ["i64", "u64", "u8"].iter().enumerate() {
        let source = format!(
            "fn kernel() -> {ty} {{ return ({ty}) 0.5; }} fn main() -> i64 {{ return 0; }}"
        );
        let fixture = lower_source_to_complete_final_mir_with_sources("float-ranges.ska", &source);
        for (bits, expected) in samples {
            let mut program = fixture.mir.program().clone();
            let function = program.definitions.iter().next().unwrap().function;
            for block in &mut program
                .definitions
                .get_mut_for_test(function)
                .unwrap()
                .body
                .blocks
            {
                for instruction in &mut block.instructions {
                    if let crate::mir::MirInstruction::Assign(assign) = instruction {
                        if matches!(
                            assign.rvalue.kind,
                            crate::mir::MirRvalueKind::ConstantF64Bits(_)
                        ) {
                            assign.rvalue.kind = crate::mir::MirRvalueKind::ConstantF64Bits(bits);
                        }
                    }
                }
            }
            let sealed = crate::passes::verify_final_mir(program).unwrap();
            let planned = plan_program(BackendInput::without_runtime_trace(&sealed)).unwrap();
            let mut worklist = ProgramBuilder::new(planned.plan().view());
            let body = lower_next(&planned, &mut worklist).unwrap().unwrap();
            assert_eq!(oracle::run(&body), expected[target], "{ty} {bits:016x}");
        }
    }
}

#[test]
fn bit_reinterpretation_intrinsics_keep_payloads_and_need_no_range_guard() {
    let (_directory, graph) = crate::test_support::load_module_sources_with_standard_library("app", &[("app.ska", "import std::f64; fn from(bits: u64) -> f64 { return std::f64::from_bits(bits); } fn to(value: f64) -> u64 { return std::f64::to_bits(value); } fn main() -> i64 { var from_ptr: fn(u64) -> f64 = from; var to_ptr: fn(f64) -> u64 = to; return 0; }")]);
    let resolved = crate::resolve::resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let program = crate::test_support::lower_hir_to_final_mir(&checked.hir.unwrap());
    let verified = crate::passes::run_mir_pipeline(program).unwrap();
    let planned = plan_program(
        BackendInput::without_runtime_trace(&verified).with_reachable_artifacts_only(),
    )
    .unwrap();
    let mut conversions = 0;
    lower_program(&planned, |body| {
        let normalized_intrinsic = body
            .draft()
            .blocks()
            .flat_map(|(_, b)| &b.instructions)
            .any(|i| {
                matches!(
                    i.operation,
                    Operation::Convert {
                        conversion: Conversion::FloatBits,
                        ..
                    }
                )
            });
        if !normalized_intrinsic {
            return Ok(());
        }
        if body.draft().inputs().len() == 1 {
            for bits in [0, 1 << 63, 0x7ff8_0000_0000_0042, 0x7ff0_0000_0000_0001] {
                let (_, input) = body
                    .draft()
                    .values()
                    .find(|(id, _)| body.draft().inputs().contains(id))
                    .unwrap();
                let (input, expected) = if input.ty == ScalarType::U64 {
                    (Constant::U64(bits), Constant::F64(bits))
                } else {
                    (Constant::F64(bits), Constant::U64(bits))
                };
                assert_eq!(oracle::run_with_inputs(&body, &[input]), Ok(expected));
            }
        }
        for (_, block) in body.draft().blocks() {
            assert!(!matches!(
                block.terminator,
                Some(Terminator::ScalarCheck { .. })
            ));
            for instruction in &block.instructions {
                if let Operation::Convert {
                    conversion: Conversion::FloatBits,
                    evidence,
                    ..
                } = &instruction.operation
                {
                    assert!(evidence.is_none());
                    conversions += 1;
                }
            }
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(conversions, 2);
}

#[test]
fn guarded_numeric_values_in_loops_have_exact_single_definitions() {
    let fixture = lower_source_to_complete_final_mir_with_sources("numeric-loop.ska", "fn kernel(n: i64, count: u64, f: f64) -> i64 { var remaining: i64 = n; var result: i64 = 1; while (remaining > 0) { result = result / remaining; result = result << count; result = result + (i64) f; remaining = remaining - 1; } return result; } fn main() -> i64 { return 0; }");
    let planned = plan_program(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    let mut worklist = ProgramBuilder::new(planned.plan().view());
    let body = lower_next(&planned, &mut worklist).unwrap().unwrap();
    assert_eq!(
        body.draft()
            .blocks()
            .filter(|(_, b)| matches!(b.terminator, Some(Terminator::ScalarCheck { .. })))
            .count(),
        3
    );
    assert!(body
        .draft()
        .values()
        .all(|(_, value)| value.definition.is_some()));
    assert!(body.analysis().is_ok());
}
