use super::*;

#[test]
fn enforces_read_only_and_mutable_receiver_access() {
    let output = check_text(concat!(
        "class Value {\n",
        "  field: i64;\n",
        "  init() { self.field = 0; }\n",
        "  mut fn set(value: i64) -> unit { self.field = value; }\n",
        "  fn write() -> unit { self.field = 1; }\n",
        "  fn forward() -> unit { self.set(2); }\n",
        "}\n",
        "fn main() -> i64 { var value: Value = Value(); value.set(3); return value.field; }\n",
    ));

    assert!(output.hir.is_none());
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == READ_ONLY_RECEIVER)
        .collect();
    assert_eq!(errors.len(), 2);
}

#[test]
fn initializer_cannot_call_a_method_before_the_receiver_is_live() {
    let output = check_text(concat!(
        "class Value {\n",
        "  field: i64;\n",
        "  init() { self.field = self.get(); }\n",
        "  fn get() -> i64 { return 1; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_INITIALIZER_BODY));
}

#[test]
fn methods_reuse_structured_definite_return_analysis() {
    let output = check_text(concat!(
        "class Value {\n",
        "  init() {}\n",
        "  fn complete(flag: bool) -> i64 { if (flag) { return 1; } else { return 2; } }\n",
        "  fn missing(flag: bool) -> i64 { if (flag) { return 1; } }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    let missing: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == MISSING_RETURN)
        .collect();
    assert_eq!(missing.len(), 1);
    assert!(missing[0].message.contains("method `missing`"));
}

#[test]
fn lowers_alias_signatures_for_every_internal_owner() {
    let output = check_text(concat!(
        "class Thing {\n",
        "  init(ref other: Other) {}\n",
        "  fn inspect(mut ref other: Thing) -> unit {}\n",
        "}\n",
        "class Other { init() {} }\n",
        "fn take(ref thing: Thing) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    assert_eq!(
        hir.declarations.get(FunctionId::new(0)).unwrap().parameters[0].mode,
        crate::hir::HirParameterMode::ReadOnlyAlias
    );
    let class = hir.class(ClassId::new(0)).unwrap();
    assert_eq!(
        class.initializer.parameters[0].mode,
        crate::hir::HirParameterMode::ReadOnlyAlias
    );
    assert_eq!(
        class.methods[0].parameters[0].mode,
        crate::hir::HirParameterMode::MutableAlias
    );
}
