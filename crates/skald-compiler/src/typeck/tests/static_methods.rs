use super::*;
use crate::{
    hir::{HirExpressionKind, HirMethodKind},
    identity::{ClassId, MethodId},
};

#[test]
fn checks_private_ready_static_bodies_with_class_ownership_but_no_receiver() {
    let output = check_text(concat!(
        "class Tools {\n",
        "  init() {}\n",
        "  private static fn increment(value: i64) -> i64 { return value + 1; }\n",
        "  static fn answer(value: i64) -> i64 { return Tools.increment(value); }\n",
        "}\n",
        "fn main() -> i64 { return Tools.answer(41); }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();

    let method = &hir.classes.get(ClassId::new(0)).unwrap().methods[0];
    assert_eq!(method.kind, HirMethodKind::Static);
    let definition = hir
        .member_definition(MethodId::new(ClassId::new(0), 0).into())
        .unwrap();
    assert_eq!(definition.class_owner, ClassId::new(0));
    assert_eq!(definition.receiver_class, None);
    assert!(definition.locals.iter().all(|local| local.name != "self"));

    let dump = dump_hir(&hir);
    assert!(dump.contains("Method c0:method0 \"increment\" static"));
    assert!(dump.contains("Method c0:method1 \"answer\" static"));
    assert!(dump.contains("StaticCall c0:method0"));
    assert!(dump.contains("StaticCall c0:method1"));
    assert!(!dump.contains("Receiver c0:method0"));

    let main = hir.definitions.get(hir.entry_function).unwrap();
    assert!(matches!(
        returned_expression(main).kind,
        HirExpressionKind::StaticCall { method, .. }
            if method == MethodId::new(ClassId::new(0), 1)
    ));
}

#[test]
fn checks_static_signatures_across_supported_parameter_and_result_categories() {
    let output = check_text(concat!(
        "class Tools {\n",
        "  init() {}\n",
        "  static fn primitive(value: i64) -> i64 { return value; }\n",
        "  static fn notify(value: bool) -> unit { return; }\n",
        "  static fn object(value: Payload) -> Payload { return value; }\n",
        "  static fn shared_value(value: shared Payload) -> shared Payload { return value; }\n",
        "  static fn optional_value(value: i64?) -> i64? { return value; }\n",
        "  static fn optional_owner(value: shared? Payload) -> shared? Payload { return value; }\n",
        "  static fn array_value(value: i64[]) -> i64[] { return value; }\n",
        "  static fn alias(ref value: Payload) -> i64 { return 1; }\n",
        "}\n",
        "class Payload { init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();

    let class = hir.classes.get(ClassId::new(0)).unwrap();
    assert_eq!(class.methods.len(), 8);
    assert!(class
        .methods
        .iter()
        .all(|method| method.kind == HirMethodKind::Static));
    let definitions = &hir.class_definitions.get(ClassId::new(0)).unwrap().methods;
    assert_eq!(definitions.len(), class.methods.len());
    assert!(definitions
        .iter()
        .all(|definition| definition.receiver_class.is_none()));
}

#[test]
fn static_methods_do_not_satisfy_interface_requirements() {
    let output = check_text(concat!(
        "interface Reader { fn read() -> i64; }\n",
        "class Tools implements Reader {\n",
        "  init() {}\n",
        "  static fn read() -> i64 { return 1; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.message == "static method `read` cannot implement `Reader.read`"
    }));
}
