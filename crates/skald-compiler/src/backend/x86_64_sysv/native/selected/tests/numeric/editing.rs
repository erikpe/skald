use super::*;

#[test]
fn same_typed_operand_substitution_cannot_change_guards_or_corrections() {
    for source in [
        "fn calculate(a:u64,b:u64)->u64 {return a << b;} fn main()->i64 {return 0;}",
        "fn calculate(a:i64,b:i64)->i64 {return a % b;} fn main()->i64 {return 0;}",
        "fn calculate(a:f64)->u64 {return (u64) a;} fn main()->i64 {return 0;}",
    ] {
        let mut tested = false;
        for_sources(source, |context, lower| {
            let body = select(context, lower).unwrap();
            let mut substitution = None;
            let mut dividend = None;
            let mut definitions = std::collections::BTreeMap::new();
            body.visit(|fact| {
                if let SelectedFact::Instruction {
                    block,
                    ordinal,
                    payload,
                } = fact
                {
                    for (value, definition) in payload.operands() {
                        if definition {
                            definitions.insert(value.value, (block, ordinal));
                        }
                    }
                    match payload.opcode() {
                        Opcode::Numeric(Numeric::Shift { input, count, .. }) => {
                            let (block, ordinal) = definitions[&count.value];
                            substitution = Some((block, ordinal, 0, input.value))
                        }
                        Opcode::Numeric(Numeric::Divide { low, .. }) => dividend = Some(low.value),
                        Opcode::Alu {
                            operation: lir::BinaryOperation::Add,
                            ..
                        } if dividend.is_some() => {
                            substitution = Some((block, ordinal, 1, dividend.unwrap()))
                        }
                        Opcode::Alu {
                            operation: lir::BinaryOperation::Xor,
                            left,
                            ..
                        } if source.contains("f64") => {
                            substitution = Some((block, ordinal, 1, left.value))
                        }
                        _ => {}
                    }
                }
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap();
            let Some((block, ordinal, slot, replacement)) = substitution else {
                return;
            };
            let verifier = Verifier::new(lower.receipt().owner().context().profile()).unwrap();
            let mut editor = body.into_editor();
            editor
                .replace_operand(
                    editor.block(block).unwrap(),
                    ordinal,
                    slot,
                    editor.value(replacement).unwrap(),
                )
                .unwrap();
            match editor.finish(&verifier) {
                Err(selected::SelectedEditFailure::Verify(failures)) => assert!(failures
                    .iter()
                    .any(|f| matches!(f.reason, selected::SelectedReason::Target(_)))),
                _ => panic!("numeric association must reject same-typed substitution"),
            }
            tested = true;
        });
        assert!(tested, "{source}");
    }
}

#[test]
fn swapped_domain_and_overflow_edges_cannot_publish() {
    let mut tested = 0;
    for_sources(
        "fn calculate(a:i64,b:i64)->i64 {return a / b;} fn main()->i64 {return 0;}",
        |context, lower| {
            for overflow in [false, true] {
                let body = select(context, lower).unwrap();
                let mut chosen = None;
                let mut overflow_block = None;
                body.visit(|fact| {
                    if let SelectedFact::Instruction { payload, .. } = fact {
                        if let Opcode::Numeric(Numeric::Divide { overflow, .. }) = payload.opcode()
                        {
                            overflow_block = *overflow;
                        }
                    }
                    Ok::<_, std::convert::Infallible>(())
                })
                .unwrap();
                body.visit(|fact| {
                    if let SelectedFact::Terminal {
                        block,
                        payload: Some(node),
                        edges,
                    } = fact
                    {
                        if (!overflow && matches!(node.opcode(), Opcode::CheckBranch { .. }))
                            || (overflow && Some(block) == overflow_block)
                        {
                            chosen = Some((block, edges.to_vec()));
                        }
                    }
                    Ok::<_, std::convert::Infallible>(())
                })
                .unwrap();
                let Some((block, edges)) = chosen else {
                    continue;
                };
                let verifier = Verifier::new(lower.receipt().owner().context().profile()).unwrap();
                let mut editor = body.into_editor();
                for slot in 0..2 {
                    let (to, args) = &edges[1 - slot];
                    let args = args
                        .iter()
                        .map(|v| editor.value(*v).unwrap())
                        .collect::<Vec<_>>();
                    editor
                        .redirect_edge(
                            editor.block(block).unwrap(),
                            slot,
                            editor.block(*to).unwrap(),
                            &args,
                        )
                        .unwrap();
                }
                assert!(matches!(
                    editor.finish(&verifier),
                    Err(selected::SelectedEditFailure::Verify(_))
                ));
                tested += 1;
            }
        },
    );
    assert_eq!(tested, 2);
}

#[test]
fn numeric_rebuild_and_split_preserve_secured_metadata_and_results() {
    for source in [
        "fn calculate(a:i64,b:i64)->i64 {return a / b;} fn main()->i64 {return 0;}",
        "fn calculate(a:f64)->u64 {return (u64) a;} fn main()->i64 {return 0;}",
    ] {
        let mut tested = false;
        for_sources(source, |context, lower| {
            let body = select(context, lower).unwrap();
            let mut arity = 0;
            let mut split = None;
            body.visit(|fact| {
                match fact {
                    SelectedFact::Entry { inputs, .. } => arity = inputs.len(),
                    SelectedFact::Instruction {
                        block,
                        ordinal,
                        payload,
                    } if matches!(payload.opcode(), Opcode::Numeric(_)) => {
                        split = Some((block, ordinal, payload.origin))
                    }
                    _ => {}
                }
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap();
            if arity == 0 {
                return;
            }
            let input = if arity == 2 {
                vec![Value::Bits(-7i64 as u64), Value::Bits(3)]
            } else {
                vec![Value::Float(9223372036854777856.0f64.to_bits())]
            };
            let expected = run(&body, &input);
            let verifier = Verifier::new(lower.receipt().owner().context().profile()).unwrap();
            let mut editor = body.into_editor();
            let (block, at, origin) = split.unwrap();
            editor
                .split_block(
                    editor.block(block).unwrap(),
                    at,
                    Instruction::new(Opcode::Jump, origin, &verifier.resources),
                )
                .unwrap();
            let mut values = editor.values().collect::<Vec<_>>();
            values.reverse();
            let mut blocks = editor.blocks().collect::<Vec<_>>();
            blocks.reverse();
            let mut objects = editor.objects().collect::<Vec<_>>();
            objects.reverse();
            let (editor, _) = editor.rebuild(&values, &blocks, &objects).unwrap();
            let fresh = editor.finish(&verifier).unwrap();
            assert_eq!(run(&fresh, &input), expected);
            tested = true;
        });
        assert!(tested);
    }
}

#[test]
fn reporter_arguments_cannot_be_replaced_by_unrelated_live_values() {
    let mut tested = false;
    for_sources(
        "fn calculate(a:u64,b:u64)->u64 {return a << b;} fn main()->i64 {return 0;}",
        |context, lower| {
            let body = select(context, lower).unwrap();
            let mut replacement = None;
            let mut reporter = None;
            body.visit(|fact| {
                match fact {
                    SelectedFact::Entry { inputs, .. } => replacement = inputs.first().copied(),
                    SelectedFact::Terminal {
                        block,
                        payload: Some(node),
                        ..
                    } if matches!(node.opcode(), Opcode::Failure { .. }) => reporter = Some(block),
                    _ => {}
                }
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap();
            let Some(block) = reporter else {
                return;
            };
            let verifier = Verifier::new(lower.receipt().owner().context().profile()).unwrap();
            let mut editor = body.into_editor();
            editor
                .replace_terminal_operand(
                    editor.block(block).unwrap(),
                    1,
                    editor.value(replacement.unwrap()).unwrap(),
                )
                .unwrap();
            match editor.finish(&verifier) {
                Err(selected::SelectedEditFailure::Verify(failures)) => {
                    assert!(failures.iter().any(|f| matches!(
                        f.reason,
                        selected::SelectedReason::Target(
                            "native failure message association mismatch"
                        )
                    )))
                }
                _ => panic!("reporter length must match its declared message"),
            }
            tested = true;
        },
    );
    assert!(tested);
}
