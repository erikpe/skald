use super::*;

#[test]
fn guarded_numeric_source_graphs_publish_concrete_cells_and_correction_blocks() {
    let sources = [
        "fn divide(a:i64,b:i64)->i64 {return a / b;} fn main()->i64 {return 0;}",
        "fn remainder(a:i64,b:i64)->i64 {return a % b;} fn main()->i64 {return 0;}",
        "fn divide(a:u8,b:u8)->u8 {return a / b;} fn main()->i64 {return 0;}",
        "fn shift(a:i64,b:u64)->i64 {return a >> b;} fn main()->i64 {return 0;}",
        "fn cast(a:f64)->u64 {return (u64) a;} fn main()->i64 {return 0;}",
        "fn cast(a:u64)->f64 {return (f64) a;} fn main()->i64 {return 0;}",
    ];
    for source in sources {
        for_sources(source, |context, lower| {
            let body = select(context, lower).unwrap();
            assert!(body.analysis().is_ok());
        });
    }
}

#[test]
fn all_primitive_cast_cells_match_independent_boundary_references() {
    let floats = [
        -0.0,
        0.0,
        -0.5,
        -1.0,
        1.75,
        255.75,
        256.0,
        -9223372036854775808.0,
        f64::from_bits((-9223372036854775808.0f64).to_bits() + 1),
        9223372036854775808.0,
        f64::from_bits(9223372036854775808.0f64.to_bits() - 1),
        f64::from_bits(18446744073709551616.0f64.to_bits() - 1),
        18446744073709551616.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::from_bits(0x7ff8000000000123),
        f64::from_bits(0x7ff0000000000123),
    ];
    for from in ["i64", "u64", "u8", "bool", "f64"] {
        let inputs = match from {
            "f64" => floats
                .iter()
                .map(|n| Value::Float(n.to_bits()))
                .collect::<Vec<_>>(),
            "bool" => vec![Value::Bits(0), Value::Bits(1)],
            "u8" => [0, 1, 127, 128, 255].into_iter().map(Value::Bits).collect(),
            _ => [
                0,
                1,
                255,
                256,
                (1 << 53) + 1,
                (1 << 63) - 1,
                1 << 63,
                (1 << 63) + 1023,
                (1 << 63) + 1024,
                (1 << 63) + 1025,
                u64::MAX - 1024,
                u64::MAX,
            ]
            .into_iter()
            .map(Value::Bits)
            .collect(),
        };
        for to in ["i64", "u64", "u8", "bool", "f64"] {
            let cases = inputs
                .iter()
                .map(|input| {
                    let expected = match (*input, to) {
                        (Value::Float(bits), "f64") => Ok(Value::Float(bits)),
                        (Value::Float(bits), "bool") => {
                            Ok(Value::Bits(u64::from(f64::from_bits(bits) != 0.0)))
                        }
                        (Value::Float(bits), _) => {
                            let n = Binary64::from_bits(bits);
                            let result = match to {
                                "i64" => match n.truncating_to_i64() {
                                    IntegerConversion::Value(v) => Some(v as u64),
                                    _ => None,
                                },
                                "u64" => match n.truncating_to_u64() {
                                    IntegerConversion::Value(v) => Some(v),
                                    _ => None,
                                },
                                "u8" => match n.truncating_to_u8() {
                                    IntegerConversion::Value(v) => Some(u64::from(v)),
                                    _ => None,
                                },
                                _ => unreachable!(),
                            };
                            result
                                .map(Value::Bits)
                                .ok_or(FailureMessage::PrimitiveCastOutOfRange)
                        }
                        (Value::Bits(n), "f64") => Ok(Value::Float(
                            (if from == "i64" {
                                (n as i64) as f64
                            } else {
                                n as f64
                            })
                            .to_bits(),
                        )),
                        (Value::Bits(n), "bool") => Ok(Value::Bits(u64::from(n != 0))),
                        (Value::Bits(n), "u8") => Ok(Value::Bits(n & 255)),
                        (Value::Bits(n), _) => Ok(Value::Bits(n)),
                        _ => unreachable!(),
                    };
                    (vec![*input], expected)
                })
                .collect::<Vec<_>>();
            evaluate(&format!("fn convert(value:{from})->{to} {{return ({to}) value;}} fn main()->i64 {{return 0;}}"), &cases);
        }
    }
}

#[test]
fn division_correction_matches_floor_semantics_and_never_executes_overflow() {
    for ty in ["i64", "u64", "u8"] {
        for op in ["/", "%"] {
            let pairs = if ty == "i64" {
                vec![
                    (-7i64 as u64, 3),
                    (7, -3i64 as u64),
                    (-7i64 as u64, -3i64 as u64),
                    (i64::MIN as u64, u64::MAX),
                    (i64::MIN as u64, 1),
                    (i64::MAX as u64, 2),
                    (1, 0),
                ]
            } else if ty == "u8" {
                vec![(255, 2), (127, 255), (1, 0)]
            } else {
                vec![(u64::MAX, 2), (1 << 63, 3), (1, 0)]
            };
            let cases = pairs
                .into_iter()
                .map(|(a, b)| {
                    let expected = if b == 0 {
                        Err(if op == "/" {
                            FailureMessage::IntegerDivisionByZero
                        } else {
                            FailureMessage::IntegerRemainderByZero
                        })
                    } else {
                        let answer = if ty == "i64" {
                            let (a, b) = (a as i64, b as i64);
                            if a == i64::MIN && b == -1 {
                                if op == "/" {
                                    i64::MIN as u64
                                } else {
                                    0
                                }
                            } else {
                                let mut q = a / b;
                                let mut r = a % b;
                                if r != 0 && (a < 0) != (b < 0) {
                                    q -= 1;
                                    r += b;
                                }
                                if op == "/" {
                                    q as u64
                                } else {
                                    r as u64
                                }
                            }
                        } else if op == "/" {
                            a / b
                        } else {
                            a % b
                        };
                        Ok(Value::Bits(answer))
                    };
                    (vec![Value::Bits(a), Value::Bits(b)], expected)
                })
                .collect::<Vec<_>>();
            evaluate(&format!("fn calculate(a:{ty},b:{ty})->{ty} {{return a {op} b;}} fn main()->i64 {{return 0;}}"),&cases);
        }
    }
    evaluate(
        "fn calculate(a:i64,b:i64)->i64 {return a/b + a%b + a;} fn main()->i64 {return 0;}",
        &[(
            vec![Value::Bits(-7i64 as u64), Value::Bits(3)],
            Ok(Value::Bits(-8i64 as u64)),
        )],
    );
}

#[test]
fn cl_shifts_guard_full_counts_and_preserve_live_inputs() {
    for ty in ["i64", "u64", "u8"] {
        let width = if ty == "u8" { 8 } else { 64 };
        let input = if ty == "u8" { 255 } else { u64::MAX };
        for op in ["<<", ">>"] {
            let cases = [0, width - 1, width, width + 256, u64::MAX]
                .into_iter()
                .map(|count| {
                    let expected = if count >= width {
                        Err(FailureMessage::ShiftCountOutOfRange)
                    } else {
                        let n = if op == "<<" {
                            input.wrapping_shl(count as u32)
                        } else if ty == "i64" {
                            ((input as i64) >> count) as u64
                        } else {
                            input >> count
                        };
                        Ok(Value::Bits(if width == 8 { n & 255 } else { n }))
                    };
                    (vec![Value::Bits(input), Value::Bits(count)], expected)
                })
                .collect::<Vec<_>>();
            evaluate(&format!("fn calculate(a:{ty},b:u64)->{ty} {{return a {op} b;}} fn main()->i64 {{return 0;}}"),&cases);
        }
    }
    evaluate(
        "fn calculate(a:u64,b:u64)->u64 {return (a << b) + a;} fn main()->i64 {return 0;}",
        &[(vec![Value::Bits(3), Value::Bits(2)], Ok(Value::Bits(15)))],
    );
}

#[test]
fn normalized_bit_intrinsics_preserve_nan_payloads_and_signed_zero() {
    let (_directory, graph) = crate::test_support::load_module_sources_with_standard_library("app", &[("app.ska", "import std::f64; fn from(bits:u64)->f64 {return std::f64::from_bits(bits);} fn to(value:f64)->u64 {return std::f64::to_bits(value);} fn main()->i64 {var from_ptr:fn(u64)->f64=from; var to_ptr:fn(f64)->u64=to; return 0;}")]);
    let resolved = crate::resolve::resolve_module_graph(&graph);
    assert!(resolved.diagnostics.is_empty());
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty());
    let mir = crate::test_support::lower_hir_to_final_mir(&checked.hir.unwrap());
    let mir = crate::passes::run_mir_pipeline(mir).unwrap();
    let planned = plan_program(
        crate::backend::BackendInput::without_runtime_trace(&mir).with_reachable_artifacts_only(),
    )
    .unwrap();
    let catalog = TargetDeclarations::new(planned.plan().view())
        .freeze()
        .unwrap();
    let context = selection_context(&catalog).unwrap();
    let mut tested = 0;
    crate::backend::lowering::lower_program(&planned, |lower| {
        let bit_conversion = lower
            .draft()
            .blocks()
            .flat_map(|(_, b)| &b.instructions)
            .find_map(|i| {
                if let lir::Operation::Convert {
                    conversion: lir::Conversion::FloatBits,
                    target,
                    ..
                } = i.operation
                {
                    Some(target)
                } else {
                    None
                }
            });
        let Some(target) = bit_conversion else {
            return Ok(());
        };
        let body = select(&context, &lower).unwrap();
        crate::backend::x86_64_sysv::native::place_native_baseline(&body).unwrap();
        for bits in [0, 1 << 63, 0x7ff8000000000042, 0x7ff0000000000001, u64::MAX] {
            let (input, output) = if target == plan::ScalarType::F64 {
                (Value::Bits(bits), Value::Float(bits))
            } else {
                (Value::Float(bits), Value::Bits(bits))
            };
            assert_eq!(run(&body, &[input]), Ok(output));
        }
        tested += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(tested, 2);
}

#[test]
fn correction_joins_before_guards_and_inside_loops_keep_exact_domains() {
    evaluate(
        "fn calculate(a:f64,b:f64)->u64 {return (u64) a / (u64) b;} fn main()->i64 {return 0;}",
        &[
            (
                vec![
                    Value::Float(9223372036854775808.0f64.to_bits()),
                    Value::Float(2.0f64.to_bits()),
                ],
                Ok(Value::Bits(1 << 62)),
            ),
            (
                vec![
                    Value::Float(7.0f64.to_bits()),
                    Value::Float(0.0f64.to_bits()),
                ],
                Err(FailureMessage::IntegerDivisionByZero),
            ),
        ],
    );
    evaluate("fn calculate(n:i64,count:u64,f:f64)->i64 {var remaining:i64=n; var result:i64=1; while (remaining > 0) {result=result / remaining; result=result << count; result=result + (i64) f; remaining=remaining - 1;} return result;} fn main()->i64 {return 0;}", &[
        (vec![Value::Bits(3),Value::Bits(1),Value::Float(2.0f64.to_bits())],Ok(Value::Bits(10))),
        (vec![Value::Bits(1),Value::Bits(256),Value::Float(2.0f64.to_bits())],Err(FailureMessage::ShiftCountOutOfRange)),
    ]);
}
