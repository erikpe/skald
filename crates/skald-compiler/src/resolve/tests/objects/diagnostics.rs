use super::*;

#[test]
fn diagnoses_invalid_intermediate_and_terminal_nested_members() {
    let output = resolve_text(concat!(
        "class Leaf { value: i64; init(value: i64) { self.value = value; } fn read() -> i64 { return self.value; } }\n",
        "class Root { child: Leaf; init() {} }\n",
        "fn first(ref root: Root) -> i64 { return root.child.value.missing; }\n",
        "fn second(ref root: Root) -> i64 { return root.child.missing; }\n",
        "fn third(ref root: Root) -> i64 { return root.child.read.value; }\n",
        "fn fourth(ref root: Root) -> i64 { return root.child.value(); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let codes: Vec<_> = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(
        codes,
        [
            INVALID_MEMBER_SELECTION,
            UNKNOWN_MEMBER,
            INVALID_MEMBER_SELECTION,
            INVALID_CALL_TARGET,
        ]
    );
    assert!(output.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("cannot be used as an object place")));
}

#[test]
fn diagnoses_unknown_and_non_type_named_field_types() {
    let unknown = resolve_text(concat!(
        "class Holder { value: Missing; init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert_eq!(unknown.diagnostics.len(), 1);
    assert_eq!(
        unknown.diagnostics.iter().next().unwrap().code,
        UNKNOWN_TYPE
    );

    let function = resolve_text(concat!(
        "fn NotAClass() -> i64 { return 0; }\n",
        "class Holder { value: NotAClass; init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert_eq!(function.diagnostics.len(), 1);
    let diagnostic = function.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, UNKNOWN_TYPE);
    assert!(diagnostic.message.contains("does not name a type"));
}

#[test]
fn diagnoses_unknown_types_members_and_wrong_owner_selection() {
    let unknown_type = resolve_text("fn main() -> i64 { var missing: Missing = 0; return 0; }");
    assert_eq!(unknown_type.diagnostics.len(), 1);
    assert_eq!(
        unknown_type.diagnostics.iter().next().unwrap().code,
        UNKNOWN_TYPE
    );

    let output = resolve_text(concat!(
        "class Left { left: i64; init() { self.left = 0; } }\n",
        "class Right { right: i64; init() { self.right = 0; } fn wrong() -> i64 { return self.left; } }\n",
        "fn main() -> i64 { var value: Left = Left(); return value.missing; }\n",
    ));
    assert_eq!(output.diagnostics.len(), 2);
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == UNKNOWN_MEMBER));
}

#[test]
fn self_is_scoped_to_initializers_and_methods() {
    let output = resolve_text("fn main() -> i64 { return self.value; }");
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        SELF_OUTSIDE_MEMBER
    );
}

#[test]
fn local_bindings_shadow_callable_class_names_but_not_type_names() {
    let output = resolve_text(concat!(
        "class Counter { init() {} }\n",
        "fn main() -> i64 {\n",
        "    var Counter: i64 = 0;\n",
        "    var value: Counter = Counter();\n",
        "    return 0;\n",
        "}\n",
    ));

    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        INVALID_CALL_TARGET
    );
    let main = output
        .program
        .definitions
        .get(output.program.entry_function.unwrap())
        .unwrap();
    assert_eq!(
        main.locals[1].type_syntax.kind,
        ResolvedTypeKind::Class(ClassId::new(0))
    );
}

#[test]
fn diagnoses_invalid_member_kinds_receivers_and_missing_initializers() {
    let output = resolve_text(concat!(
        "class Empty {}\n",
        "class Value { field: i64; init() { self.field = 0; } fn method() -> i64 { return self.field; } }\n",
        "fn main() -> i64 {\n",
        "    var scalar: i64 = 0;\n",
        "    var value: Value = Value();\n",
        "    value.field();\n",
        "    var method: i64 = value.method;\n",
        "    var missing: Empty = Empty();\n",
        "    return scalar.field;\n",
        "}\n",
    ));

    let codes: Vec<_> = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(
        codes,
        [
            INVALID_CALL_TARGET,
            INVALID_FUNCTION_REFERENCE,
            INVALID_CONSTRUCTION_TARGET,
            INVALID_MEMBER_SELECTION,
        ]
    );
}

#[test]
fn resolves_only_first_statement_super_for_derived_ordinary_initializers() {
    let output = resolve_text(concat!(
        "class Base { init(value: i64) {} init(value: bool) {} }\n",
        "class Good extends Base { init(value: i64) { super(value); } }\n",
        "class Missing extends Base { value: i64; init() { self.value = 0; } }\n",
        "class Duplicate extends Base { init() { super(0); super(1); } }\n",
        "class Root { init() { super(); } }\n",
        "class Copy extends Base { init() { super(0); } copy(ref source: Copy) { super(0); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let diagnostics = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == INVALID_BASE_INITIALIZATION)
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 4);
    assert!(diagnostics[0].message.contains("must begin"));
    assert!(diagnostics[1]
        .message
        .contains("only as the first statement"));
    assert!(diagnostics[2]
        .message
        .contains("only as the first statement"));
    assert!(diagnostics[3]
        .message
        .contains("only as the first statement"));

    let definition = output
        .program
        .class_definitions
        .get(ClassId::new(1))
        .unwrap();
    let ResolvedStatement::BaseInitialization(base) =
        &definition.initializers[0].body.statements[0]
    else {
        panic!("expected resolved base initialization");
    };
    assert_eq!(base.base, ClassId::new(0));
    assert_eq!(base.arguments.len(), 1);
    let dump = dump_resolved(&output.program);
    assert!(dump.contains("BaseInitialization c0"));
    assert!(!dump.contains("BaseInitialization c0 c0:init"));
}

#[test]
fn diagnoses_super_when_the_direct_base_has_no_initializer() {
    let output = resolve_text(concat!(
        "class Base {}\n",
        "class Derived extends Base { init() { super(); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == INVALID_BASE_INITIALIZATION)
        .expect("missing base initializer must be diagnosed");
    assert!(diagnostic.message.contains("has no ordinary initializer"));
}

#[test]
fn rejects_the_copy_marker_for_non_class_calls() {
    let output = resolve_text(concat!(
        "class Value { init() {} }\n",
        "fn consume(ref value: Value) -> i64 { return 0; }\n",
        "fn main() -> i64 {\n",
        "  var value: Value = Value();\n",
        "  return consume(copy value);\n",
        "}\n",
    ));

    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == INVALID_CONSTRUCTION_TARGET)
        .expect("copy marker on a function call must be rejected");
    assert!(diagnostic.message.contains("concrete class"));
    assert!(diagnostic.labels[0].message.contains("marker"));
}
