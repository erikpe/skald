use super::*;

#[test]
fn nan_float_predicates_and_flag_events_have_explicit_atomic_native_recipes() {
    use crate::primitive_comparison::PrimitiveComparisonPredicate::*;
    let source = concat!(
        "fn eq(a: f64, b: f64) -> bool { return a == b; } ",
        "fn ne(a: f64, b: f64) -> bool { return a != b; } ",
        "fn lt(a: f64, b: f64) -> bool { return a < b; } ",
        "fn le(a: f64, b: f64) -> bool { return a <= b; } ",
        "fn gt(a: f64, b: f64) -> bool { return a > b; } ",
        "fn ge(a: f64, b: f64) -> bool { return a >= b; } ",
        "fn main() -> i64 { return 0; }",
    );
    let mut predicates = vec![];
    for_sources(source, |context, lower| {
        let body = select(context, lower).unwrap();
        body.visit(|fact| {
            if let SelectedFact::Instruction { payload, .. } = fact {
                if let Opcode::FloatCompare {
                    condition,
                    predicate,
                    ..
                } = payload.opcode
                {
                    predicates.push(predicate);
                    // Independent UCOMISD flag model, including unordered inputs.
                    let evaluate = |a: f64, b: f64| {
                        let unordered = a.is_nan() || b.is_nan();
                        let zf = unordered || a == b;
                        let cf = unordered || a < b;
                        let pf = unordered;
                        match condition {
                            model::FloatCondition::EqualOrdered => zf && !pf,
                            model::FloatCondition::NotEqualOrUnordered => !zf || pf,
                            model::FloatCondition::BelowOrdered => cf && !pf,
                            model::FloatCondition::BelowEqualOrdered => (cf || zf) && !pf,
                            model::FloatCondition::Above => !cf && !zf,
                            model::FloatCondition::AboveEqual => !cf,
                        }
                    };
                    for (a, b) in [
                        (f64::NAN, 1.0),
                        (1.0, f64::NAN),
                        (f64::NAN, f64::NAN),
                        (-0.0, 0.0),
                        (-1.0, 1.0),
                        (1.0, -1.0),
                        (f64::INFINITY, f64::INFINITY),
                    ] {
                        let expected = match predicate {
                            Equal => a == b,
                            NotEqual => a != b,
                            LessThan => a < b,
                            LessEqual => a <= b,
                            GreaterThan => a > b,
                            GreaterEqual => a >= b,
                        };
                        assert_eq!(evaluate(a, b), expected);
                    }
                    let desc = payload.describe();
                    assert!(desc.bundle.is_some());
                    assert_eq!(desc.clobbers, [(Timing::Late, payload.resources.flags())]);
                    assert_eq!(
                        desc.events()
                            .filter(|event| matches!(event, selected::Event::Clobber { .. }))
                            .count(),
                        1
                    );
                    let mut corrupted = payload.clone();
                    if let Opcode::FloatCompare { condition, .. } = &mut corrupted.opcode {
                        *condition = if *condition == model::FloatCondition::Above {
                            model::FloatCondition::EqualOrdered
                        } else {
                            model::FloatCondition::Above
                        };
                    }
                    let verifier =
                        Verifier::new(lower.receipt().owner().context().profile()).unwrap();
                    assert!(verifier.verify_payload(&corrupted, false).is_err());
                }
            }
            Ok::<_, std::convert::Infallible>(())
        })
        .unwrap();
    });
    assert_eq!(
        predicates,
        vec![
            Equal,
            NotEqual,
            LessThan,
            LessEqual,
            GreaterThan,
            GreaterEqual
        ]
    );
}
