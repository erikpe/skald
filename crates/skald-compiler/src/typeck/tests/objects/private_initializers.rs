use super::*;
use crate::typeck::{AMBIGUOUS_INITIALIZER, NO_MATCHING_INITIALIZER, PRIVATE_INITIALIZER_ACCESS};

fn check_with_private_initializers(
    source: &str,
    initializers: &[InitializerId],
) -> crate::typeck::TypeCheckOutput {
    let mut program = resolve_text(source);
    for initializer in initializers {
        make_initializer_private(&mut program, *initializer);
    }
    crate::typeck::type_check(&program)
}

#[test]
fn exact_class_bodies_can_use_private_initializers_without_typed_visibility() {
    let source = concat!(
        "class Secret {\n",
        "  init() {}\n",
        "  static fn make() -> Secret { return Secret(); }\n",
        "  static fn allocate() -> shared Secret { return new Secret(); }\n",
        "  fn duplicate() -> Secret { return Secret(); }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let program = resolve_text(source);
    let public = crate::typeck::type_check(&program);
    assert!(!public.has_errors(), "{:?}", public.diagnostics);

    let mut private_program = program;
    make_initializer_private(&mut private_program, InitializerId::new(ClassId::new(0), 0));
    let private = crate::typeck::type_check(&private_program);
    assert!(!private.has_errors(), "{:?}", private.diagnostics);

    assert_eq!(
        dump_hir(public.hir.as_ref().unwrap()),
        dump_hir(private.hir.as_ref().unwrap())
    );
    assert!(!dump_hir(private.hir.as_ref().unwrap()).contains("Private"));
}

#[test]
fn direct_allocation_and_base_calls_share_the_private_initializer_diagnostic() {
    let output = check_with_private_initializers(
        concat!(
            "class Base { init() {} }\n",
            "class Direct { init() {} }\n",
            "class Derived extends Base { init() { super(); } }\n",
            "fn main() -> i64 {\n",
            "  var direct: Direct = Direct();\n",
            "  var allocated: shared Direct = new Direct();\n",
            "  return 0;\n",
            "}\n",
        ),
        &[
            InitializerId::new(ClassId::new(0), 0),
            InitializerId::new(ClassId::new(1), 0),
        ],
    );

    let diagnostics = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == PRIVATE_INITIALIZER_ACCESS)
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.labels.len() == 2
            && diagnostic.labels[0].message.contains("not accessible here")
            && diagnostic.labels[1].message == "declared private here"
            && diagnostic
                .notes
                .iter()
                .any(|note| note.contains("only inside the declaring class"))
    }));
    assert!(output.hir.is_none());
}

#[test]
fn access_is_checked_after_selecting_the_most_specific_overload() {
    let output = check_with_private_initializers(
        concat!(
            "interface Named {}\n",
            "class Key implements Named { init() {} }\n",
            "class Choice {\n",
            "  init(ref value: Obj) {}\n",
            "  init(ref value: Named) {}\n",
            "}\n",
            "fn main() -> i64 {\n",
            "  var key: Key = Key();\n",
            "  var choice: Choice = Choice(key);\n",
            "  return 0;\n",
            "}\n",
        ),
        &[InitializerId::new(ClassId::new(1), 1)],
    );

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
fn no_match_and_ambiguity_take_precedence_over_private_access() {
    let output = check_with_private_initializers(
        concat!(
            "interface Left {}\n",
            "interface Right {}\n",
            "class Both implements Left, Right { init() {} }\n",
            "class Choice {\n",
            "  init(ref value: Left) {}\n",
            "  init(ref value: Right) {}\n",
            "  init(value: i64) {}\n",
            "}\n",
            "fn main() -> i64 {\n",
            "  var both: Both = Both();\n",
            "  var ambiguous: Choice = Choice(both);\n",
            "  var missing: Choice = Choice(true);\n",
            "  return 0;\n",
            "}\n",
        ),
        &[
            InitializerId::new(ClassId::new(1), 0),
            InitializerId::new(ClassId::new(1), 1),
            InitializerId::new(ClassId::new(1), 2),
        ],
    );

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
fn class_element_default_arrays_authorize_their_selected_initializer() {
    let rejected = check_with_private_initializers(
        concat!(
            "class Item { init() {} }\n",
            "fn main() -> i64 {\n",
            "  var inline: Item[] = Item[](2u);\n",
            "  var owners: (shared Item)[] = (shared Item)[](2u);\n",
            "  return 0;\n",
            "}\n",
        ),
        &[InitializerId::new(ClassId::new(0), 0)],
    );
    assert_eq!(
        rejected
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == PRIVATE_INITIALIZER_ACCESS)
            .count(),
        2
    );

    let accepted = check_with_private_initializers(
        concat!(
            "class Item {\n",
            "  init() {}\n",
            "  static fn arrays() -> unit {\n",
            "    var inline: Item[] = Item[](2u);\n",
            "    var owners: (shared Item)[] = (shared Item)[](2u);\n",
            "    return;\n",
            "  }\n",
            "}\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        &[InitializerId::new(ClassId::new(0), 0)],
    );
    assert!(!accepted.has_errors(), "{:?}", accepted.diagnostics);
}

#[test]
fn empty_and_explicit_copy_arrays_do_not_authorize_initializers() {
    let output = check_with_private_initializers(
        concat!(
            "class Item { init() {} }\n",
            "fn main() -> i64 {\n",
            "  var empty: Item[] = Item[]();\n",
            "  var copied: Item[] = Item[](copy empty);\n",
            "  return 0;\n",
            "}\n",
        ),
        &[InitializerId::new(ClassId::new(0), 0)],
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
}
