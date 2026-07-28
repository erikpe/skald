use super::*;

#[test]
fn top_level_and_ordinary_member_namespaces_reject_cross_kind_duplicates() {
    let output = resolve_text(concat!(
        "class Same { init() {} }\n",
        "fn Same() -> unit {}\n",
        "class Members {\n",
        "    value: i64;\n",
        "    fn value() -> i64 { return 0; }\n",
        "    init() {}\n",
        "    init(value: i64) {}\n",
        "    fn get() -> i64 { return 1; }\n",
        "    fn get(value: i64) -> i64 { return value; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let codes: Vec<_> = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(
        codes,
        [DUPLICATE_TOP_LEVEL, DUPLICATE_MEMBER, DUPLICATE_MEMBER,]
    );
    assert_eq!(output.program.declarations.len(), 1);
    assert_eq!(output.program.classes.len(), 2);
    let members = class(&output, 1);
    assert_eq!(members.fields.len(), 1);
    assert!(members.methods.iter().any(|method| method.name == "get"));
    assert!(!members.methods.iter().any(|method| method.name == "value"));
}

#[test]
fn identical_member_names_are_independent_between_owners() {
    let output = resolve_text(concat!(
        "class Left { value: i64; init(value: i64) { self.value = value; } fn get() -> i64 { return self.value; } }\n",
        "class Right { value: i64; init(value: i64) { self.value = value; } fn get() -> i64 { return self.value; } }\n",
        "fn main() -> i64 { var value: Right = Right(1); return value.get(); }\n",
    ));

    assert!(!output.has_errors());
    assert_eq!(class(&output, 0).fields[0].id.class(), ClassId::new(0));
    assert_eq!(class(&output, 1).fields[0].id.class(), ClassId::new(1));
}

#[test]
fn private_members_are_visible_in_every_declaring_class_body_kind() {
    let output = resolve_text(concat!(
        "class Secret {\n",
        "  private value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Secret) { self.value = source.value; }\n",
        "  assign(ref source: Secret) { self.value = source.value; }\n",
        "  destroy { self.value = 0; }\n",
        "  private mut fn set(value: i64) -> unit { self.value = value; }\n",
        "  private fn read(ref other: Secret) -> i64 { return other.value; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let secret = class(&output, 0);
    assert!(matches!(
        secret.fields[0].visibility,
        ResolvedMemberVisibility::Private { .. }
    ));
    assert!(secret
        .methods
        .iter()
        .all(|method| method.visibility.private_span().is_some()));
    let dump = dump_resolved(&output.program);
    assert!(dump.contains("Field c0:field0 private \"value\""));
    assert!(dump.contains("Method c0:method0 mutable private \"set\""));
    assert!(dump.contains("Method c0:method1 readonly private \"read\""));
    assert_eq!(
        dump.lines()
            .filter(|line| line.trim_start().starts_with("Private @"))
            .count(),
        3
    );
}

#[test]
fn private_access_uses_the_selected_declaring_class_and_has_stable_precedence() {
    let output = resolve_text(concat!(
        "class Secret {\n",
        "  private value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  private fn read() -> i64 { return self.value; }\n",
        "}\n",
        "class Derived extends Secret {\n",
        "  init(value: i64) { super(value); }\n",
        "  fn inherited() -> i64 { return self.value; }\n",
        "}\n",
        "class Other {\n",
        "  init(ref secret: Secret) { secret.read(); }\n",
        "  fn read(ref secret: Secret) -> i64 { return secret.value; }\n",
        "}\n",
        "fn inspect(ref secret: Secret) -> i64 { secret.value(); return secret.missing; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_eq!(
        output
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        [
            PRIVATE_MEMBER_ACCESS,
            UNKNOWN_MEMBER,
            PRIVATE_MEMBER_ACCESS,
            PRIVATE_MEMBER_ACCESS,
            PRIVATE_MEMBER_ACCESS,
        ]
    );
    assert!(output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == PRIVATE_MEMBER_ACCESS)
        .all(|diagnostic| diagnostic.message.contains("private to class `Secret`")));
    assert!(!output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_MEMBER_SELECTION));
}

#[test]
fn inherited_private_members_still_block_redeclaration() {
    let output = resolve_text(concat!(
        "class Base { private value: i64; init(value: i64) { self.value = value; } }\n",
        "class Derived extends Base {\n",
        "  value: i64;\n",
        "  init(value: i64) { super(value); self.value = value; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_eq!(
        output
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        [INHERITED_MEMBER_COLLISION]
    );
}
