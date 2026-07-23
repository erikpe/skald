use super::*;
use crate::{
    hir::HirMethodDispatch,
    identity::{ClassId, MethodId, VirtualFamilyId, VirtualSlotId},
    typeck::INVALID_OVERRIDE_SIGNATURE,
};

#[test]
fn exact_override_signatures_allow_different_parameter_names() {
    let output = check_text(concat!(
        "class Value { init() {} }\n",
        "class Base {\n",
        "  init() {}\n",
        "  virtual fn inspect(ref original: Value, count: i64) -> bool { return true; }\n",
        "}\n",
        "class Derived extends Base {\n",
        "  init() { super(); }\n",
        "  override fn inspect(ref renamed: Value, amount: i64) -> bool { return false; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let hir = output.hir.expect("valid overrides must produce HIR");
    let root = MethodId::new(ClassId::new(1), 0);
    assert_eq!(
        hir.classes.get(ClassId::new(1)).unwrap().methods[0].dispatch,
        HirMethodDispatch::VirtualRoot {
            family: VirtualFamilyId::new(0),
            slot: VirtualSlotId::new(0),
        }
    );
    assert_eq!(
        hir.classes.get(ClassId::new(2)).unwrap().methods[0].dispatch,
        HirMethodDispatch::Override {
            family: VirtualFamilyId::new(0),
            slot: VirtualSlotId::new(0),
            root,
            overridden: root,
        }
    );
}

#[test]
fn override_signature_diagnostics_follow_declaration_order_and_one_rule_per_method() {
    let output = check_text(concat!(
        "class Value { init() {} }\n",
        "class Base {\n",
        "  init() {}\n",
        "  virtual mut fn receiver() -> unit {}\n",
        "  virtual fn count(value: i64) -> unit {}\n",
        "  virtual fn mode(ref value: Value) -> unit {}\n",
        "  virtual fn parameter(value: i64) -> unit {}\n",
        "  virtual fn result() -> i64 { return 0; }\n",
        "}\n",
        "class Derived extends Base {\n",
        "  init() { super(); }\n",
        "  override fn receiver() -> unit {}\n",
        "  override fn count() -> unit {}\n",
        "  override fn mode(value: Value) -> unit {}\n",
        "  override fn parameter(value: bool) -> unit {}\n",
        "  override fn result() -> bool { return false; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert_eq!(
        output
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        [INVALID_OVERRIDE_SIGNATURE; 5]
    );
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.notes.len() == 1));
}

#[test]
fn hir_virtual_declaration_dump_is_exact_and_identity_based() {
    let output = check_text(concat!(
        "class Base { init() {} virtual fn read() -> i64 { return 1; } }\n",
        "class Derived extends Base { init() { super(); } override fn read() -> i64 { return 2; } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let dump = dump_hir(output.hir.as_ref().unwrap());
    let relevant_lines = dump
        .lines()
        .filter(|line| {
            line.contains("Method ") || line.contains("Dispatch ") || line.contains("Family ")
        })
        .map(|line| line.split(" @").next().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        relevant_lines,
        [
            "        Method c0:method0 \"read\" readonly -> i64",
            "          Dispatch VirtualRoot vf0 slot vs0",
            "        Method c1:method0 \"read\" readonly -> i64",
            "          Dispatch Override vf0 slot vs0 root c0:method0 overridden c0:method0",
            "    Family vf0 slot vs0 root c0:method0",
        ]
    );
}
