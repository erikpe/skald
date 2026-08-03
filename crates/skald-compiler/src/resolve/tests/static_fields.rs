use super::*;
use crate::{
    identity::{ClassId, FieldId, FunctionId, MethodId, StaticFieldId},
    resolve::{
        ResolvedClassMember, ResolvedExpression, ResolvedStatement, DUPLICATE_MEMBER,
        INHERITED_MEMBER_COLLISION, INVALID_CALL_TARGET, INVALID_MEMBER_SELECTION,
        INVALID_OVERRIDE, PRIVATE_MEMBER_ACCESS, UNKNOWN_MEMBER, UNKNOWN_NAME,
    },
    test_support::load_module_sources,
    typeck::{type_check, INVALID_INTERFACE_CONFORMANCE},
};

#[test]
fn collects_static_fields_with_independent_dense_ids_and_deterministic_dump() {
    let output = resolve_text(concat!(
        "class Sample {\n",
        "  first: i64;\n",
        "  static count: u64;\n",
        "  fn read() -> i64 { return self.first; }\n",
        "  private static enabled: bool;\n",
        "  second: u8;\n",
        "  static values: i64[];\n",
        "  init() { self.first = 0; self.second = 0u8; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let class = output.program.class(ClassId::new(0)).unwrap();
    assert_eq!(
        class
            .fields
            .iter()
            .map(|field| field.id)
            .collect::<Vec<_>>(),
        [
            FieldId::new(ClassId::new(0), 0),
            FieldId::new(ClassId::new(0), 1),
        ]
    );
    assert_eq!(
        class
            .static_fields
            .iter()
            .map(|field| field.id)
            .collect::<Vec<_>>(),
        [
            StaticFieldId::new(ClassId::new(0), 0),
            StaticFieldId::new(ClassId::new(0), 1),
            StaticFieldId::new(ClassId::new(0), 2),
        ]
    );
    assert_eq!(class.methods[0].id, MethodId::new(ClassId::new(0), 0));
    assert_eq!(
        output
            .program
            .static_field(StaticFieldId::new(ClassId::new(0), 1))
            .unwrap()
            .name,
        "enabled"
    );
    assert!(output
        .program
        .static_field(StaticFieldId::new(ClassId::new(1), 0))
        .is_none());

    let dump = dump_resolved(&output.program);
    assert!(dump.contains("StaticFields"));
    assert!(dump.contains("StaticField c0:static0 \"count\""));
    assert!(dump.contains("StaticField c0:static1 private \"enabled\""));
    assert!(dump.contains("StaticField c0:static2 \"values\""));
}

#[test]
fn shares_the_ordinary_namespace_and_retains_inherited_declaring_identity() {
    let output = resolve_text(concat!(
        "class Base { static count: u64; init() {} }\n",
        "class Derived extends Base { init() { super(); } }\n",
        "class StaticCollision extends Base { static count: u64; init() { super(); } }\n",
        "class FieldCollision extends Base { count: u64; init() { super(); } }\n",
        "class MethodCollision extends Base { fn count() -> u64 { return 0u; } init() { super(); } }\n",
        "class Direct { static same: i64; fn same() -> i64 { return 0; } init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let inherited = ResolvedClassMember::StaticField(StaticFieldId::new(ClassId::new(0), 0));
    assert_eq!(
        output.program.hierarchy.member(ClassId::new(1), "count"),
        Some(inherited)
    );
    assert_eq!(inherited.declaring_class(), ClassId::new(0));

    let inherited_collisions = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == INHERITED_MEMBER_COLLISION)
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(inherited_collisions.len(), 3);
    assert!(inherited_collisions
        .iter()
        .all(|message| message.contains("inherited static field")));
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == DUPLICATE_MEMBER)
            .count(),
        1
    );
}

#[test]
fn resolves_reads_and_writes_to_the_declaring_static_identity() {
    let output = resolve_text(concat!(
        "class Base { static count: i64; init() {} }\n",
        "class Derived extends Base { init() { super(); } }\n",
        "fn update(value: i64) -> i64 { Derived.count = value; return Base.count; }\n",
        "fn main() -> i64 { return update(1); }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let expected = StaticFieldId::new(ClassId::new(0), 0);
    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedStatement::StaticFieldAssignment(assignment) = &definition.body.statements[0]
    else {
        panic!("expected resolved static assignment");
    };
    assert_eq!(assignment.field, expected);
    let ResolvedStatement::Return(returned) = &definition.body.statements[1] else {
        panic!("expected return statement");
    };
    let ResolvedExpression::StaticFieldAccess(access) = returned.value.as_ref().unwrap() else {
        panic!("expected resolved static read");
    };
    assert_eq!(access.field, expected);

    let dump = dump_resolved(&output.program);
    assert!(dump.contains("StaticFieldAssignment c0:static0"));
    assert!(dump.contains("StaticFieldAccess c0:static0"));
}

#[test]
fn diagnoses_static_wrong_kind_privacy_unknown_call_and_shadowing_uses() {
    let output = resolve_text(concat!(
        "class State {\n",
        "  value: i64; private static secret: i64; static count: i64;\n",
        "  init() { self.value = 0; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "  static fn own_secret() -> i64 { return State.secret; }\n",
        "  static fn bare() -> i64 { return count; }\n",
        "}\n",
        "fn object_selected(state: State) -> i64 { return state.count; }\n",
        "fn shadowed(State: State) -> i64 { return State.count; }\n",
        "fn class_field() -> i64 { return State.value; }\n",
        "fn class_method() -> i64 { return State.read(); }\n",
        "fn private_use() -> i64 { return State.secret; }\n",
        "fn called_field() -> i64 { return State.count(); }\n",
        "fn unknown() -> i64 { return State.missing; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_MEMBER_SELECTION)
            .count(),
        3,
        "{:?}",
        output.diagnostics
    );
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == PRIVATE_MEMBER_ACCESS)
            .count(),
        1
    );
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_CALL_TARGET)
            .count(),
        2
    );
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == UNKNOWN_MEMBER)
            .count(),
        1
    );
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == UNKNOWN_NAME)
            .count(),
        1
    );
}

#[test]
fn resolves_module_qualified_static_uses() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import state; fn main() -> i64 { state::State.count = 1; return state::State.count; }\n",
            ),
            (
                "state.ska",
                "public class State { static count: i64; init() {} }\n",
            ),
        ],
    );
    let output = resolve_module_graph(&graph);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let checked = type_check(&output.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    assert!(checked.hir.is_some());
}

#[test]
fn resolves_static_expression_uses_and_enforces_privacy() {
    let output = resolve_text(concat!(
        "class Base {\n",
        "  private static secret: i64;\n",
        "  static count: i64;\n",
        "  static fn own_use() -> i64 { return Base.count; }\n",
        "  init() {}\n",
        "}\n",
        "class Derived extends Base {\n",
        "  override fn count() -> i64 { return 0; }\n",
        "  init() { super(); }\n",
        "}\n",
        "fn public_use() -> i64 { return Base.count; }\n",
        "fn private_use() -> i64 { return Base.secret; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_OVERRIDE
            && diagnostic.message == "method `count` cannot override an inherited static field"
    }));
    assert!(!output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_MEMBER_SELECTION));
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == PRIVATE_MEMBER_ACCESS)
            .count(),
        1
    );
}

#[test]
fn zero_default_declarations_cross_the_hir_boundary() {
    let output = resolve_text(concat!(
        "class State { static count: i64; static enabled: bool; init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let checked = type_check(&output.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.expect("zero-default statics must lower to HIR");
    let class = hir.class(ClassId::new(0)).unwrap();
    assert_eq!(class.static_fields.len(), 2);
    assert_eq!(class.static_fields[0].id, StaticFieldId::new(class.id, 0));
    assert_eq!(class.static_fields[1].id, StaticFieldId::new(class.id, 1));
}

#[test]
fn static_fields_do_not_satisfy_interface_requirements() {
    let output = resolve_text(concat!(
        "interface Counter { fn count() -> i64; }\n",
        "class State implements Counter { static count: i64; init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let checked = type_check(&output.program);
    assert!(checked.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_INTERFACE_CONFORMANCE
            && diagnostic
                .message
                .contains("does not implement requirement `Counter.count`")
    }));
    assert!(!checked
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code != INVALID_INTERFACE_CONFORMANCE }));
}

#[test]
fn static_fields_do_not_participate_in_string_instance_shape_matching() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "from std::str import Str;\n",
                    "fn main() -> i64 { var value: Str = \"x\"; return 0; }\n",
                ),
            ),
            (
                "std/str.ska",
                concat!(
                    "public class Str {\n",
                    "  private _storage: shared u8[];\n",
                    "  private _start: i64;\n",
                    "  private _length: u64;\n",
                    "  static instances: u64;\n",
                    "  init() {\n",
                    "    self._storage = new u8[]();\n",
                    "    self._start = 0;\n",
                    "    self._length = 0u;\n",
                    "  }\n",
                    "}\n",
                ),
            ),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let item = output
        .program
        .string_language_item
        .as_ref()
        .expect("the three instance fields must retain canonical string matching");
    assert_eq!(item.class, ClassId::new(0));
    assert_eq!(output.program.class(item.class).unwrap().fields.len(), 3);
    assert_eq!(
        output
            .program
            .class(item.class)
            .unwrap()
            .static_fields
            .len(),
        1
    );
}

#[test]
fn resolves_static_declarations_across_a_cyclic_module_graph() {
    let (_workspace, graph) = load_module_sources(
        "first",
        &[
            (
                "first.ska",
                concat!(
                    "import second;\n",
                    "public class First { static peer: second::Second?; init() {} }\n",
                    "fn main() -> i64 { return 0; }\n",
                ),
            ),
            (
                "second.ska",
                concat!(
                    "import first;\n",
                    "public class Second { static peer: first::First?; init() {} }\n",
                ),
            ),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert_eq!(output.program.classes.iter().count(), 2);
    for class in output.program.classes.iter() {
        assert_eq!(class.fields.len(), 0);
        assert_eq!(class.static_fields.len(), 1);
        assert_eq!(class.static_fields[0].id, StaticFieldId::new(class.id, 0));
    }
    let dump = dump_resolved(&output.program);
    assert_eq!(dump.matches("StaticField c").count(), 2);
    assert!(dump.contains("\"peer\""));
}
