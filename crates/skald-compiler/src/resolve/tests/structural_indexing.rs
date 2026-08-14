use super::*;

#[test]
fn normalizes_structural_index_reads_and_writes_to_ordinary_method_calls() {
    let output = resolve_text(concat!(
        "class Key { init() {} }\n",
        "class Table {\n",
        "  init() {}\n",
        "  fn index_get(ref key: Key) -> bool { return true; }\n",
        "  mut fn index_set(ref key: Key, replacement: i64) -> unit {}\n",
        "}\n",
        "fn read(ref table: Table, ref key: Key) -> bool { return table[key]; }\n",
        "fn write(mut ref table: Table, ref key: Key) -> unit { table[key] = 42; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let read = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedExpression::MethodCall(call) = return_value(&read.body.statements[0]) else {
        panic!("structural index read must normalize to a method call");
    };
    assert_eq!(call.method, MethodId::new(ClassId::new(1), 0));
    assert_eq!(call.arguments.len(), 1);

    let write = output.program.definitions.get(FunctionId::new(1)).unwrap();
    let ResolvedStatement::Expression(statement) = &write.body.statements[0] else {
        panic!("structural index write must normalize to a call statement");
    };
    let ResolvedExpression::MethodCall(call) = &statement.expression else {
        panic!("structural index write must call index_set");
    };
    assert_eq!(call.method, MethodId::new(ClassId::new(1), 1));
    assert_eq!(call.arguments.len(), 2);
}

#[test]
fn selects_inherited_protocol_methods_and_projects_the_receiver() {
    let output = resolve_text(concat!(
        "class Base {\n",
        "  init() {}\n",
        "  fn index_get(key: bool) -> i64 { return 1; }\n",
        "  mut fn index_set(key: bool, value: i64) -> unit {}\n",
        "}\n",
        "class Derived extends Base { init() { super(); } }\n",
        "fn read(ref value: Derived) -> i64 { return value[true]; }\n",
        "fn write(mut ref value: Derived) -> unit { value[false] = 2; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let read = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedExpression::MethodCall(call) = return_value(&read.body.statements[0]) else {
        panic!("expected inherited getter call");
    };
    assert_eq!(call.method, MethodId::new(ClassId::new(0), 0));
    assert_eq!(call.receiver.class(), ClassId::new(0));

    let write = output.program.definitions.get(FunctionId::new(1)).unwrap();
    let ResolvedStatement::Expression(statement) = &write.body.statements[0] else {
        panic!("expected inherited setter call statement");
    };
    let ResolvedExpression::MethodCall(call) = &statement.expression else {
        panic!("expected inherited setter call");
    };
    assert_eq!(call.method, MethodId::new(ClassId::new(0), 1));
    assert_eq!(call.receiver.class(), ClassId::new(0));
}

#[test]
fn getter_and_setter_eligibility_are_independent() {
    let output = resolve_text(concat!(
        "class ReadOnly { fn index_get(key: i64) -> i64 { return key; } }\n",
        "class WriteOnly { mut fn index_set(key: i64, value: bool) -> unit {} }\n",
        "fn read(ref value: ReadOnly) -> i64 { return value[3]; }\n",
        "fn write(mut ref value: WriteOnly) -> unit { value[4] = true; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
}

#[test]
fn resolves_explicit_shared_arrow_indexing_without_an_implicit_dereference() {
    let output = resolve_text(concat!(
        "class Values {\n",
        "  init() {}\n",
        "  fn index_get(key: i64) -> i64 { return key; }\n",
        "  mut fn index_set(key: i64, value: i64) -> unit {}\n",
        "}\n",
        "fn read(owner: shared Values) -> i64 { return owner->[1]; }\n",
        "fn write(owner: shared Values) -> unit { owner->[1] = 2; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let read = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedExpression::MethodCall(call) = return_value(&read.body.statements[0]) else {
        panic!("shared-arrow read must normalize to a method call");
    };
    assert!(matches!(
        call.receiver,
        ResolvedObjectReceiver::Dereference { .. }
    ));

    let write = output.program.definitions.get(FunctionId::new(1)).unwrap();
    let ResolvedStatement::Expression(statement) = &write.body.statements[0] else {
        panic!("shared-arrow write must normalize to a call statement");
    };
    let ResolvedExpression::MethodCall(call) = &statement.expression else {
        panic!("shared-arrow write must normalize to a method call");
    };
    assert!(matches!(
        call.receiver,
        ResolvedObjectReceiver::Dereference { .. }
    ));
}

#[test]
fn preserves_intrinsic_array_projection_and_assignment_nodes() {
    let output = resolve_text(concat!(
        "fn update(mut ref values: i64[]) -> i64 {\n",
        "  values[1] = 2;\n",
        "  return values[1];\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let update = output.program.definitions.get(FunctionId::new(0)).unwrap();
    assert!(matches!(
        update.body.statements[0],
        ResolvedStatement::ArrayAssignment(_)
    ));
    assert!(matches!(
        return_value(&update.body.statements[1]),
        ResolvedExpression::ArrayProjection(_)
    ));
}

#[test]
fn structural_index_dumps_are_deterministic_and_use_only_call_nodes() {
    let output = resolve_text(concat!(
        "class Values {\n",
        "  fn index_get(key: i64) -> bool { return true; }\n",
        "  mut fn index_set(key: i64, value: bool) -> unit {}\n",
        "}\n",
        "fn use(mut ref values: Values) -> bool { values[1] = true; return values[1]; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert_eq!(dump.matches("MethodCall").count(), 2);
    assert!(!dump.contains("ArrayAssignment"));
    assert!(!dump.contains("ArrayProjection"));
}

#[test]
fn diagnoses_missing_private_and_non_method_protocol_members() {
    let cases = [
        (
            "class Value {} fn use(ref value: Value) -> i64 { return value[0]; } fn main() -> i64 { return 0; }",
            INVALID_INDEX_PROTOCOL,
        ),
        (
            "class Value { private fn index_get(key: i64) -> i64 { return 0; } } fn use(ref value: Value) -> i64 { return value[0]; } fn main() -> i64 { return 0; }",
            PRIVATE_MEMBER_ACCESS,
        ),
        (
            "class Value { index_get: i64; } fn use(ref value: Value) -> i64 { return value[0]; } fn main() -> i64 { return 0; }",
            INVALID_INDEX_PROTOCOL,
        ),
        (
            "class Value { static index_get: i64 = 0; } fn use(ref value: Value) -> i64 { return value[0]; } fn main() -> i64 { return 0; }",
            INVALID_INDEX_PROTOCOL,
        ),
    ];

    for (source, code) in cases {
        let output = resolve_text(source);
        assert!(output.has_errors(), "source should be rejected: {source}");
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code),
            "expected {code}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn permits_private_protocol_selection_inside_the_declaring_class() {
    let output = resolve_text(concat!(
        "class Secret {\n",
        "  private fn index_get(key: i64) -> i64 { return key; }\n",
        "  private mut fn index_set(key: i64, value: i64) -> unit {}\n",
        "  mut fn round_trip() -> i64 { self[0] = 1; return self[0]; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
}

#[test]
fn diagnoses_a_static_field_receiver_without_panicking() {
    let output = resolve_text(concat!(
        "class Item { fn index_get(key: i64) -> i64 { return key; } }\n",
        "class Holder { static item: Item; }\n",
        "fn use() -> i64 { return Holder.item[0]; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.has_errors());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_INDEX_PROTOCOL));
}

#[test]
fn diagnoses_invalid_getter_shapes() {
    let declarations = [
        "static fn index_get(key: i64) -> i64 { return 0; }",
        "mut fn index_get(key: i64) -> i64 { return 0; }",
        "fn index_get() -> i64 { return 0; }",
        "fn index_get(mut ref key: Key) -> i64 { return 0; }",
    ];
    for declaration in declarations {
        let source = format!(
            "class Key {{}} class Value {{ {declaration} }} fn use(ref value: Value, mut ref key: Key) -> i64 {{ return value[key]; }} fn main() -> i64 {{ return 0; }}"
        );
        let output = resolve_text(&source);
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == INVALID_INDEX_PROTOCOL
                    && diagnostic.message.contains("index_get")
            }),
            "{:?}",
            output.diagnostics
        );
    }
}

#[test]
fn diagnoses_invalid_setter_shapes_without_requiring_a_getter() {
    let declarations = [
        "static fn index_set(key: i64, value: i64) -> unit {}",
        "fn index_set(key: i64, value: i64) -> unit {}",
        "mut fn index_set(key: i64) -> unit {}",
        "mut fn index_set(key: i64, mut ref value: Item) -> unit {}",
        "mut fn index_set(key: i64, value: i64) -> bool { return true; }",
    ];
    for declaration in declarations {
        let source = format!(
            "class Item {{}} class Value {{ {declaration} }} fn use(mut ref value: Value, mut ref item: Item) -> unit {{ value[0] = item; }} fn main() -> i64 {{ return 0; }}"
        );
        let output = resolve_text(&source);
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == INVALID_INDEX_PROTOCOL
                    && diagnostic.message.contains("index_set")
            }),
            "{:?}",
            output.diagnostics
        );
        assert!(!output.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == INVALID_INDEX_PROTOCOL && diagnostic.message.contains("index_get")
        }));
    }
}
