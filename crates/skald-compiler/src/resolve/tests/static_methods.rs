use super::*;

#[test]
fn resolves_static_declarations_calls_inheritance_and_local_shadowing_by_identity() {
    let output = resolve_text(concat!(
        "class Base {\n",
        "  init() {}\n",
        "  static fn answer(value: i64) -> i64 { return value; }\n",
        "}\n",
        "class Derived extends Base { init() { super(); } }\n",
        "class Local {\n",
        "  init() {}\n",
        "  fn answer(value: i64) -> i64 { return value + 1; }\n",
        "}\n",
        "fn inherited() -> i64 { return Derived.answer(41); }\n",
        "fn shadowed() -> i64 {\n",
        "  var Derived: Local = Local();\n",
        "  return Derived.answer(41);\n",
        "}\n",
        "fn main() -> i64 { return inherited() + shadowed(); }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let method = &output.program.classes.get(ClassId::new(0)).unwrap().methods[0];
    assert_eq!(method.kind, ResolvedMethodKind::Static);

    let inherited = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedExpression::StaticCall(call) =
        return_value(inherited.body.statements.last().unwrap())
    else {
        panic!("expected inherited class-selected static call");
    };
    assert_eq!(call.method, MethodId::new(ClassId::new(0), 0));

    let shadowed = output.program.definitions.get(FunctionId::new(1)).unwrap();
    let ResolvedExpression::MethodCall(call) =
        return_value(shadowed.body.statements.last().unwrap())
    else {
        panic!("local class-name shadow must retain object method selection");
    };
    assert_eq!(call.method, MethodId::new(ClassId::new(2), 0));

    let dump = dump_resolved(&output.program);
    assert!(dump.contains("Method c0:method0 static \"answer\""));
    assert!(dump.contains("StaticCall c0:method0"));
}

#[test]
fn static_bodies_have_private_class_access_but_no_self() {
    let allowed = resolve_text(concat!(
        "class Secret {\n",
        "  private value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  private fn read() -> i64 { return self.value; }\n",
        "  private static fn inspect(ref value: Secret) -> i64 { return value.read(); }\n",
        "  static fn relay(ref value: Secret) -> i64 { return Secret.inspect(value); }\n",
        "}\n",
        "fn main() -> i64 { var value: Secret = Secret(42); return Secret.relay(value); }\n",
    ));
    assert!(!allowed.has_errors(), "{:?}", allowed.diagnostics);

    let invalid_self = resolve_text(concat!(
        "class Secret {\n",
        "  init() {}\n",
        "  static fn invalid() -> i64 { return self.invalid(); }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(invalid_self
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == SELF_OUTSIDE_MEMBER));
}

#[test]
fn diagnoses_wrong_selection_kinds_static_values_and_private_static_access() {
    let output = resolve_text(concat!(
        "class Tools {\n",
        "  value: i64;\n",
        "  init() { self.value = 0; }\n",
        "  fn instance() -> i64 { return self.value; }\n",
        "  static fn utility() -> i64 { return 1; }\n",
        "  private static fn hidden() -> i64 { return 2; }\n",
        "}\n",
        "fn class_instance() -> i64 { return Tools.instance(); }\n",
        "fn object_static() -> i64 { var value: Tools = Tools(); return value.utility(); }\n",
        "fn class_field() -> i64 { return Tools.value(); }\n",
        "fn static_value() -> i64 { Tools.utility; return 0; }\n",
        "fn private_static() -> i64 { return Tools.hidden(); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let messages = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(messages.contains(&"instance method `instance` requires an object receiver"));
    assert!(messages.contains(&"static method `utility` must be called through a class"));
    assert!(messages.contains(&"field `value` requires an object receiver"));
    assert!(messages.contains(&"static method `utility` cannot be used as a value"));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == PRIVATE_MEMBER_ACCESS));
}

#[test]
fn inherited_private_static_methods_remain_selected_but_inaccessible() {
    let output = resolve_text(concat!(
        "class Base {\n",
        "  init() {}\n",
        "  private static fn secret() -> i64 { return 1; }\n",
        "}\n",
        "class Derived extends Base {\n",
        "  init() { super(); }\n",
        "  static fn invalid() -> i64 { return Derived.secret(); }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == PRIVATE_MEMBER_ACCESS)
            .count(),
        1
    );
}
