use super::*;

#[test]
fn resolution_preserves_nested_primitive_cast_targets_without_lookup() {
    let output = resolve_text(
        "fn cast(value: i64) -> u8 { return (u8) (u64) value; }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedExpression::PrimitiveCast(outer) = return_value(&definition.body.statements[0])
    else {
        panic!("expected outer resolved primitive cast");
    };
    assert_eq!(outer.target, ResolvedPrimitiveType::U8);
    let ResolvedExpression::PrimitiveCast(inner) = outer.source.as_ref() else {
        panic!("expected nested resolved primitive cast");
    };
    assert_eq!(inner.target, ResolvedPrimitiveType::U64);
    assert!(matches!(
        inner.source.as_ref(),
        ResolvedExpression::Binding(_)
    ));

    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert!(dump.contains("PrimitiveCast target u8"));
    assert!(dump.contains("PrimitiveCast target u64"));
}

#[test]
fn resolution_preserves_all_primitive_targets_without_declaration_lookup() {
    let output = resolve_text(
        "fn cast(value: i64) -> unit {\n\
           var a: i64 = (i64) value;\n\
           var b: u64 = (u64) value;\n\
           var c: u8 = (u8) value;\n\
           var d: f64 = (f64) value;\n\
           var e: bool = (bool) value;\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let expected = [
        ResolvedPrimitiveType::I64,
        ResolvedPrimitiveType::U64,
        ResolvedPrimitiveType::U8,
        ResolvedPrimitiveType::F64,
        ResolvedPrimitiveType::Bool,
    ];
    for (statement, target) in definition.body.statements.iter().zip(expected) {
        let ResolvedExpression::PrimitiveCast(cast) = local_initializer(statement) else {
            panic!("expected resolved primitive cast");
        };
        assert_eq!(cast.target, target);
    }

    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert!(dump.contains("PrimitiveCast target f64"));
    assert!(dump.contains("PrimitiveCast target bool"));
}
