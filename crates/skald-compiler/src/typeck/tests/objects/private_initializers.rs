use super::*;
use crate::typeck::{AMBIGUOUS_INITIALIZER, NO_MATCHING_INITIALIZER, PRIVATE_INITIALIZER_ACCESS};

#[test]
fn every_exact_class_body_can_use_a_private_initializer() {
    let output = check_text(concat!(
        "class Secret {\n",
        "  private child: shared? Secret;\n",
        "  private init() { self.child = none; }\n",
        "  init(value: i64) { self.child = new Secret(); }\n",
        "  copy(ref source: Secret) { self.child = new Secret(); }\n",
        "  assign(ref source: Secret) { self.child = new Secret(); }\n",
        "  destroy { self.child = new Secret(); }\n",
        "  fn duplicate() -> Secret { return Secret(); }\n",
        "  static fn make() -> Secret { return Secret(); }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = dump_hir(output.hir.as_ref().unwrap());
    assert!(!hir.contains("private"));
    assert!(!hir.contains("Private"));
}

#[test]
fn exact_class_construction_paths_can_select_a_private_initializer() {
    let output = check_text(concat!(
        "class Secret {\n",
        "  private init() {}\n",
        "  static fn exercise() -> unit {\n",
        "    var direct: Secret = Secret();\n",
        "    var allocated: shared Secret = new Secret();\n",
        "    var inline: Secret[] = Secret[](2u);\n",
        "    var owners: (shared Secret)[] = (shared Secret)[](2u);\n",
        "    return;\n",
        "  }\n",
        "  static fn produce() -> Secret { return Secret(); }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
}

#[test]
fn foreign_construction_paths_share_the_private_initializer_diagnostic() {
    let output = check_text(concat!(
        "class Secret {\n",
        "  private init() {}\n",
        "}\n",
        "class Holder {\n",
        "  secret: Secret;\n",
        "  init() { self.secret = Secret(); }\n",
        "  fn duplicate() -> Secret { return Secret(); }\n",
        "}\n",
        "fn produce() -> Secret { return Secret(); }\n",
        "fn main() -> i64 {\n",
        "  var direct: Secret = Secret();\n",
        "  var allocated: shared Secret = new Secret();\n",
        "  var inline: Secret[] = Secret[](2u);\n",
        "  var owners: (shared Secret)[] = (shared Secret)[](2u);\n",
        "  return 0;\n",
        "}\n",
    ));

    let diagnostics = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == PRIVATE_INITIALIZER_ACCESS)
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 7);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.labels.len() == 2
            && diagnostic.labels[0].message == "private initializer is not accessible here"
            && diagnostic.labels[1].message == "declared private here"
            && diagnostic
                .notes
                .iter()
                .any(|note| note.contains("only inside the declaring class"))
    }));
    assert!(output.hir.is_none());
}

#[test]
fn derived_super_calls_cannot_use_private_base_initializers() {
    let output = check_text(concat!(
        "class Base { private init() {} }\n",
        "class Derived extends Base { init() { super(); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == PRIVATE_INITIALIZER_ACCESS)
            .count(),
        1
    );
    assert!(output.hir.is_none());
}

#[test]
fn access_is_checked_after_selecting_the_most_specific_overload() {
    let output = check_text(concat!(
        "interface Named {}\n",
        "class Key implements Named { init() {} }\n",
        "class Choice {\n",
        "  init(ref value: Obj) {}\n",
        "  private init(ref value: Named) {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var key: Key = Key();\n",
        "  var choice: Choice = Choice(key);\n",
        "  return 0;\n",
        "}\n",
    ));

    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == PRIVATE_INITIALIZER_ACCESS)
            .count(),
        1
    );
    assert!(!output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == NO_MATCHING_INITIALIZER
            || diagnostic.code == AMBIGUOUS_INITIALIZER));
}

#[test]
fn public_overloads_remain_callable_beside_private_overloads() {
    let output = check_text(concat!(
        "class Choice {\n",
        "  init(value: i64) {}\n",
        "  private init(flag: bool) {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var choice: Choice = Choice(42);\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
}

#[test]
fn no_match_and_ambiguity_take_precedence_over_private_access() {
    let output = check_text(concat!(
        "interface Left {}\n",
        "interface Right {}\n",
        "class Both implements Left, Right { init() {} }\n",
        "class Choice {\n",
        "  private init(ref value: Left) {}\n",
        "  private init(ref value: Right) {}\n",
        "  private init(value: i64) {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var both: Both = Both();\n",
        "  var ambiguous: Choice = Choice(both);\n",
        "  var missing: Choice = Choice(true);\n",
        "  return 0;\n",
        "}\n",
    ));

    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == AMBIGUOUS_INITIALIZER)
            .count(),
        1
    );
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == NO_MATCHING_INITIALIZER)
            .count(),
        1
    );
    assert!(!output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == PRIVATE_INITIALIZER_ACCESS));
}

#[test]
fn empty_and_explicit_copy_arrays_do_not_authorize_initializers() {
    let output = check_text(concat!(
        "class Item { private init() {} }\n",
        "fn main() -> i64 {\n",
        "  var empty: Item[] = Item[]();\n",
        "  var copied: Item[] = Item[](copy empty);\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
}

#[test]
fn resolved_visibility_mutations_change_access_without_changing_hir_shape() {
    let mut program = resolve_text(concat!(
        "class Choice {\n",
        "  init(value: i64) {}\n",
        "  init(flag: bool) {}\n",
        "}\n",
        "fn main() -> i64 { var choice: Choice = Choice(42); return 0; }\n",
    ));
    let public = crate::typeck::type_check(&program);
    assert!(!public.has_errors(), "{:?}", public.diagnostics);
    let public_hir = dump_hir(public.hir.as_ref().unwrap());

    let class = program.classes.get_mut(ClassId::new(0)).unwrap();
    class.initializers[1].visibility = crate::resolve::ResolvedMemberVisibility::Private {
        span: class.initializers[1].span,
    };
    let unused_private = crate::typeck::type_check(&program);
    assert!(
        !unused_private.has_errors(),
        "{:?}",
        unused_private.diagnostics
    );
    assert_eq!(public_hir, dump_hir(unused_private.hir.as_ref().unwrap()));

    let class = program.classes.get_mut(ClassId::new(0)).unwrap();
    class.initializers[0].visibility = crate::resolve::ResolvedMemberVisibility::Private {
        span: class.initializers[0].span,
    };
    let selected_private = crate::typeck::type_check(&program);
    assert_eq!(
        selected_private
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == PRIVATE_INITIALIZER_ACCESS)
            .count(),
        1
    );
    assert!(selected_private.hir.is_none());
}
