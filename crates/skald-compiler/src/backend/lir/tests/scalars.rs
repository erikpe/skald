use super::*;

#[test]
fn constants_preserve_canonical_bytes_booleans_and_all_binary64_bits() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    let bits = [
        0,
        0x8000_0000_0000_0000,
        0x7ff8_0000_0000_0001,
        0xfff8_0000_0000_0042,
        0x7ff0_0000_0000_0001,
    ];
    for bits in bits {
        constant(&mut builder, entry, Constant::F64(bits));
    }
    for value in [
        Constant::U8(0),
        Constant::U8(255),
        Constant::Bool(false),
        Constant::Bool(true),
        Constant::I64(i64::MIN),
        Constant::U64(u64::MAX),
    ] {
        constant(&mut builder, entry, value);
    }
    assert_eq!(
        err(builder.append(entry, Operation::Constant(Constant::Null(ScalarType::U64)))),
        BuildError::InvalidScalar
    );
    let draft = builder.finish();
    let actual = draft
        .block(entry)
        .unwrap()
        .instructions
        .iter()
        .take(5)
        .map(|instruction| match instruction.operation {
            Operation::Constant(Constant::F64(bits)) => bits,
            _ => panic!("expected binary64 bits"),
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, bits);
    assert_ne!(Constant::F64(bits[0]), Constant::F64(bits[1]));
    assert_ne!(Constant::F64(bits[2]), Constant::F64(bits[3]));
    assert_eq!(Constant::U8(255).scalar_type(), Ok(ScalarType::U8));
    assert_eq!(Constant::Bool(true).scalar_type(), Ok(ScalarType::Bool));
}

#[test]
fn scalar_schemas_accept_wrapping_arithmetic_and_reject_mixed_domains() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    for literal in [
        Constant::I64(i64::MIN),
        Constant::U64(u64::MAX),
        Constant::U8(255),
    ] {
        let value = constant(&mut builder, entry, literal);
        for operation in [UnaryOperation::Negate, UnaryOperation::Complement] {
            builder
                .append(entry, Operation::Unary { operation, value })
                .unwrap();
        }
        for operation in [
            BinaryOperation::Add,
            BinaryOperation::Subtract,
            BinaryOperation::Multiply,
            BinaryOperation::And,
            BinaryOperation::Or,
            BinaryOperation::Xor,
        ] {
            builder
                .append(
                    entry,
                    Operation::Binary {
                        operation,
                        left: value,
                        right: value,
                    },
                )
                .unwrap();
        }
        for result in [DivisionResult::Quotient, DivisionResult::Remainder] {
            builder
                .append(
                    entry,
                    Operation::Divide {
                        result,
                        dividend: value,
                        divisor: value,
                        evidence: ScalarDomainEvidence::ExactConstant(value),
                    },
                )
                .unwrap();
        }
    }
    let signed = constant(&mut builder, entry, Constant::I64(i64::MIN));
    let minus_one = constant(&mut builder, entry, Constant::I64(-1));
    builder
        .append(
            entry,
            Operation::Divide {
                result: DivisionResult::Quotient,
                dividend: signed,
                divisor: minus_one,
                evidence: ScalarDomainEvidence::ExactConstant(minus_one),
            },
        )
        .unwrap();
    let float = constant(&mut builder, entry, Constant::F64(1.0f64.to_bits()));
    let boolean = constant(&mut builder, entry, Constant::Bool(true));
    for operation in [
        BinaryOperation::Add,
        BinaryOperation::Subtract,
        BinaryOperation::Multiply,
        BinaryOperation::FloatDivide,
    ] {
        builder
            .append(
                entry,
                Operation::Binary {
                    operation,
                    left: float,
                    right: float,
                },
            )
            .unwrap();
    }
    builder
        .append(
            entry,
            Operation::Unary {
                operation: UnaryOperation::Negate,
                value: float,
            },
        )
        .unwrap();
    builder
        .append(
            entry,
            Operation::Unary {
                operation: UnaryOperation::LogicalNot,
                value: boolean,
            },
        )
        .unwrap();
    assert_eq!(
        err(builder.append(
            entry,
            Operation::Binary {
                operation: BinaryOperation::Add,
                left: signed,
                right: float
            }
        )),
        BuildError::InvalidScalar
    );
    assert_eq!(
        err(builder.append(
            entry,
            Operation::Unary {
                operation: UnaryOperation::Complement,
                value: float
            }
        )),
        BuildError::InvalidScalar
    );
    for operation in [
        BinaryOperation::And,
        BinaryOperation::Or,
        BinaryOperation::Xor,
    ] {
        builder
            .append(
                entry,
                Operation::Binary {
                    operation,
                    left: boolean,
                    right: boolean,
                },
            )
            .unwrap();
    }
    // This layer records the wrapping/floor operation, including the overflow
    // pair; execution recipe equivalence remains a native target obligation.
    let draft = builder.finish();
    assert!(draft.values().all(|(_, value)| value.definition.is_some()));
}

#[test]
fn all_primitive_cast_cells_and_bit_conversions_have_distinct_schemas() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    let types = [
        ScalarType::I64,
        ScalarType::U64,
        ScalarType::U8,
        ScalarType::F64,
        ScalarType::Bool,
    ];
    for literal in [
        Constant::I64(-1),
        Constant::U64(1),
        Constant::U8(1),
        Constant::F64(0.5f64.to_bits()),
        Constant::Bool(true),
    ] {
        let value = constant(&mut builder, entry, literal);
        let from = literal.scalar_type().unwrap();
        for target in types {
            let conversion = if from == target {
                Conversion::Identity
            } else if target == ScalarType::Bool {
                Conversion::ToBoolean
            } else if target == ScalarType::F64 {
                Conversion::ToFloat
            } else if from == ScalarType::Bool {
                Conversion::FromBoolean
            } else if from == ScalarType::F64 {
                Conversion::TruncateFloat
            } else {
                Conversion::IntegerBits
            };
            let evidence = (conversion == Conversion::TruncateFloat)
                .then_some(ScalarDomainEvidence::ExactConstant(value));
            builder
                .append(
                    entry,
                    Operation::Convert {
                        conversion,
                        value,
                        target,
                        evidence,
                    },
                )
                .unwrap();
        }
    }
    let bits = constant(&mut builder, entry, Constant::U64(0x8000_0000_0000_0000));
    let float = builder
        .append(
            entry,
            Operation::Convert {
                conversion: Conversion::FloatBits,
                value: bits,
                target: ScalarType::F64,
                evidence: None,
            },
        )
        .unwrap()[0];
    builder
        .append(
            entry,
            Operation::Convert {
                conversion: Conversion::FloatBits,
                value: float,
                target: ScalarType::U64,
                evidence: None,
            },
        )
        .unwrap();
    assert_eq!(
        err(builder.append(
            entry,
            Operation::Convert {
                conversion: Conversion::TruncateFloat,
                value: float,
                target: ScalarType::I64,
                evidence: None
            }
        )),
        BuildError::InvalidEvidence
    );
    assert_eq!(
        err(builder.append(
            entry,
            Operation::Convert {
                conversion: Conversion::IntegerBits,
                value: float,
                target: ScalarType::I64,
                evidence: None
            }
        )),
        BuildError::InvalidScalar
    );
    assert!(!Conversion::ToBoolean.accepts(ScalarType::DataAddress, ScalarType::Bool));
    assert!(!Conversion::FloatBits.accepts(ScalarType::F64, ScalarType::I64));
    assert_eq!(builder.finish().blocks().len(), 1);
}

#[test]
fn comparisons_and_shifts_are_typed_without_importing_execution_enums() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let signature = plan.view().callables().next().unwrap().signature;
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    let count = constant(&mut builder, entry, Constant::U64(7));
    for literal in [
        Constant::I64(-1),
        Constant::U64(1),
        Constant::U8(1),
        Constant::F64(0x7ff8_0000_0000_0001),
        Constant::Bool(true),
        Constant::Null(ScalarType::DataAddress),
        Constant::Null(ScalarType::CodeAddress(signature)),
    ] {
        let value = constant(&mut builder, entry, literal);
        let ty = literal.scalar_type().unwrap();
        for predicate in [
            Predicate::Equal,
            Predicate::NotEqual,
            Predicate::LessThan,
            Predicate::LessEqual,
            Predicate::GreaterThan,
            Predicate::GreaterEqual,
        ] {
            let result = builder.append(
                entry,
                Operation::Compare {
                    predicate,
                    left: value,
                    right: value,
                },
            );
            if matches!(
                ty,
                ScalarType::Bool | ScalarType::DataAddress | ScalarType::CodeAddress(_)
            ) && !predicate.is_equality()
            {
                assert_eq!(err(result), BuildError::InvalidScalar);
            } else {
                assert_eq!(result.unwrap().len(), 1);
            }
        }
        for direction in [
            ShiftDirection::Left,
            ShiftDirection::ArithmeticRight,
            ShiftDirection::LogicalRight,
        ] {
            let result = builder.append(
                entry,
                Operation::Shift {
                    direction,
                    value,
                    count,
                    evidence: ScalarDomainEvidence::ExactConstant(count),
                },
            );
            if matches!(ty, ScalarType::I64 | ScalarType::U64 | ScalarType::U8) {
                result.unwrap();
            } else {
                assert_eq!(err(result), BuildError::InvalidScalar);
            }
        }
    }
    let boolean = constant(&mut builder, entry, Constant::Bool(false));
    assert_eq!(
        err(builder.append(
            entry,
            Operation::Shift {
                direction: ShiftDirection::Left,
                value: count,
                count: boolean,
                evidence: ScalarDomainEvidence::ExactConstant(boolean)
            }
        )),
        BuildError::InvalidScalar
    );
    let pointer = constant(&mut builder, entry, Constant::Null(ScalarType::DataAddress));
    assert_eq!(
        err(builder.append(
            entry,
            Operation::Compare {
                predicate: Predicate::Equal,
                left: pointer,
                right: count
            }
        )),
        BuildError::InvalidScalar
    );
    let bits = builder
        .append(
            entry,
            Operation::Convert {
                conversion: Conversion::PointerBits,
                value: pointer,
                target: ScalarType::U64,
                evidence: None,
            },
        )
        .unwrap()[0];
    builder
        .append(
            entry,
            Operation::Convert {
                conversion: Conversion::PointerBits,
                value: bits,
                target: ScalarType::CodeAddress(signature),
                evidence: None,
            },
        )
        .unwrap();
}

#[test]
fn scalar_checks_and_evidence_are_explicit_unverified_obligations() {
    let plan = CheckedPlan::check(facts()).unwrap();
    for case in 0..3 {
        let mut builder = builder(&plan);
        let entry = entry_block(&mut builder);
        let success = block(&mut builder);
        let failure = block(&mut builder);
        let literal = match case {
            0 => Constant::I64(-1),
            1 => Constant::U64(7),
            _ => Constant::F64(255.9f64.to_bits()),
        };
        let secured = constant(&mut builder, entry, literal);
        let relation = match case {
            0 => ScalarCheck::NonZeroDivisor {
                ty: ScalarType::I64,
                divisor: secured,
            },
            1 => ScalarCheck::ShiftCountBelowWidth {
                count: secured,
                width: 8,
            },
            _ => ScalarCheck::FiniteTruncatedF64InIntegerRange {
                source: secured,
                target: ScalarType::U8,
            },
        };
        builder
            .terminate(
                entry,
                Terminator::ScalarCheck {
                    relation,
                    success: empty_edge(success),
                    failure: empty_edge(failure),
                },
            )
            .unwrap();
        let evidence = ScalarDomainEvidence::SuccessCheck(entry);
        let operation = match case {
            0 => Operation::Divide {
                result: DivisionResult::Remainder,
                dividend: secured,
                divisor: secured,
                evidence,
            },
            1 => {
                let value = constant(&mut builder, success, Constant::U8(255));
                Operation::Shift {
                    direction: ShiftDirection::LogicalRight,
                    value,
                    count: secured,
                    evidence,
                }
            }
            _ => Operation::Convert {
                conversion: Conversion::TruncateFloat,
                value: secured,
                target: ScalarType::U8,
                evidence: Some(evidence),
            },
        };
        builder.append(success, operation).unwrap();
        builder
            .terminate(success, Terminator::Return(vec![]))
            .unwrap();
        builder.terminate(failure, Terminator::HardTrap).unwrap();
        let draft = builder.finish();
        let record: &Block = draft.block(entry).unwrap();
        assert_eq!(
            record
                .terminator
                .as_ref()
                .unwrap()
                .edges(entry.id())
                .count(),
            2
        );
        match record.terminator.as_ref().unwrap() {
            Terminator::ScalarCheck {
                relation: ScalarCheck::NonZeroDivisor { divisor, .. },
                ..
            } => assert_eq!(*divisor, secured.id()),
            Terminator::ScalarCheck {
                relation: ScalarCheck::ShiftCountBelowWidth { count, width },
                ..
            } => {
                assert_eq!(*count, secured.id());
                assert_eq!(*width, 8);
            }
            Terminator::ScalarCheck {
                relation: ScalarCheck::FiniteTruncatedF64InIntegerRange { source, target },
                ..
            } => {
                assert_eq!(*source, secured.id());
                assert_eq!(*target, ScalarType::U8);
            }
            _ => panic!("expected scalar check"),
        }
    }
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    let success = block(&mut builder);
    let failure = block(&mut builder);
    let zero = constant(&mut builder, entry, Constant::U64(0));
    assert_eq!(
        err(builder.terminate(
            entry,
            Terminator::ScalarCheck {
                relation: ScalarCheck::ShiftCountBelowWidth {
                    count: zero,
                    width: 32
                },
                success: empty_edge(success),
                failure: empty_edge(failure)
            }
        )),
        BuildError::InvalidScalar
    );
    // Construction deliberately does not certify false constant evidence.
    builder
        .append(
            entry,
            Operation::Divide {
                result: DivisionResult::Quotient,
                dividend: zero,
                divisor: zero,
                evidence: ScalarDomainEvidence::ExactConstant(zero),
            },
        )
        .unwrap();
    assert!(builder.finish().block(entry).unwrap().terminator.is_none());
}
