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
        "  values[1:] = values[:1];\n",
        "  var copy: i64[] = values[:];\n",
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
        update.body.statements[1],
        ResolvedStatement::ArrayAssignment(_)
    ));
    assert!(matches!(
        local_initializer(&update.body.statements[2]),
        ResolvedExpression::ArrayProjection(_)
    ));
    assert!(matches!(
        return_value(&update.body.statements[3]),
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
fn supports_class_valued_static_field_receivers() {
    let output = resolve_text(concat!(
        "class Item {\n",
        "  init() {}\n",
        "  fn index_get(key: i64) -> i64 { return key; }\n",
        "  mut fn slice_set(start: i64?, end: i64?, value: i64) -> unit {}\n",
        "}\n",
        "class Holder { static item: Item = Item(); }\n",
        "fn read() -> i64 { return Holder.item[0]; }\n",
        "fn write() -> unit { Holder.item[:] = 1; }\n",
        "fn explicit() -> i64 { return Holder.item.index_get(0); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let read = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedExpression::MethodCall(call) = return_value(&read.body.statements[0]) else {
        panic!("static field bracket read must normalize to a method call");
    };
    assert!(matches!(
        call.receiver,
        ResolvedObjectReceiver::StaticField { .. }
    ));

    let write = output.program.definitions.get(FunctionId::new(1)).unwrap();
    let ResolvedStatement::Expression(statement) = &write.body.statements[0] else {
        panic!("static field bracket write must normalize to a call statement");
    };
    let ResolvedExpression::MethodCall(call) = &statement.expression else {
        panic!("static field bracket write must normalize to a method call");
    };
    assert!(matches!(
        call.receiver,
        ResolvedObjectReceiver::StaticField { .. }
    ));

    let explicit = output.program.definitions.get(FunctionId::new(2)).unwrap();
    let ResolvedExpression::MethodCall(call) = return_value(&explicit.body.statements[0]) else {
        panic!("explicit static field method call must use the same receiver carrier");
    };
    assert!(matches!(
        call.receiver,
        ResolvedObjectReceiver::StaticField { .. }
    ));
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

#[test]
fn normalizes_every_structural_slice_shape_to_protocol_arguments() {
    let output = resolve_text(concat!(
        "class Window {\n",
        "  fn slice_get(start: i64?, end: i64?) -> bool { return true; }\n",
        "  mut fn slice_set(start: i64?, end: i64?, replacement: u8) -> unit {}\n",
        "}\n",
        "fn read_both(ref value: Window, start: i64, end: i64) -> bool { return value[start:end]; }\n",
        "fn read_start(ref value: Window, start: i64) -> bool { return value[start:]; }\n",
        "fn read_end(ref value: Window, end: i64) -> bool { return value[:end]; }\n",
        "fn read_neither(ref value: Window) -> bool { return value[:]; }\n",
        "fn write_both(mut ref value: Window, start: i64, end: i64) -> unit { value[start:end] = 1u8; }\n",
        "fn write_start(mut ref value: Window, start: i64) -> unit { value[start:] = 2u8; }\n",
        "fn write_end(mut ref value: Window, end: i64) -> unit { value[:end] = 3u8; }\n",
        "fn write_neither(mut ref value: Window) -> unit { value[:] = 4u8; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let expected_bounds = [(true, true), (true, false), (false, true), (false, false)];
    for (index, (start_present, end_present)) in expected_bounds.into_iter().enumerate() {
        let read = output
            .program
            .definitions
            .get(FunctionId::new(index))
            .unwrap();
        let ResolvedExpression::MethodCall(call) = return_value(&read.body.statements[0]) else {
            panic!("slice read must normalize to slice_get");
        };
        assert_eq!(call.method, MethodId::new(ClassId::new(0), 0));
        assert_slice_bound_presence(&call.arguments, start_present, end_present);

        let write = output
            .program
            .definitions
            .get(FunctionId::new(index + 4))
            .unwrap();
        let ResolvedStatement::Expression(statement) = &write.body.statements[0] else {
            panic!("slice assignment must normalize to a call statement");
        };
        let ResolvedExpression::MethodCall(call) = &statement.expression else {
            panic!("slice assignment must normalize to slice_set");
        };
        assert_eq!(call.method, MethodId::new(ClassId::new(0), 1));
        assert_eq!(call.arguments.len(), 3);
        assert_slice_bound_presence(&call.arguments[..2], start_present, end_present);
        assert!(!matches!(call.arguments[2], ResolvedExpression::Absent(_)));
    }
}

fn assert_slice_bound_presence(
    arguments: &[ResolvedExpression],
    start_present: bool,
    end_present: bool,
) {
    assert_eq!(arguments.len(), 2);
    assert_eq!(
        !matches!(arguments[0], ResolvedExpression::Absent(_)),
        start_present
    );
    assert_eq!(
        !matches!(arguments[1], ResolvedExpression::Absent(_)),
        end_present
    );
}

#[test]
fn slice_getter_and_setter_are_independent() {
    let output = resolve_text(concat!(
        "class ReadOnly { fn slice_get(start: i64?, end: i64?) -> bool { return true; } }\n",
        "class WriteOnly { mut fn slice_set(start: i64?, end: i64?, value: u8) -> unit {} }\n",
        "fn read(ref value: ReadOnly) -> bool { return value[:]; }\n",
        "fn write(mut ref value: WriteOnly) -> unit { value[:] = 1u8; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
}

#[test]
fn diagnoses_missing_slice_protocol_members_by_operation() {
    let cases = [
        (
            "class Value {} fn use(ref value: Value) -> i64 { return value[:]; } fn main() -> i64 { return 0; }",
            "slice_get",
        ),
        (
            "class Value {} fn use(mut ref value: Value) -> unit { value[:] = 1; } fn main() -> i64 { return 0; }",
            "slice_set",
        ),
    ];
    for (source, protocol) in cases {
        let output = resolve_text(source);
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == INVALID_INDEX_PROTOCOL && diagnostic.message.contains(protocol)
            }),
            "{:?}",
            output.diagnostics
        );
    }
}

#[test]
fn diagnoses_invalid_slice_getter_shapes() {
    let declarations = [
        "static fn slice_get(start: i64?, end: i64?) -> i64 { return 0; }",
        "mut fn slice_get(start: i64?, end: i64?) -> i64 { return 0; }",
        "fn slice_get(start: i64?) -> i64 { return 0; }",
        "fn slice_get(start: i64, end: i64?) -> i64 { return 0; }",
        "fn slice_get(start: i64?, end: u64?) -> i64 { return 0; }",
        "fn slice_get(ref start: i64?, end: i64?) -> i64 { return 0; }",
    ];
    for declaration in declarations {
        let source = format!(
            "class Value {{ {declaration} }} fn use(ref value: Value) -> i64 {{ return value[:]; }} fn main() -> i64 {{ return 0; }}"
        );
        let output = resolve_text(&source);
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == INVALID_INDEX_PROTOCOL
                    && diagnostic.message.contains("slice_get")
            }),
            "{:?}",
            output.diagnostics
        );
    }
}

#[test]
fn diagnoses_invalid_slice_setter_shapes_without_selecting_a_getter() {
    let declarations = [
        "static fn slice_set(start: i64?, end: i64?, value: i64) -> unit {}",
        "fn slice_set(start: i64?, end: i64?, value: i64) -> unit {}",
        "mut fn slice_set(start: i64?, end: i64?) -> unit {}",
        "mut fn slice_set(start: i64, end: i64?, value: i64) -> unit {}",
        "mut fn slice_set(start: i64?, ref end: i64?, value: i64) -> unit {}",
        "mut fn slice_set(start: i64?, end: i64?, mut ref value: Item) -> unit {}",
        "mut fn slice_set(start: i64?, end: i64?, value: i64) -> bool { return true; }",
    ];
    for declaration in declarations {
        let source = format!(
            "class Item {{}} class Value {{ {declaration} }} fn use(mut ref value: Value, mut ref item: Item) -> unit {{ value[:] = item; }} fn main() -> i64 {{ return 0; }}"
        );
        let output = resolve_text(&source);
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == INVALID_INDEX_PROTOCOL
                    && diagnostic.message.contains("slice_set")
            }),
            "{:?}",
            output.diagnostics
        );
        assert!(!output.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == INVALID_INDEX_PROTOCOL && diagnostic.message.contains("slice_get")
        }));
    }
}

#[test]
fn structural_slice_keeps_effectful_receiver_and_bounds_once_without_hidden_length() {
    let output = resolve_text(concat!(
        "class Window { init() {} fn slice_get(start: i64?, end: i64?) -> bool { return true; } }\n",
        "fn make() -> Window { return Window(); }\n",
        "fn start() -> i64 { return 1; }\n",
        "fn end() -> i64 { return 2; }\n",
        "fn read() -> bool { return make()[start():end()]; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let read = output.program.definitions.get(FunctionId::new(3)).unwrap();
    let ResolvedExpression::MethodCall(call) = return_value(&read.body.statements[0]) else {
        panic!("structural slice read must normalize to a method call");
    };
    let ResolvedObjectReceiver::Produced { producer, .. } = &call.receiver else {
        panic!("effectful class result must remain one produced receiver");
    };
    assert!(matches!(&**producer, ResolvedExpression::DirectCall(_)));
    assert!(matches!(
        call.arguments[0],
        ResolvedExpression::DirectCall(_)
    ));
    assert!(matches!(
        call.arguments[1],
        ResolvedExpression::DirectCall(_)
    ));

    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert_eq!(dump.matches("MethodCall").count(), 1);
    assert!(!dump.contains("ArrayProjection"));
    assert!(!dump.contains("ArrayLength"));
}

#[test]
fn structural_getter_results_can_receive_read_only_methods_directly() {
    let output = resolve_text(concat!(
        "class Part { init() {} fn read() -> i64 { return 1; } }\n",
        "interface Source { fn slice_get(start: i64?, end: i64?) -> Part; }\n",
        "class Value implements Source {\n",
        "  fn index_get(key: i64) -> Part { return Part(); }\n",
        "  fn slice_get(start: i64?, end: i64?) -> Part { return Part(); }\n",
        "}\n",
        "fn direct(ref value: Value) -> i64 { return value[0].read(); }\n",
        "fn through_interface(ref value: Source) -> i64 { return value[:].read(); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let direct = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedExpression::MethodCall(direct_call) = return_value(&direct.body.statements[0])
    else {
        panic!("structural result member access must select an ordinary method call");
    };
    let ResolvedObjectReceiver::Produced { producer, .. } = &direct_call.receiver else {
        panic!("structural class result must become a produced receiver");
    };
    assert!(matches!(&**producer, ResolvedExpression::MethodCall(_)));

    let through_interface = output.program.definitions.get(FunctionId::new(1)).unwrap();
    let ResolvedExpression::MethodCall(interface_call) =
        return_value(&through_interface.body.statements[0])
    else {
        panic!("interface structural result member access must select an ordinary method call");
    };
    let ResolvedObjectReceiver::Produced { producer, .. } = &interface_call.receiver else {
        panic!("structural interface result must become a produced receiver");
    };
    assert!(matches!(&**producer, ResolvedExpression::InterfaceCall(_)));
}

#[test]
fn normalizes_interface_brackets_to_exact_declared_requirements() {
    let output = resolve_text(concat!(
        "interface Sequence {\n",
        "  fn index_get(key: bool) -> i64;\n",
        "  mut fn index_set(key: bool, replacement: u8) -> unit;\n",
        "  fn slice_get(start: i64?, end: i64?) -> bool;\n",
        "  mut fn slice_set(start: i64?, end: i64?, replacement: u8) -> unit;\n",
        "}\n",
        "fn read_index(ref value: Sequence) -> i64 { return value[true]; }\n",
        "fn write_index(mut ref value: Sequence) -> unit { value[false] = 1u8; }\n",
        "fn read_slice(ref value: Sequence) -> bool { return value[:]; }\n",
        "fn write_slice(mut ref value: Sequence) -> unit { value[1:] = 2u8; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    for (function, requirement, assignment) in
        [(0, 0, false), (1, 1, true), (2, 2, false), (3, 3, true)]
    {
        let definition = output
            .program
            .definitions
            .get(FunctionId::new(function))
            .unwrap();
        let expression = if assignment {
            let ResolvedStatement::Expression(statement) = &definition.body.statements[0] else {
                panic!("interface bracket write must normalize to a call statement");
            };
            &statement.expression
        } else {
            return_value(&definition.body.statements[0])
        };
        let ResolvedExpression::InterfaceCall(call) = expression else {
            panic!("interface bracket must normalize to an interface call");
        };
        assert_eq!(call.interface, InterfaceId::new(0));
        assert_eq!(
            call.requirement,
            InterfaceRequirementId::new(InterfaceId::new(0), requirement)
        );
        assert!(matches!(
            call.receiver,
            ResolvedInterfaceReceiver::Binding { .. }
        ));
    }
}

#[test]
fn supports_shared_interface_arrow_and_star_bracket_receivers() {
    let output = resolve_text(concat!(
        "interface Sequence { fn index_get(key: i64) -> i64; fn slice_get(start: i64?, end: i64?) -> i64; }\n",
        "fn arrow(owner: shared Sequence) -> i64 { return owner->[0]; }\n",
        "fn star(owner: shared Sequence) -> i64 { return (*owner)[:]; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    for function in 0..=1 {
        let definition = output
            .program
            .definitions
            .get(FunctionId::new(function))
            .unwrap();
        let ResolvedExpression::InterfaceCall(call) = return_value(&definition.body.statements[0])
        else {
            panic!("shared interface bracket must normalize to an interface call");
        };
        assert!(matches!(
            call.receiver,
            ResolvedInterfaceReceiver::Dereference(_)
        ));
    }
}

#[test]
fn validates_interface_protocol_requirements_before_call_checking() {
    let cases = [
        (
            "interface Bad { mut fn index_get(key: i64) -> i64; } fn use(ref value: Bad) -> i64 { return value[0]; } fn main() -> i64 { return 0; }",
            "index_get",
        ),
        (
            "interface Bad { fn index_set(key: i64, value: i64) -> unit; } fn use(mut ref value: Bad) -> unit { value[0] = 1; } fn main() -> i64 { return 0; }",
            "index_set",
        ),
        (
            "interface Bad { fn slice_get(start: i64, end: i64?) -> i64; } fn use(ref value: Bad) -> i64 { return value[:]; } fn main() -> i64 { return 0; }",
            "slice_get",
        ),
        (
            "interface Bad { mut fn slice_set(start: i64?, end: i64?, value: i64) -> i64; } fn use(mut ref value: Bad) -> unit { value[:] = 1; } fn main() -> i64 { return 0; }",
            "slice_set",
        ),
    ];
    for (source, protocol) in cases {
        let output = resolve_text(source);
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == INVALID_INDEX_PROTOCOL && diagnostic.message.contains(protocol)
            }),
            "{:?}",
            output.diagnostics
        );
    }
}
