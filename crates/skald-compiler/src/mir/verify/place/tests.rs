use crate::{
    identity::{ArrayTypeId, ClassId, FieldId, FunctionId},
    mir::{verify_mir, MirInstruction, MirPlace, MirPlaceProjection, MirRvalueKind, StorageId},
    test_support::lower_source_to_mir,
};

use super::{is_ancestor, places_overlap, projects_into_array_element_storage};

#[test]
fn ancestry_requires_the_same_base_and_a_projection_prefix() {
    let function = FunctionId::new(0);
    let class = ClassId::new(0);
    let root = MirPlace::base(StorageId::new(function, 0));
    let child = root.clone().project_field(FieldId::new(class, 0));
    let grandchild = child.clone().project_field(FieldId::new(class, 1));
    let sibling = root.clone().project_field(FieldId::new(class, 2));
    let other_root = MirPlace::base(StorageId::new(function, 1));
    let alias_root = MirPlace::alias_parameter(StorageId::new(function, 0));

    assert!(is_ancestor(&root, &root));
    assert!(is_ancestor(&root, &grandchild));
    assert!(is_ancestor(&child, &grandchild));
    assert!(!is_ancestor(&grandchild, &child));
    assert!(!is_ancestor(&sibling, &grandchild));
    assert!(!is_ancestor(&root, &other_root));
    assert!(!is_ancestor(&root, &alias_root));
}

#[test]
fn overlap_is_symmetric_and_limited_to_ancestor_relationships() {
    let function = FunctionId::new(0);
    let class = ClassId::new(0);
    let root = MirPlace::base(StorageId::new(function, 0));
    let child = root.clone().project_field(FieldId::new(class, 0));
    let sibling = root.clone().project_field(FieldId::new(class, 1));

    assert!(places_overlap(&root, &child));
    assert!(places_overlap(&child, &root));
    assert!(!places_overlap(&child, &sibling));
}

#[test]
fn array_element_projection_crosses_into_backing_owned_storage() {
    let function = FunctionId::new(0);
    let class = ClassId::new(0);
    let root = MirPlace::alias_parameter(StorageId::new(function, 0));
    let field = root.clone().project_field(FieldId::new(class, 0));
    let element = field
        .clone()
        .project_array_element(ArrayTypeId::new(0), StorageId::new(function, 1));
    let element_field = element.clone().project_field(FieldId::new(class, 1));

    assert!(!projects_into_array_element_storage(&root));
    assert!(!projects_into_array_element_storage(&field));
    assert!(projects_into_array_element_storage(&element));
    assert!(projects_into_array_element_storage(&element_field));
}

#[test]
fn projection_validation_retains_the_exact_non_class_diagnostic() {
    let mut program = lower_source_to_mir("fn main() -> i64 { var value: i64 = 0; return value; }");
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let store = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Store(store) => Some(store),
            _ => None,
        })
        .unwrap();
    store.destination = store
        .destination
        .clone()
        .project_field(FieldId::new(ClassId::new(0), 0));

    assert!(verify_mir(&program)
        .unwrap_err()
        .iter()
        .any(|error| error.message == "field projection c0:field0 has a non-class base"));
}

#[test]
fn projection_validation_retains_the_exact_wrong_owner_diagnostic() {
    let mut program = lower_source_to_mir(concat!(
        "class Inner { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Outer { inner: Inner; init(value: i64) { self.inner = Inner(value); } }\n",
        "fn main() -> i64 { var outer: Outer = Outer(1); return outer.inner.value; }\n",
    ));
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let place = function.body.blocks[0]
        .instructions
        .iter_mut()
        .rev()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) => match &mut assignment.rvalue.kind {
                MirRvalueKind::Load(place) if place.projections.len() == 2 => Some(place),
                _ => None,
            },
            _ => None,
        })
        .unwrap();
    place.projections[1] = MirPlaceProjection::Field(FieldId::new(ClassId::new(1), 0));

    assert!(verify_mir(&program)
        .unwrap_err()
        .iter()
        .any(|error| error.message == "field projection c1:field0 belongs to the wrong class"));
}
