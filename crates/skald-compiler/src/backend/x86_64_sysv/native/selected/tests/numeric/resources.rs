use super::*;

#[test]
fn numeric_fixed_events_exclude_divisor_aliases_and_declare_call_footprints() {
    use crate::backend::x86_64_sysv::native::Gpr;
    let mut covered = [false; 3];
    for_sources(
        "fn calculate(a:i64,b:i64,c:u64)->i64 {return (a / b) >> c;} fn main()->i64 {return 0;}",
        |context, lower| {
            let body = select(context, lower).unwrap();
            let verifier = Verifier::new(lower.receipt().owner().context().profile()).unwrap();
            body.visit(|fact| {
                let (node, terminal) = match fact {
                    SelectedFact::Instruction { payload, .. } => (payload, false),
                    SelectedFact::Terminal {
                        payload: Some(payload),
                        ..
                    } => (payload, true),
                    _ => return Ok::<_, std::convert::Infallible>(()),
                };
                let description = node.describe();
                match node.opcode() {
                    Opcode::Numeric(Numeric::Divide { .. }) => {
                        for (slot, gpr) in
                            [(0, Gpr::Rax), (1, Gpr::Rdx), (3, Gpr::Rax), (4, Gpr::Rdx)]
                        {
                            assert!(matches!(description.operands[slot].constraint,
                                Constraint::Fixed(view) if view == verifier.resources.gpr(gpr,64).unwrap()));
                            assert_eq!(description.operands[slot].timing, Timing::Late);
                        }
                        let Constraint::Resources { views, memory } =
                            description.operands[2].constraint
                        else {
                            panic!("divisor needs nonaliasing choices")
                        };
                        assert!(!memory);
                        for gpr in [Gpr::Rax, Gpr::Rdx, Gpr::Rsp, Gpr::Rbp] {
                            assert!(!views.contains(&verifier.resources.gpr(gpr, 64).unwrap()));
                        }
                        covered[0] = true;
                    }
                    Opcode::Numeric(Numeric::Shift { input, out, .. }) => {
                        assert_ne!(input.value, out.value);
                        assert!(matches!(description.operands[1].constraint,
                            Constraint::Fixed(view) if view == verifier.resources.gpr(Gpr::Rcx,8).unwrap()));
                        assert_eq!(
                            (description.ties[0].input, description.ties[0].output),
                            (0, 2)
                        );
                        covered[1] = true;
                    }
                    Opcode::Failure { .. } => {
                        assert!(matches!(description.bundle, Some(Bundle::Atomic)));
                        assert_eq!(*description.effects, plan::service_effects(plan::RuntimeService::Panic));
                        assert_eq!(
                            description.clobbers.len(),
                            verifier.resources.caller_clobbers().len()
                        );
                        let mut invalid = node.clone();
                        invalid.clobbers.clear();
                        assert!(verifier.verify_payload(&invalid, terminal).is_err());
                        let mut invalid = node.clone();
                        invalid.effects = Effects::default();
                        assert!(verifier.verify_payload(&invalid, terminal).is_err());
                        covered[2] = true;
                    }
                    _ => {}
                }
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap();
        },
    );
    assert!(covered.into_iter().all(|v| v));
}

#[test]
fn malformed_numeric_cells_have_total_descriptors_and_reject_wrong_banks() {
    let mut tested = 0;
    for source in [
        "fn calculate(a:i64,b:i64)->i64 {return a / b;} fn main()->i64 {return 0;}",
        "fn calculate(a:u64,b:u64)->u64 {return a << b;} fn main()->i64 {return 0;}",
        "fn calculate(a:f64)->u8 {return (u8) a;} fn main()->i64 {return 0;}",
    ] {
        for_sources(source, |context, lower| {
            let body = select(context, lower).unwrap();
            let verifier = Verifier::new(lower.receipt().owner().context().profile()).unwrap();
            body.visit(|fact| {
                let SelectedFact::Instruction { payload, .. } = fact else {
                    return Ok::<_, std::convert::Infallible>(());
                };
                let mut malformed = payload.clone();
                let wrong = match &mut malformed.opcode {
                    Opcode::Numeric(
                        Numeric::Cell { out, .. }
                        | Numeric::Dividend { high: out, .. }
                        | Numeric::Divide { remainder: out, .. }
                        | Numeric::Shift { out, .. },
                    ) => out,
                    Opcode::Numeric(Numeric::CheckedTruncate { input, .. }) => input,
                    _ => return Ok(()),
                };
                wrong.representation = selected::Representation::new(
                    if wrong.representation.kind == selected::RepresentationKind::Float {
                        selected::RepresentationKind::Bits
                    } else {
                        selected::RepresentationKind::Float
                    },
                    64,
                )
                .unwrap();
                malformed.refresh();
                assert!(std::panic::catch_unwind(|| malformed.describe()).is_ok());
                assert!(verifier.verify_payload(&malformed, false).is_err());
                let mut effects = payload.clone();
                effects.effects = Effects::new([crate::backend::effects::Effect::Call]);
                assert!(verifier.verify_payload(&effects, false).is_err());
                tested += 1;
                Ok(())
            })
            .unwrap();
        });
    }
    assert!(tested >= 5);
}
