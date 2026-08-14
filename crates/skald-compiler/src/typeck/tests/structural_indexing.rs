use super::*;

#[test]
fn structural_sugar_exists_only_in_the_source_ast_dump() {
    let source = concat!(
        "class Cell {\n",
        "  init() {}\n",
        "  fn index_get(key: i64) -> i64 { return key; }\n",
        "  mut fn index_set(key: i64, replacement: i64) -> unit {}\n",
        "  fn slice_get(start: i64?, end: i64?) -> i64 { return 1; }\n",
        "  mut fn slice_set(start: i64?, end: i64?, replacement: i64) -> unit {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var cell: Cell = Cell();\n",
        "  var index: i64 = cell[0];\n",
        "  cell[0] = 2;\n",
        "  var slice: i64 = cell[:];\n",
        "  cell[:] = slice;\n",
        "  return index;\n",
        "}\n",
    );
    let (_, parsed) = crate::test_support::parse_source(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ast_dump = crate::syntax::dump_ast(&parsed.ast);
    assert_eq!(ast_dump, crate::syntax::dump_ast(&parsed.ast));
    assert_eq!(ast_dump.matches("BracketProjection").count(), 4);

    let resolved = crate::resolve::resolve(&parsed.ast);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let resolved_dump = crate::resolve::dump_resolved(&resolved.program);
    assert_eq!(
        resolved_dump,
        crate::resolve::dump_resolved(&resolved.program)
    );
    for method in 0..4 {
        assert!(
            resolved_dump.contains(&format!("MethodCall c0:method{method}")),
            "{resolved_dump}"
        );
    }
    assert!(!resolved_dump.contains("BracketProjection"));
    assert!(!resolved_dump.contains("Structural"));

    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked
        .hir
        .expect("valid structural source must produce HIR");
    let hir_dump = dump_hir(&hir);
    assert_eq!(hir_dump, dump_hir(&hir));
    for method in 0..4 {
        assert!(
            hir_dump.contains(&format!("MethodCall Direct c0:method{method}")),
            "{hir_dump}"
        );
    }
    assert!(!hir_dump.contains("BracketProjection"));
    assert!(!hir_dump.contains("Structural"));

    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).expect("ordinary calls selected by brackets must verify");
    let mir_dump = crate::mir::dump_mir(&mir);
    assert_eq!(mir_dump, crate::mir::dump_mir(&mir));
    for method in 0..4 {
        assert!(
            mir_dump.contains(&format!("call direct c0:method{method}")),
            "{mir_dump}"
        );
    }
    assert!(!mir_dump.contains("BracketProjection"));
    assert!(!mir_dump.contains("Structural"));
}

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

#[test]
fn interface_brackets_reuse_interface_calls_for_all_four_operations() {
    let output = check_text(concat!(
        "interface Sequence {\n",
        "  fn index_get(key: i64) -> i64;\n",
        "  mut fn index_set(key: i64, replacement: i64) -> unit;\n",
        "  fn slice_get(start: i64?, end: i64?) -> i64;\n",
        "  mut fn slice_set(start: i64?, end: i64?, replacement: i64) -> unit;\n",
        "}\n",
        "fn use(mut ref value: Sequence) -> i64 {\n",
        "  value[0] = 1;\n",
        "  value[:] = 2;\n",
        "  return value[0] + value[:];\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let definition = hir.definitions.get(FunctionId::new(0)).unwrap();

    for (statement, requirement) in definition.body.statements[..2].iter().zip([1, 3]) {
        let HirStatement::Call(statement) = statement else {
            panic!("interface bracket assignment must be a call statement");
        };
        let HirExpressionKind::InterfaceCall { target, .. } = &statement.call.kind else {
            panic!("interface bracket assignment must remain an interface call");
        };
        assert_eq!(target.requirement.index(), requirement);
    }

    let HirExpressionKind::Binary { left, right, .. } = &returned_expression(definition).kind
    else {
        panic!("expected two interface bracket reads");
    };
    for (expression, requirement) in [(left, 0), (right, 2)] {
        let HirExpressionKind::InterfaceCall { target, .. } = &expression.kind else {
            panic!("interface bracket read must remain an interface call");
        };
        assert_eq!(target.requirement.index(), requirement);
    }

    let dump = dump_hir(&hir);
    assert_eq!(dump.matches("InterfaceCall").count(), 4);
    assert!(!dump.contains("ArrayElement"));
    assert!(!dump.contains("ArraySlice"));
    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).expect("structural interface calls must lower and verify");
}

#[test]
fn structural_brackets_preserve_virtual_and_private_dispatch_selection() {
    let output = check_text(concat!(
        "class Root {\n",
        "  init() {}\n",
        "  virtual fn index_get(key: i64) -> i64 { return 1; }\n",
        "  virtual fn slice_get(start: i64?, end: i64?) -> i64 { return 2; }\n",
        "}\n",
        "class Leaf extends Root {\n",
        "  init() { super(); }\n",
        "  override fn index_get(key: i64) -> i64 { return 3; }\n",
        "  override fn slice_get(start: i64?, end: i64?) -> i64 { return 4; }\n",
        "}\n",
        "class Secret {\n",
        "  init() {}\n",
        "  private fn index_get(key: i64) -> i64 { return key; }\n",
        "  fn read() -> i64 { return self[5]; }\n",
        "}\n",
        "fn through(ref value: Root) -> i64 { return value[0] + value[:]; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let through = hir.definitions.get(FunctionId::new(0)).unwrap();
    let HirExpressionKind::Binary { left, right, .. } = &returned_expression(through).kind else {
        panic!("expected virtual index and slice reads");
    };
    for expression in [left, right] {
        assert!(matches!(
            expression.kind,
            HirExpressionKind::MethodCall {
                target: crate::hir::HirMethodCallTarget::Virtual { .. },
                ..
            }
        ));
    }

    let secret = hir
        .class_definitions
        .get(crate::identity::ClassId::new(2))
        .unwrap()
        .methods
        .get(1)
        .expect("private bracket caller method must exist");
    let HirStatement::Return(returned) = &secret.body.statements[0] else {
        panic!("private structural read must return its call");
    };
    let crate::hir::HirReturnValue::Scalar(expression) = returned.value.as_ref().unwrap() else {
        panic!("private structural read must return a scalar");
    };
    assert!(matches!(
        expression.kind,
        HirExpressionKind::MethodCall {
            target: crate::hir::HirMethodCallTarget::Direct(_),
            ..
        }
    ));
}

#[test]
fn class_brackets_support_fields_statics_checked_views_and_unwrapped_owners() {
    let output = check_text(concat!(
        "class Item {\n",
        "  init() {}\n",
        "  fn index_get(key: i64) -> i64 { return key; }\n",
        "  mut fn index_set(key: i64, value: i64) -> unit {}\n",
        "  fn slice_get(start: i64?, end: i64?) -> i64 { return 7; }\n",
        "  mut fn slice_set(start: i64?, end: i64?, value: i64) -> unit {}\n",
        "}\n",
        "class Holder {\n",
        "  item: Item;\n",
        "  static current: Item = Item();\n",
        "  init() { self.item = Item(); }\n",
        "  mut fn through_self() -> i64 { self.item[0] = 1; return self.item[:]; }\n",
        "}\n",
        "fn field(mut ref holder: Holder) -> i64 { holder.item[:] = 1; return holder.item[2]; }\n",
        "fn static_field() -> i64 { Holder.current[:] = 2; return Holder.current[3]; }\n",
        "fn checked(ref value: Obj) -> i64 { return ((Item) value)[4]; }\n",
        "fn optional(value: Item?) -> i64 { return value![:]; }\n",
        "fn unwrapped(owner: (shared Item)?) -> i64 { return owner!->[5]; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let static_read = returned_expression(hir.definitions.get(FunctionId::new(1)).unwrap());
    let HirExpressionKind::MethodCall { receiver, .. } = &static_read.kind else {
        panic!("static field bracket must remain a method call");
    };
    assert!(matches!(
        receiver,
        crate::hir::HirObjectReceiver::View { view, .. }
            if matches!(view.source, crate::hir::HirViewSource::Static { .. })
    ));

    let checked = returned_expression(hir.definitions.get(FunctionId::new(2)).unwrap());
    assert!(matches!(
        checked.kind,
        HirExpressionKind::MethodCall {
            receiver: crate::hir::HirObjectReceiver::Checked { .. },
            ..
        }
    ));
    let optional = returned_expression(hir.definitions.get(FunctionId::new(3)).unwrap());
    assert!(matches!(
        optional.kind,
        HirExpressionKind::MethodCall {
            receiver: crate::hir::HirObjectReceiver::View { .. },
            ..
        }
    ));

    let dump = dump_hir(&hir);
    assert!(dump.contains("StaticMethodReceiver"), "{dump}");
    let mir = crate::test_support::lower_hir_to_final_mir(&hir);
    crate::mir::verify_mir(&mir).expect("all structural receiver forms must lower and verify");
}

#[test]
fn shared_bracket_receivers_preserve_stable_and_anchored_sources_before_arguments() {
    let output = check_text(concat!(
        "class Item { init() {} fn index_get(key: i64) -> i64 { return key; } }\n",
        "class Holder { owner: shared Item; init() { self.owner = new Item(); } }\n",
        "fn effect() -> i64 { return 1; }\n",
        "fn make() -> shared Item { return new Item(); }\n",
        "fn stable(owner: shared Item) -> i64 { return owner->[effect()]; }\n",
        "fn replaceable(ref holder: Holder) -> i64 { return holder.owner->[effect()]; }\n",
        "fn produced() -> i64 { return make()->[effect()]; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    for (function, anchored) in [(2, false), (3, true), (4, true)] {
        let expression =
            returned_expression(hir.definitions.get(FunctionId::new(function)).unwrap());
        let HirExpressionKind::MethodCall {
            receiver,
            arguments,
            ..
        } = &expression.kind
        else {
            panic!("shared bracket must remain a method call");
        };
        match (receiver, anchored) {
            (crate::hir::HirObjectReceiver::Place { .. }, false) => {}
            (crate::hir::HirObjectReceiver::View { view, .. }, true) => assert!(matches!(
                view.source,
                crate::hir::HirViewSource::AnchoredShared { .. }
            )),
            _ => panic!("shared bracket receiver has the wrong stable/anchored carrier"),
        }
        assert!(matches!(
            arguments[0],
            crate::hir::HirCallArgument::Value(HirExpression {
                kind: HirExpressionKind::DirectCall { .. },
                ..
            })
        ));
    }
    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).expect("stable and anchored shared brackets must verify");
}

#[test]
fn rejects_raw_shared_and_produced_mutable_bracket_receivers() {
    let raw = crate::test_support::resolve_source(concat!(
        "class Item { init() {} fn index_get(key: i64) -> i64 { return key; } }\n",
        "fn use(owner: shared Item) -> i64 { return owner[0]; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let diagnostic = raw
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == crate::resolve::IMPLICIT_SHARED_DEREFERENCE)
        .expect("raw shared brackets must require explicit dereference");
    assert_eq!(
        diagnostic.message,
        "shared owner bracket access requires explicit dereference"
    );
    assert!(diagnostic
        .notes
        .iter()
        .any(|note| note.contains("owner->[...]")));
    assert!(diagnostic
        .notes
        .iter()
        .any(|note| note.contains("(*owner)[...]")));

    let optional_raw = check_text(concat!(
        "class Item { init() {} fn index_get(key: i64) -> i64 { return key; } }\n",
        "fn use(owner: (shared Item)?) -> i64 { return owner->[0]; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(optional_raw.hir.is_none());
    assert!(
        optional_raw
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == crate::typeck::INVALID_SHARED_CONVERSION }),
        "{:?}",
        optional_raw.diagnostics
    );

    for assignment in ["Item()[0] = 1;", "Item()[:] = 1;"] {
        let output = check_text(&format!(
            "class Item {{ init() {{}} mut fn index_set(key: i64, value: i64) -> unit {{}} mut fn slice_set(start: i64?, end: i64?, value: i64) -> unit {{}} }} fn use() -> unit {{ {assignment} }} fn main() -> i64 {{ return 0; }}"
        ));
        assert!(output.hir.is_none());
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == crate::typeck::READ_ONLY_RECEIVER }),
            "{:?}",
            output.diagnostics
        );
    }
}

#[test]
fn checks_structural_slicing_on_a_closed_generic_class() {
    let hir = check_generic_source(concat!(
        "class Window<R, W> {\n",
        "  result: R;\n",
        "  init(result: R) { self.result = result; }\n",
        "  fn slice_get(start: i64?, end: i64?) -> R { return self.result; }\n",
        "  mut fn slice_set(start: i64?, end: i64?, replacement: W) -> unit {}\n",
        "}\n",
        "fn use(mut ref value: Window<bool, u8>) -> bool { value[:] = 1u8; return value[:]; }\n",
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
        .expect("closed generic structural slice function must exist");
    assert!(matches!(
        definition.body.statements[0],
        HirStatement::Call(_)
    ));
    assert!(matches!(
        returned_expression(definition).kind,
        HirExpressionKind::MethodCall { .. }
    ));
}
