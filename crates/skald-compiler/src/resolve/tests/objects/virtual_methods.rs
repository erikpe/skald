use super::*;
use crate::{
    identity::{MethodId, VirtualFamilyId, VirtualSlotId},
    resolve::{ResolvedMethodDispatch, INHERITED_MEMBER_COLLISION, INVALID_OVERRIDE},
};

#[test]
fn forward_declared_deep_overrides_share_one_stable_family() {
    let output = resolve_text(concat!(
        "class Leaf extends Middle {\n",
        "  override fn speak(value: i64) -> i64 { return value; }\n",
        "}\n",
        "class Root {\n",
        "  virtual fn speak(value: i64) -> i64 { return value; }\n",
        "}\n",
        "class Middle extends Root {\n",
        "  override fn speak(value: i64) -> i64 { return value; }\n",
        "}\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let family = output
        .program
        .virtual_families
        .get(VirtualFamilyId::new(0))
        .expect("virtual root must allocate a family");
    let root = MethodId::new(ClassId::new(1), 0);
    let middle = MethodId::new(ClassId::new(2), 0);
    let leaf = MethodId::new(ClassId::new(0), 0);
    assert_eq!(family.root, root);
    assert_eq!(family.slot, VirtualSlotId::new(0));
    assert_eq!(
        class(&output, 1).methods[0].dispatch,
        ResolvedMethodDispatch::VirtualRoot {
            family: VirtualFamilyId::new(0),
            slot: VirtualSlotId::new(0),
        }
    );
    assert_eq!(
        class(&output, 2).methods[0].dispatch,
        ResolvedMethodDispatch::Override {
            family: VirtualFamilyId::new(0),
            slot: VirtualSlotId::new(0),
            root,
            overridden: root,
        }
    );
    assert_eq!(
        class(&output, 0).methods[0].dispatch,
        ResolvedMethodDispatch::Override {
            family: VirtualFamilyId::new(0),
            slot: VirtualSlotId::new(0),
            root,
            overridden: middle,
        }
    );
    assert_eq!(leaf, class(&output, 0).methods[0].id);
}

#[test]
fn invalid_overrides_and_implicit_redeclarations_have_stable_precedence() {
    let output = resolve_text(concat!(
        "class Base {\n",
        "  field: i64;\n",
        "  fn direct() -> unit {}\n",
        "  virtual fn dynamic() -> unit {}\n",
        "}\n",
        "class Derived extends Base {\n",
        "  override fn missing() -> unit {}\n",
        "  override fn field() -> unit {}\n",
        "  override fn direct() -> unit {}\n",
        "  fn dynamic() -> unit {}\n",
        "}\n",
    ));

    assert_eq!(
        output
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        [
            INVALID_OVERRIDE,
            INVALID_OVERRIDE,
            INVALID_OVERRIDE,
            INHERITED_MEMBER_COLLISION,
        ]
    );
    assert!(output.program.virtual_families.len() == 1);
    assert!(class(&output, 1)
        .methods
        .iter()
        .all(|method| matches!(method.dispatch, ResolvedMethodDispatch::Direct)));
}

#[test]
fn resolved_virtual_declaration_dump_is_exact_and_identity_based() {
    let output = resolve_text(concat!(
        "class Base { virtual fn read() -> i64 { return 1; } }\n",
        "class Derived extends Base { override fn read() -> i64 { return 2; } }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let dump = dump_resolved(&output.program);
    let relevant_lines = dump
        .lines()
        .filter(|line| {
            line.contains("Method ") || line.contains("Dispatch ") || line.contains("Family ")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        relevant_lines,
        [
            "        Method c0:method0 readonly \"read\" @13..51",
            "          Dispatch VirtualRoot vf0 slot vs0",
            "        Method c1:method0 readonly \"read\" @83..122",
            "          Dispatch Override vf0 slot vs0 root c0:method0 overridden c0:method0",
            "    Family vf0 slot vs0 root c0:method0",
        ]
    );
}
