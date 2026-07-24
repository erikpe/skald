use crate::{
    identity::{ClassId, CopyAssignmentId, DestructorId, FieldId, InitializerId, MethodId},
    resolve::{
        ResolvedCopyOperation, ResolvedExpression, ResolvedStatement, DUPLICATE_MEMBER,
        INVALID_BASE_CLASS, INVALID_LIFECYCLE_SIGNATURE,
    },
    test_support::resolve_source,
};

#[test]
fn resolves_forward_direct_bases_to_stable_class_ids() {
    let output = resolve_source(concat!(
        "class Derived extends Base { init() { super(); } }\n",
        "class Base { init() {} }\n",
        "class Leaf extends Derived { init() { super(); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert_eq!(
        output
            .program
            .classes
            .get(ClassId::new(0))
            .unwrap()
            .direct_base
            .map(|base| base.class),
        Some(ClassId::new(1))
    );
    assert_eq!(
        output
            .program
            .classes
            .get(ClassId::new(1))
            .unwrap()
            .direct_base
            .map(|base| base.class),
        None
    );
    assert_eq!(
        output
            .program
            .classes
            .get(ClassId::new(2))
            .unwrap()
            .direct_base
            .map(|base| base.class),
        Some(ClassId::new(0))
    );
}

#[test]
fn rejects_invalid_direct_base_names_in_source_order() {
    let output = resolve_source(concat!(
        "class Unknown extends Missing { init() {} }\n",
        "fn helper() -> i64 { return 0; }\n",
        "class WrongKind extends helper { init() {} }\n",
        "class SelfBase extends SelfBase { init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let diagnostics: Vec<_> = output.diagnostics.iter().collect();
    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == INVALID_BASE_CLASS));
    assert!(diagnostics[0]
        .message
        .contains("unknown base class `Missing`"));
    assert!(diagnostics[1]
        .message
        .contains("does not name a base class"));
    assert!(diagnostics[2].message.contains("cannot extend itself"));
    assert!(output
        .program
        .classes
        .iter()
        .all(|class| class.direct_base.is_none()));
    assert!((0..3).all(|index| {
        output
            .program
            .hierarchy
            .direct_base(ClassId::new(index))
            .is_none()
    }));
}

#[test]
fn inherited_body_uses_select_the_declaring_base_projection() {
    let output = resolve_source(concat!(
        "class Base { value: i64; init() {} }\n",
        "class Derived extends Base {\n",
        "    init() { super(); }\n",
        "    fn read() -> i64 { return self.value; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.diagnostics.is_empty());
    assert_eq!(
        output
            .program
            .classes
            .get(ClassId::new(1))
            .unwrap()
            .direct_base
            .map(|base| base.class),
        Some(ClassId::new(0))
    );
    assert_eq!(
        output
            .program
            .hierarchy
            .inherited_member(ClassId::new(1), "value"),
        Some(crate::resolve::ResolvedClassMember::Field(FieldId::new(
            ClassId::new(0),
            0,
        )))
    );
    let definition = output
        .program
        .class_definitions
        .get(ClassId::new(1))
        .unwrap();
    let ResolvedStatement::Return(result) = &definition.methods[0].body.statements[0] else {
        panic!("derived method must return the inherited field");
    };
    let Some(ResolvedExpression::FieldAccess(access)) = &result.value else {
        panic!("derived method must retain the inherited field selection");
    };
    assert_eq!(access.field, FieldId::new(ClassId::new(0), 0));
    assert_eq!(
        access.receiver.projections(),
        [crate::object_path::ObjectProjection::Base(ClassId::new(0))]
    );
}

#[test]
fn source_order_assigns_dense_ids_and_records_every_accepted_body() {
    let output = resolve_source(concat!(
        "class Sample {\n",
        "    init(ref source: Sample) {}\n",
        "    first: i64;\n",
        "    fn read() -> i64 { return 0; }\n",
        "    init(value: i64) {}\n",
        "    second: u8;\n",
        "    mut fn write() -> unit {}\n",
        "    assign(ref source: Sample) {}\n",
        "    destroy {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors());
    let class = output.program.classes.get(ClassId::new(0)).unwrap();
    assert_eq!(class.fields[0].id, FieldId::new(class.id, 0));
    assert_eq!(class.fields[1].id, FieldId::new(class.id, 1));
    assert_eq!(class.methods[0].id, MethodId::new(class.id, 0));
    assert_eq!(class.methods[1].id, MethodId::new(class.id, 1));
    assert_eq!(
        class.copy_constructor,
        ResolvedCopyOperation::User(InitializerId::new(class.id, 0))
    );
    assert_eq!(
        class.initializer.as_ref().unwrap().id,
        InitializerId::new(class.id, 1)
    );
    assert_eq!(
        class.copy_assignment,
        ResolvedCopyOperation::User(CopyAssignmentId::new(class.id, 0))
    );
    assert_eq!(
        class.destructor.as_ref().unwrap().id,
        DestructorId::new(class.id, 0)
    );

    let definitions = output.program.class_definitions.get(class.id).unwrap();
    assert!(definitions.initializer.is_some());
    assert!(definitions.copy_constructor.is_some());
    assert!(definitions.copy_assignment.is_some());
    assert!(definitions.destructor.is_some());
    assert_eq!(definitions.methods.len(), 2);
}

#[test]
fn lifecycle_duplicates_and_invalid_signatures_recover_in_source_order() {
    let output = resolve_source(concat!(
        "class Other { init() {} }\n",
        "class Duplicate {\n",
        "    init() {}\n",
        "    init(value: i64) {}\n",
        "    init(ref first: Duplicate) {}\n",
        "    init(ref second: Duplicate) {}\n",
        "    assign(ref first: Duplicate) {}\n",
        "    assign(ref second: Duplicate) {}\n",
        "    destroy {}\n",
        "    destroy { return; }\n",
        "}\n",
        "class MissingSource { init() {} assign() {} }\n",
        "class MutableSource { init() {} assign(mut ref other: MutableSource) {} }\n",
        "class WrongSource { init() {} assign(ref other: Other) {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let diagnostics: Vec<_> = output.diagnostics.iter().collect();
    assert_eq!(diagnostics.len(), 7);
    assert_eq!(
        diagnostics[..4]
            .iter()
            .map(|diagnostic| (diagnostic.code, diagnostic.message.as_str()))
            .collect::<Vec<_>>(),
        [
            (
                DUPLICATE_MEMBER,
                "duplicate ordinary initializer in class `Duplicate`"
            ),
            (
                DUPLICATE_MEMBER,
                "duplicate copy constructor in class `Duplicate`"
            ),
            (
                DUPLICATE_MEMBER,
                "duplicate copy assignment in class `Duplicate`"
            ),
            (
                DUPLICATE_MEMBER,
                "duplicate destructor in class `Duplicate`"
            ),
        ]
    );
    assert!(diagnostics[..4].iter().all(|diagnostic| {
        diagnostic.labels[0].message == "redeclared here"
            && diagnostic.labels[1].message == "first declared here"
    }));
    assert!(diagnostics[4..]
        .iter()
        .all(|diagnostic| diagnostic.code == INVALID_LIFECYCLE_SIGNATURE));
    for class_index in 2..=4 {
        assert_eq!(
            output
                .program
                .classes
                .get(ClassId::new(class_index))
                .unwrap()
                .copy_assignment,
            ResolvedCopyOperation::Unavailable
        );
    }

    let duplicate = output.program.classes.get(ClassId::new(1)).unwrap();
    let definitions = output.program.class_definitions.get(duplicate.id).unwrap();
    assert_eq!(
        definitions
            .initializer
            .as_ref()
            .unwrap()
            .body
            .statements
            .len(),
        0
    );
    assert_eq!(
        definitions
            .copy_constructor
            .as_ref()
            .unwrap()
            .body
            .statements
            .len(),
        0
    );
    assert_eq!(
        definitions
            .destructor
            .as_ref()
            .unwrap()
            .body
            .statements
            .len(),
        0
    );
}

#[test]
fn ordinary_member_duplicates_keep_the_first_kind_and_dense_ids() {
    let output = resolve_source(concat!(
        "class Members {\n",
        "    value: i64;\n",
        "    fn value() -> i64 { return 0; }\n",
        "    fn get() -> i64 { return 1; }\n",
        "    get: i64;\n",
        "    remaining: bool;\n",
        "    init() {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let diagnostics: Vec<_> = output.diagnostics.iter().collect();
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.code == DUPLICATE_MEMBER
            && diagnostic.message.starts_with("duplicate class member")
    }));
    let class = output.program.classes.get(ClassId::new(0)).unwrap();
    assert_eq!(
        class
            .fields
            .iter()
            .map(|field| (field.id, field.name.as_str()))
            .collect::<Vec<_>>(),
        [
            (FieldId::new(class.id, 0), "value"),
            (FieldId::new(class.id, 1), "remaining"),
        ]
    );
    assert_eq!(class.methods.len(), 1);
    assert_eq!(class.methods[0].id, MethodId::new(class.id, 0));
    assert_eq!(class.methods[0].name, "get");
    assert_eq!(
        output
            .program
            .class_definitions
            .get(class.id)
            .unwrap()
            .methods
            .len(),
        1
    );
}
