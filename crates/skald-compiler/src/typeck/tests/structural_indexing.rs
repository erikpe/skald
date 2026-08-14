use super::*;

#[test]
fn checks_structural_indexing_with_independent_key_result_and_replacement_types() {
    let output = check_text(concat!(
        "class Key { init() {} }\n",
        "class Table {\n",
        "  init() {}\n",
        "  fn index_get(ref key: Key) -> bool { return true; }\n",
        "  mut fn index_set(ref key: Key, replacement: i64) -> unit {}\n",
        "}\n",
        "fn use(mut ref table: Table, ref key: Key) -> bool {\n",
        "  table[key] = 42;\n",
        "  return table[key];\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let definition = hir.definitions.get(FunctionId::new(0)).unwrap();

    let HirStatement::Call(setter) = &definition.body.statements[0] else {
        panic!("structural assignment must lower through an ordinary call statement");
    };
    let HirExpressionKind::MethodCall {
        target, arguments, ..
    } = &setter.call.kind
    else {
        panic!("structural assignment must lower to an ordinary method call");
    };
    assert_eq!(
        target.selected(),
        crate::identity::MethodId::new(crate::identity::ClassId::new(1), 1)
    );
    assert_eq!(arguments.len(), 2);
    assert_eq!(setter.call.ty, Type::Unit);

    let getter = returned_expression(definition);
    let HirExpressionKind::MethodCall {
        target, arguments, ..
    } = &getter.kind
    else {
        panic!("structural read must lower to an ordinary method call");
    };
    assert_eq!(
        target.selected(),
        crate::identity::MethodId::new(crate::identity::ClassId::new(1), 0)
    );
    assert_eq!(arguments.len(), 1);
    assert_eq!(getter.ty, Type::Bool);

    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert_eq!(dump.matches("MethodCall").count(), 2);
    assert!(!dump.contains("ArrayElementAssignment"));
}

#[test]
fn structural_sugar_and_explicit_calls_select_the_same_methods() {
    let output = check_text(concat!(
        "class Values {\n",
        "  init() {}\n",
        "  fn index_get(key: bool) -> i64 { return 1; }\n",
        "  mut fn index_set(key: bool, value: u8) -> unit {}\n",
        "}\n",
        "fn use(mut ref values: Values) -> i64 {\n",
        "  values[true] = 2u8;\n",
        "  values.index_set(true, 2u8);\n",
        "  return values[true] + values.index_get(true);\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let definition = hir.definitions.get(FunctionId::new(0)).unwrap();

    let method_of_statement = |statement: &HirStatement| {
        let HirStatement::Call(call) = statement else {
            panic!("expected call statement");
        };
        let HirExpressionKind::MethodCall { target, .. } = &call.call.kind else {
            panic!("expected method call");
        };
        target.selected()
    };
    assert_eq!(
        method_of_statement(&definition.body.statements[0]),
        method_of_statement(&definition.body.statements[1])
    );

    let HirExpressionKind::Binary { left, right, .. } = &returned_expression(definition).kind
    else {
        panic!("expected getter comparison expression");
    };
    let method_of_expression = |expression: &HirExpression| {
        let HirExpressionKind::MethodCall { target, .. } = &expression.kind else {
            panic!("expected method call expression");
        };
        target.selected()
    };
    assert_eq!(method_of_expression(left), method_of_expression(right));
}

#[test]
fn structural_setter_reuses_ordinary_receiver_mutability_checks() {
    let output = check_text(concat!(
        "class Values { init() {} mut fn index_set(key: i64, value: i64) -> unit {} }\n",
        "fn use(ref values: Values) -> unit { values[0] = 1; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == crate::typeck::READ_ONLY_RECEIVER
            && diagnostic.message.contains("index_set")
    }));
}

#[test]
fn structural_calls_reuse_ordinary_argument_mode_and_type_checks() {
    let output = check_text(concat!(
        "class Key { init() {} }\n",
        "class Values {\n",
        "  init() {}\n",
        "  fn index_get(ref key: Key) -> i64 { return 0; }\n",
        "  mut fn index_set(ref key: Key, value: bool) -> unit {}\n",
        "}\n",
        "fn use(mut ref values: Values) -> i64 {\n",
        "  values[0] = 1;\n",
        "  return values[false];\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == crate::typeck::TYPE_MISMATCH));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == crate::typeck::INVALID_ALIAS_ARGUMENT));
}

#[test]
fn checks_structural_indexing_on_a_closed_generic_class() {
    let hir = check_generic_source(concat!(
        "class Key { init() {} }\n",
        "class Table<K, R, V> {\n",
        "  result: R;\n",
        "  init(result: R) { self.result = result; }\n",
        "  fn index_get(ref key: K) -> R { return self.result; }\n",
        "  mut fn index_set(ref key: K, replacement: V) -> unit {}\n",
        "}\n",
        "fn use(mut ref table: Table<Key, bool, i64>, ref key: Key) -> bool {\n",
        "  table[key] = 7;\n",
        "  return table[key];\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let definition = hir
        .definitions
        .iter()
        .find(|definition| {
            hir.declarations
                .get(definition.function)
                .is_some_and(|declaration| declaration.name == "use")
        })
        .expect("closed-generic use function must be checked");
    assert!(matches!(
        definition.body.statements[0],
        HirStatement::Call(_)
    ));
    assert!(matches!(
        returned_expression(definition).kind,
        HirExpressionKind::MethodCall { .. }
    ));
}

#[test]
fn checks_all_structural_slice_shapes_with_typed_optional_bounds() {
    let output = check_text(concat!(
        "class Window {\n",
        "  init() {}\n",
        "  fn slice_get(start: i64?, end: i64?) -> bool { return true; }\n",
        "  mut fn slice_set(start: i64?, end: i64?, replacement: u8) -> unit {}\n",
        "}\n",
        "fn use(mut ref value: Window, start: i64, end: i64) -> bool {\n",
        "  var both: bool = value[start:end];\n",
        "  var only_start: bool = value[start:];\n",
        "  var only_end: bool = value[:end];\n",
        "  var neither: bool = value[:];\n",
        "  value[start:end] = 1u8;\n",
        "  value[start:] = 2u8;\n",
        "  value[:end] = 3u8;\n",
        "  value[:] = 4u8;\n",
        "  return both;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let definition = hir.definitions.get(FunctionId::new(0)).unwrap();
    let expected_bounds = [(true, true), (true, false), (false, true), (false, false)];

    for (index, (start_present, end_present)) in expected_bounds.into_iter().enumerate() {
        let HirStatement::Local(local) = &definition.body.statements[index] else {
            panic!("slice read must initialize a scalar local");
        };
        let HirLocalInitializer::Value(expression) = &local.initializer else {
            panic!("slice_get result must be a scalar value");
        };
        let HirExpressionKind::MethodCall {
            target, arguments, ..
        } = &expression.kind
        else {
            panic!("slice read must remain an ordinary method call in HIR");
        };
        assert_eq!(
            target.selected(),
            crate::identity::MethodId::new(crate::identity::ClassId::new(0), 0)
        );
        assert_typed_slice_bounds(arguments, start_present, end_present);
        assert_eq!(expression.ty, Type::Bool);

        let HirStatement::Call(statement) = &definition.body.statements[index + 4] else {
            panic!("slice assignment must remain an ordinary call statement");
        };
        let HirExpressionKind::MethodCall {
            target, arguments, ..
        } = &statement.call.kind
        else {
            panic!("slice assignment must call slice_set");
        };
        assert_eq!(
            target.selected(),
            crate::identity::MethodId::new(crate::identity::ClassId::new(0), 1)
        );
        assert_typed_slice_bounds(&arguments[..2], start_present, end_present);
        assert!(matches!(
            arguments[2],
            crate::hir::HirCallArgument::Value(_)
        ));
        assert_eq!(statement.call.ty, Type::Unit);
    }

    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert_eq!(dump.matches("MethodCall").count(), 8);
    assert!(!dump.contains("ArraySlice"));
    assert!(!dump.contains("ArraySliceAssignment"));
}

fn assert_typed_slice_bounds(
    arguments: &[crate::hir::HirCallArgument],
    start_present: bool,
    end_present: bool,
) {
    assert_eq!(arguments.len(), 2);
    for (argument, present) in arguments.iter().zip([start_present, end_present]) {
        let crate::hir::HirCallArgument::Optional { source, payload } = argument else {
            panic!("slice bounds must be typed as optional call arguments");
        };
        assert_eq!(*payload, crate::hir::HirPrimitiveType::I64);
        assert_eq!(
            matches!(source, crate::hir::HirOptionalSource::Present(_)),
            present
        );
        assert_eq!(
            matches!(source, crate::hir::HirOptionalSource::Absent { .. }),
            !present
        );
    }
}

#[test]
fn structural_slice_reuses_ordinary_receiver_mutability_checks() {
    let output = check_text(concat!(
        "class Window { init() {} mut fn slice_set(start: i64?, end: i64?, value: i64) -> unit {} }\n",
        "fn use(ref value: Window) -> unit { value[:] = 1; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == crate::typeck::READ_ONLY_RECEIVER
            && diagnostic.message.contains("slice_set")
    }));
}

#[test]
fn supplied_slice_bounds_are_injected_once_as_ordinary_call_arguments() {
    let output = check_text(concat!(
        "class Window { init() {} fn slice_get(start: i64?, end: i64?) -> bool { return true; } }\n",
        "fn next() -> i64 { return 1; }\n",
        "fn use(ref value: Window) -> bool { return value[next():next()]; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let definition = hir.definitions.get(FunctionId::new(1)).unwrap();
    let expression = returned_expression(definition);
    let HirExpressionKind::MethodCall { arguments, .. } = &expression.kind else {
        panic!("structural slice must be an ordinary method call");
    };
    assert_typed_slice_bounds(arguments, true, true);
    for argument in arguments {
        let crate::hir::HirCallArgument::Optional {
            source: crate::hir::HirOptionalSource::Present(bound),
            ..
        } = argument
        else {
            panic!("supplied bound must receive one-layer optional injection");
        };
        assert!(matches!(bound.kind, HirExpressionKind::DirectCall { .. }));
    }
}
