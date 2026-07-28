use super::*;

#[test]
fn resolution_preserves_primitive_integer_cast_targets_without_lookup() {
    let output = resolve_text(
        "fn cast(value: i64) -> u8 { return (u8) (u64) value; }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedExpression::IntegerCast(outer) = return_value(&definition.body.statements[0])
    else {
        panic!("expected outer resolved integer cast");
    };
    assert_eq!(outer.target, ResolvedIntegerType::U8);
    let ResolvedExpression::IntegerCast(inner) = outer.source.as_ref() else {
        panic!("expected nested resolved integer cast");
    };
    assert_eq!(inner.target, ResolvedIntegerType::U64);
    assert!(matches!(
        inner.source.as_ref(),
        ResolvedExpression::Binding(_)
    ));

    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert!(dump.contains("IntegerCast target u8"));
    assert!(dump.contains("IntegerCast target u64"));
}
