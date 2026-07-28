use super::*;
use crate::{
    hir::HirMethodKind,
    identity::{ClassId, MethodId},
    test_support::type_check_internal_static_methods,
};

#[test]
fn checks_private_ready_static_bodies_with_class_ownership_but_no_receiver() {
    let hir = type_check_internal_static_methods(concat!(
        "class Tools {\n",
        "  init() {}\n",
        "  private fn answer(value: i64) -> i64 { return value + 1; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let method = &hir.classes.get(ClassId::new(0)).unwrap().methods[0];
    assert_eq!(method.kind, HirMethodKind::Static);
    let definition = hir
        .member_definition(MethodId::new(ClassId::new(0), 0).into())
        .unwrap();
    assert_eq!(definition.class_owner, ClassId::new(0));
    assert_eq!(definition.receiver_class, None);
    assert!(definition.locals.iter().all(|local| local.name != "self"));

    let dump = dump_hir(&hir);
    assert!(dump.contains("Method c0:method0 \"answer\" static"));
    assert!(!dump.contains("Receiver c0:method0"));
}

#[test]
fn checks_static_signatures_across_supported_parameter_and_result_categories() {
    let hir = type_check_internal_static_methods(concat!(
        "class Tools {\n",
        "  init() {}\n",
        "  fn primitive(value: i64) -> i64 { return value; }\n",
        "  fn notify(value: bool) -> unit { return; }\n",
        "  fn object(value: Payload) -> Payload { return value; }\n",
        "  fn shared_value(value: shared Payload) -> shared Payload { return value; }\n",
        "  fn optional_value(value: i64?) -> i64? { return value; }\n",
        "  fn optional_owner(value: shared? Payload) -> shared? Payload { return value; }\n",
        "  fn array_value(value: i64[]) -> i64[] { return value; }\n",
        "  fn alias(ref value: Payload) -> i64 { return 1; }\n",
        "}\n",
        "class Payload { init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

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
