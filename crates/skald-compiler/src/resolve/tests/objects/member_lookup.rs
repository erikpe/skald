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
