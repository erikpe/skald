use super::*;
use crate::{
    identity::OptionalTypeId,
    resolve::{dump_resolved, UNSATISFIED_GENERIC_REQUIREMENT},
    test_support::{parse_source, resolve_source},
    typeck::{
        type_check, INVALID_ARRAY_ELEMENT, INVALID_OPTIONAL_TYPE, RECURSIVE_INLINE_CONTAINMENT,
    },
};

#[test]
fn vector_storage_substitution_preserves_exact_optional_composition() {
    let output = resolve_source(
        "interface View { fn inspect() -> unit; }\n\
         class Item { init() {} }\n\
         class Vec<T> { storage: T?[]; init() {} }\n\
         fn exact(value: Vec<Item>) -> unit {}\n\
         fn optional(value: Vec<Item?>) -> unit {}\n\
         fn owner(value: Vec<shared Item>) -> unit {}\n\
         fn view(value: Vec<shared View>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert_eq!(output.program.classes.len(), 5);
    let generated = output.program.classes.iter().skip(1).collect::<Vec<_>>();
    assert_eq!(
        generated.iter().map(|class| class.id).collect::<Vec<_>>(),
        [
            ClassId::new(1),
            ClassId::new(2),
            ClassId::new(3),
            ClassId::new(4),
        ]
    );
    for class in &generated {
        assert_eq!(class.fields.len(), 1);
        let ResolvedTypeKind::Array(array) = class.fields[0].type_syntax.kind else {
            panic!("vector storage must specialize to an array")
        };
        let element = output.program.array_types.get(array).unwrap().element.kind;
        assert!(matches!(element, ResolvedTypeKind::Optional(_)));
    }

    let exact_optional = optional_array_payload(&output.program, generated[0]);
    let nested_optional = optional_array_payload(&output.program, generated[1]);
    assert_eq!(
        output
            .program
            .optional_types
            .get(exact_optional)
            .unwrap()
            .payload
            .kind,
        ResolvedTypeKind::Class(ClassId::new(0))
    );
    assert_eq!(
        output
            .program
            .optional_types
            .get(nested_optional)
            .unwrap()
            .payload
            .kind,
        ResolvedTypeKind::Optional(exact_optional)
    );
    assert!(matches!(
        output
            .program
            .optional_types
            .get(optional_array_payload(&output.program, generated[2]))
            .unwrap()
            .payload
            .kind,
        ResolvedTypeKind::Shared(_)
    ));

    let checked = type_check(&output.program);
    assert!(!checked.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic.code,
        INVALID_OPTIONAL_TYPE | INVALID_ARRAY_ELEMENT
    )));
}

#[test]
fn headers_allocate_stable_member_and_callable_identities() {
    let output = resolve_source(
        "interface Mark { fn mark() -> unit; }\n\
         class Base { init() {} }\n\
         class Pair<Left, Right> extends Base implements Mark {\n\
           private left: Left;\n\
           right: Right;\n\
           static cached: Right?;\n\
           init(left: Left, right: Right) {}\n\
           copy(ref source: Pair<Left, Right>) {}\n\
           assign(ref source: Pair<Left, Right>) {}\n\
           destroy {}\n\
           mut fn replace(value: Left) -> Right { return self.right; }\n\
         }\n\
         fn use(value: Pair<i64, bool>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let pair = output.program.classes.get(ClassId::new(1)).unwrap();
    assert_eq!(pair.direct_base.unwrap().class, ClassId::new(0));
    assert_eq!(
        pair.implemented_interfaces[0].interface,
        InterfaceId::new(0)
    );
    assert_eq!(pair.fields[0].id, FieldId::new(pair.id, 0));
    assert!(matches!(
        pair.fields[0].visibility,
        ResolvedMemberVisibility::Private { .. }
    ));
    assert_eq!(pair.fields[1].id, FieldId::new(pair.id, 1));
    assert_eq!(pair.fields[0].type_syntax.kind, ResolvedTypeKind::I64);
    assert_eq!(pair.fields[1].type_syntax.kind, ResolvedTypeKind::Bool);
    assert!(matches!(
        pair.fields[1].visibility,
        ResolvedMemberVisibility::Public
    ));
    assert_eq!(pair.static_fields[0].id, StaticFieldId::new(pair.id, 0));
    assert_eq!(pair.initializers[0].id, InitializerId::new(pair.id, 0));
    assert_eq!(
        pair.copy_constructor_declaration.as_ref().unwrap().id,
        CopyConstructorId::new(pair.id, 0)
    );
    assert_eq!(
        pair.copy_assignment_declaration.as_ref().unwrap().id,
        CopyAssignmentId::new(pair.id, 0)
    );
    assert_eq!(
        pair.destructor.as_ref().unwrap().id,
        DestructorId::new(pair.id, 0)
    );
    assert_eq!(pair.methods[0].id, MethodId::new(pair.id, 0));
    assert!(matches!(
        pair.methods[0].kind,
        ResolvedMethodKind::Instance {
            receiver_access: ResolvedReceiverAccess::Mutable,
            ..
        }
    ));
}

#[test]
fn use_site_legality_depends_on_the_specialized_declaration_shape() {
    let observer = resolve_source(
        "interface View { fn inspect() -> unit; }\n\
         class Observer<T> { init() {} fn see(ref value: T) -> unit {} }\n\
         fn use(value: Observer<View>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert_eq!(requirement_failures(&observer), 0);
    assert_eq!(observer.program.classes.len(), 1);
    assert!(matches!(
        observer
            .program
            .classes
            .get(ClassId::new(0))
            .unwrap()
            .methods[0]
            .parameters[0]
            .type_syntax
            .kind,
        ResolvedTypeKind::Interface(_)
    ));

    let owner = resolve_source(
        "interface View { fn inspect() -> unit; }\n\
         class Owner<T> { value: shared T; init() {} }\n\
         fn use(value: Owner<View>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert_eq!(requirement_failures(&owner), 0);
    assert_eq!(owner.program.classes.len(), 1);

    for argument in ["View", "Obj"] {
        let output = resolve_source(format!(
            "interface View {{ fn inspect() -> unit; }}\n\
             class Vec<T> {{ storage: T?[]; init() {{}} }}\n\
             fn use(value: Vec<{argument}>) -> unit {{}}\n\
             fn main() -> i64 {{ return 0; }}\n"
        ));
        assert_eq!(
            requirement_failures(&output),
            1,
            "{argument}: {:?}",
            output.diagnostics
        );
        let failure = output
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == UNSATISFIED_GENERIC_REQUIREMENT)
            .unwrap();
        assert_eq!(failure.labels.len(), 3);
        assert!(output.program.classes.is_empty());
    }

    // `unit` is not a storage type, so the generic-argument grammar rejects
    // it before a specialization request exists.
    let (_, unit) = parse_source(
        "class Vec<T> { storage: T?[]; init() {} }\n\
         fn use(value: Vec<unit>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(unit.diagnostics.has_errors());
}

#[test]
fn generated_declarations_use_ordinary_containment_validation() {
    let recursive = resolve_source(
        "class Loop<T> { value: Loop<T>; init() {} }\n\
         fn use(value: Loop<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    let checked = type_check(&recursive.program);
    assert!(checked
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == RECURSIVE_INLINE_CONTAINMENT));

    let indirect = resolve_source(
        "class Node<T> { next: shared Node<T>; init() {} }\n\
         fn use(value: Node<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    let checked = type_check(&indirect.program);
    assert!(!checked
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == RECURSIVE_INLINE_CONTAINMENT));
}

#[test]
fn resolved_dump_links_generated_classes_to_parameters_and_origins() {
    let output = resolve_source(
        "class Box<T> { value: T; init() {} }\n\
         fn use(value: Box<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    for fragment in [
        "Class c0 module m0 \"Box<i64>\"",
        "SpecializedFrom template0<i64>",
        "TypeArgument template0:type0 = i64",
        "SpecializationOrigin module m0",
        "Field c0:field0 \"value\"",
    ] {
        assert!(dump.contains(fragment), "missing `{fragment}` in:\n{dump}");
    }
}

fn optional_array_payload(
    program: &ResolvedProgram,
    class: &ResolvedClassDeclaration,
) -> OptionalTypeId {
    let ResolvedTypeKind::Array(array) = class.fields[0].type_syntax.kind else {
        unreachable!()
    };
    let ResolvedTypeKind::Optional(optional) = program.array_types.get(array).unwrap().element.kind
    else {
        unreachable!()
    };
    optional
}

fn requirement_failures(output: &crate::resolve::ResolveOutput) -> usize {
    output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == UNSATISFIED_GENERIC_REQUIREMENT)
        .count()
}
