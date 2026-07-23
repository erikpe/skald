use crate::{
    identity::{ClassId, FieldId, MethodId},
    resolve::{ResolvedClassMember, INHERITANCE_CYCLE, INHERITED_MEMBER_COLLISION},
    test_support::resolve_source,
};

#[test]
fn answers_forward_deep_ancestry_and_declaring_owner_queries() {
    let output = resolve_source(concat!(
        "class Leaf extends Middle { leaf: i64; init() {} }\n",
        "class Root { root: i64; init() {} fn read() -> i64 { return self.root; } }\n",
        "class Middle extends Root { middle: i64; init() {} }\n",
        "class Unrelated { init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hierarchy = &output.program.hierarchy;
    assert_eq!(
        hierarchy
            .base_chain(ClassId::new(0))
            .expect("leaf ancestry must be valid")
            .collect::<Vec<_>>(),
        [ClassId::new(2), ClassId::new(1)]
    );
    assert_eq!(
        hierarchy.is_subtype(ClassId::new(0), ClassId::new(1)),
        Some(true)
    );
    assert_eq!(
        hierarchy.is_subtype(ClassId::new(0), ClassId::new(0)),
        Some(true)
    );
    assert_eq!(
        hierarchy.is_subtype(ClassId::new(1), ClassId::new(0)),
        Some(false)
    );
    assert_eq!(
        hierarchy.is_subtype(ClassId::new(0), ClassId::new(3)),
        Some(false)
    );
    assert_eq!(
        hierarchy.member(ClassId::new(0), "leaf"),
        Some(ResolvedClassMember::Field(FieldId::new(ClassId::new(0), 0)))
    );
    assert_eq!(
        hierarchy.inherited_member(ClassId::new(0), "root"),
        Some(ResolvedClassMember::Field(FieldId::new(ClassId::new(1), 0)))
    );
    let inherited_method = hierarchy
        .inherited_member(ClassId::new(0), "read")
        .expect("deep inherited method must be selected");
    assert_eq!(
        inherited_method,
        ResolvedClassMember::Method(MethodId::new(ClassId::new(1), 0))
    );
    assert_eq!(inherited_method.declaring_class(), ClassId::new(1));
}

#[test]
fn reports_one_normalized_cycle_per_component_in_source_order() {
    let output = resolve_source(concat!(
        "class Alpha extends Beta { init() {} }\n",
        "class Beta extends Alpha { init() {} }\n",
        "class Tail extends Alpha { init() {} }\n",
        "class Delta extends Echo { init() {} }\n",
        "class Echo extends Foxtrot { init() {} }\n",
        "class Foxtrot extends Delta { init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let diagnostics: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == INHERITANCE_CYCLE)
        .collect();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics[0].message,
        "inheritance cycle: `Alpha -> Beta -> Alpha`"
    );
    assert_eq!(
        diagnostics[1].message,
        "inheritance cycle: `Delta -> Echo -> Foxtrot -> Delta`"
    );
    assert_eq!(diagnostics[0].labels.len(), 2);
    assert_eq!(diagnostics[1].labels.len(), 3);
    assert!(output
        .program
        .hierarchy
        .base_chain(ClassId::new(2))
        .is_none());
    assert_eq!(
        output
            .program
            .hierarchy
            .is_subtype(ClassId::new(2), ClassId::new(0)),
        None
    );
}

#[test]
fn diagnoses_inherited_collisions_by_class_then_member_source_order() {
    let output = resolve_source(concat!(
        "class Root {\n",
        "    value: i64;\n",
        "    fn read() -> i64 { return self.value; }\n",
        "    init() {}\n",
        "}\n",
        "class Middle extends Root { init() {} }\n",
        "class Leaf extends Middle {\n",
        "    fn value() -> i64 { return 0; }\n",
        "    read: i64;\n",
        "    init() {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let diagnostics: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == INHERITED_MEMBER_COLLISION)
        .collect();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics[0].message,
        "method `value` in class `Leaf` conflicts with inherited field"
    );
    assert_eq!(
        diagnostics[1].message,
        "field `read` in class `Leaf` conflicts with inherited method"
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.labels.len() == 2));

    let hierarchy = &output.program.hierarchy;
    assert_eq!(
        hierarchy.inherited_member(ClassId::new(2), "value"),
        Some(ResolvedClassMember::Field(FieldId::new(ClassId::new(0), 0)))
    );
    assert_eq!(
        hierarchy.inherited_member(ClassId::new(2), "read"),
        Some(ResolvedClassMember::Method(MethodId::new(
            ClassId::new(0),
            0
        )))
    );
}

#[test]
fn direct_duplicate_recovery_does_not_cascade_into_inherited_collisions() {
    let output = resolve_source(concat!(
        "class Base { shared: i64; init() {} }\n",
        "class Derived extends Base {\n",
        "    shared: i64;\n",
        "    shared: i64;\n",
        "    init() {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INHERITED_MEMBER_COLLISION)
            .count(),
        1
    );
}
