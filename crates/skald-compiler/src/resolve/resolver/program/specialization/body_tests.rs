use crate::{
    identity::{ClassId, ClassTemplateId},
    resolve::{dump_resolved, ResolvedExpression, ResolvedStatement, ResolvedTypeKind},
    test_support::resolve_source,
    typeck::type_check,
};

#[test]
fn generated_definitions_close_lifecycle_methods_locals_and_static_members() {
    let output = resolve_source(
        "class Vec<T> {\n\
           static seed: i64 = 1;\n\
           values: T?[];\n\
           init() { self.values = T?[](2u); }\n\
           copy(ref source: Vec<T>) { self.values = source.values; }\n\
           assign(ref source: Vec<T>) { self.values = source.values; }\n\
           destroy {}\n\
           static fn identity(value: T) -> T { return value; }\n\
           mut fn replace(value: T) -> T {\n\
             var previous: T = value;\n\
             self.values[0] = some(value);\n\
             Vec<T>::seed = 2;\n\
             return Vec<T>::identity(previous);\n\
           }\n\
         }\n\
         fn use(ref value: Vec<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert_eq!(
        generic_body_diagnostics(&output),
        0,
        "{:?}",
        output.diagnostics
    );
    let class = output.program.classes.get(ClassId::new(0)).unwrap();
    let definition = output.program.class_definitions.get(class.id).unwrap();
    assert_eq!(definition.initializers.len(), 1);
    assert!(definition.copy_constructor.is_some());
    assert!(definition.copy_assignment.is_some());
    assert!(definition.destructor.is_some());
    assert_eq!(definition.methods.len(), 2);
    assert_eq!(
        definition.methods[1].locals[0].type_syntax.kind,
        ResolvedTypeKind::I64
    );
    assert!(class.static_fields[0].initializer.is_some());

    let dump = dump_resolved(&output.program);
    for fragment in [
        "ClassDefinition c0",
        "ArrayConstruction inline",
        "StaticFieldAssignment c0:static0",
        "StaticCall c0:method0",
        "Type I64",
    ] {
        assert!(dump.contains(fragment), "missing `{fragment}` in:\n{dump}");
    }

    // Specialization publishes complete bodies before the ordinary checker,
    // including generated members not called by the source program.
    let checked = type_check(&output.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
}

#[test]
fn applications_discovered_only_in_bodies_use_the_same_deterministic_worklist() {
    let output = resolve_source(
        "class Vec<T> {\n\
           value: T;\n\
           init(value: T) { self.value = value; }\n\
           init(value: T, marker: i64) { self.value = value; }\n\
           copy(ref source: Vec<T>) { self.value = source.value; }\n\
         }\n\
         class Item { init() {} }\n\
         class Factory<T> {\n\
           init() {}\n\
           static fn borrow(ref value: T) -> unit {}\n\
           fn build(value: T, ref object: Obj) -> bool {\n\
             var inline: Vec<T> = Vec<T>(value);\n\
             var copied: Vec<T> = Vec<T>(copy inline);\n\
             var owner: shared Vec<T> = new Vec<T>(value);\n\
             var copied_owner: shared Vec<T> = new Vec<T>(copy inline);\n\
             var cast: T = (T) object;\n\
             Factory<T>::borrow(cast);\n\
             return object is T;\n\
           }\n\
         }\n\
         fn use(ref factory: Factory<Item>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert_eq!(
        generic_body_diagnostics(&output),
        0,
        "{:?}",
        output.diagnostics
    );
    let entries = output
        .program
        .generic_specializations
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2, "{}", dump_resolved(&output.program));
    assert_eq!(entries[0].key.template, ClassTemplateId::new(1));
    assert_eq!(entries[1].key.template, ClassTemplateId::new(0));

    let factory = output.program.classes.get(ClassId::new(1)).unwrap();
    let body = &output
        .program
        .class_definitions
        .get(factory.id)
        .unwrap()
        .methods[1]
        .body;
    assert_eq!(body.statements.len(), 7);
    assert!(matches!(body.statements[0], ResolvedStatement::Local(_)));
    assert!(matches!(body.statements[1], ResolvedStatement::Local(_)));
    assert!(matches!(body.statements[2], ResolvedStatement::Local(_)));
    assert!(matches!(body.statements[3], ResolvedStatement::Local(_)));
    let ResolvedStatement::Local(cast) = &body.statements[4] else {
        panic!("expected the closed object-cast local")
    };
    assert!(matches!(
        cast.initializer,
        ResolvedExpression::ObjectCast(_)
    ));
    let ResolvedStatement::Return(result) = &body.statements[6] else {
        panic!("expected the closed type test")
    };
    assert!(matches!(
        result.value,
        Some(ResolvedExpression::TypeTest(_))
    ));

    let dump = dump_resolved(&output.program);
    for fragment in [
        "Construct c2",
        "CopyConstruct c2",
        "Allocate c2",
        "CopyAllocate c2",
        "ObjectCast target class c0",
        "TypeTest target class c0",
        "StaticCall c1:method0",
    ] {
        assert!(dump.contains(fragment), "missing `{fragment}` in:\n{dump}");
    }
}

#[test]
fn an_invalid_unused_generated_member_is_reported_at_the_application_and_template() {
    let output = resolve_source(
        "class Empty<T> { init() {} }\n\
         class Broken<T> {\n\
           init() {}\n\
           fn unused() -> i64 { return Empty<T>::missing(); }\n\
         }\n\
         fn use(ref value: Broken<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.labels.iter().any(|label| {
                label.message == "this application uses a generic class with an invalid body"
            })
        })
        .expect("the unused generated member must still be resolved");
    assert_eq!(diagnostic.labels.len(), 3);
    assert_eq!(
        diagnostic.labels[0].style,
        crate::diagnostics::LabelStyle::Primary
    );
    assert_eq!(diagnostic.labels[2].message, "template declared here");
    assert!(output.program.class_definitions.is_empty());
}

#[test]
fn body_operation_requirements_report_application_and_template_origins() {
    let output = resolve_source(
        "class Resource { init() {} copy() {} }\n\
         class Box<T> {\n\
           value: T;\n\
           init(value: T) { self.value = value; }\n\
         }\n\
         fn use(ref value: Box<Resource>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == super::super::super::UNSATISFIED_GENERIC_REQUIREMENT)
        .expect("stored initialization must require a copyable argument");
    assert_eq!(diagnostic.labels.len(), 3);
    assert_eq!(
        diagnostic.labels[0].style,
        crate::diagnostics::LabelStyle::Primary
    );
    assert!(diagnostic.labels[0].message.contains("copy construction"));
    assert!(diagnostic.labels[1]
        .message
        .contains("stored-value initialization"));
    assert_eq!(diagnostic.labels[2].message, "generic class declared here");
    assert_eq!(output.program.classes.len(), 1);
    assert!(output.program.class_definitions.is_empty());
}

fn generic_body_diagnostics(output: &crate::resolve::ResolveOutput) -> usize {
    output.diagnostics.len()
}
