use crate::{
    identity::{ClassId, CopyAssignmentId, DestructorId, InitializerId, MethodId},
    test_support::resolve_source,
};

#[test]
fn resolves_every_accepted_class_body_through_ordered_definition_slots() {
    let output = resolve_source(concat!(
        "class Complete {\n",
        "    value: i64;\n",
        "    init(value: i64) { self.value = value; }\n",
        "    init(ref source: Complete) { self.value = source.value; }\n",
        "    assign(ref source: Complete) { self.value = source.value; }\n",
        "    destroy { var old: i64 = self.value; }\n",
        "    fn first() -> i64 { return self.value; }\n",
        "    fn second() -> i64 { return self.first(); }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let definition = output
        .program
        .class_definitions
        .get(ClassId::new(0))
        .unwrap();
    assert_eq!(
        definition.initializer.as_ref().unwrap().callable,
        InitializerId::new(ClassId::new(0), 0).into()
    );
    assert_eq!(
        definition.copy_constructor.as_ref().unwrap().callable,
        InitializerId::new(ClassId::new(0), 1).into()
    );
    assert_eq!(
        definition.copy_assignment.as_ref().unwrap().callable,
        CopyAssignmentId::new(ClassId::new(0), 0).into()
    );
    assert_eq!(
        definition.destructor.as_ref().unwrap().callable,
        DestructorId::new(ClassId::new(0), 0).into()
    );
    assert_eq!(
        definition.methods[0].callable,
        MethodId::new(ClassId::new(0), 0).into()
    );
    assert_eq!(
        definition.methods[1].callable,
        MethodId::new(ClassId::new(0), 1).into()
    );
    assert_eq!(definition.methods.len(), 2);
    assert_eq!(
        definition
            .initializer
            .as_ref()
            .unwrap()
            .body
            .statements
            .len(),
        1
    );
}
