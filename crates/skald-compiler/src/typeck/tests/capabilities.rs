use super::*;
use crate::{
    hir::{
        HirArrayAssignElement, HirArrayCopyElement, HirArrayDefaultElement, HirArrayDestroyElement,
        HirBaseCopy, HirCopyCapability, HirSelectedCopyOperation, HirSynthesizedFieldCopy,
    },
    identity::{ArrayTypeId, ClassId, CopyAssignmentId, CopyConstructorId, FieldId},
    resolve::ResolvedCopyOperation,
    type_capabilities::{
        LifecycleComputationReport, LifecyclePathElement, ResolvedLifecycleCapabilities,
    },
    typeck::{
        capabilities::{CopyCapabilities, CopyCapabilityComputationReport},
        type_check, COPY_OPERATION_UNAVAILABLE,
    },
};

fn capability_baseline_fixture() -> crate::resolve::ResolvedProgram {
    let mut program = resolve_text(concat!(
        "class Empty { init() {} }\n",
        "class User {\n",
        "  value: i64;\n",
        "  init() { self.value = 0; }\n",
        "  copy(ref other: User) { self.value = other.value; }\n",
        "  assign(ref other: User) { self.value = other.value; }\n",
        "}\n",
        "class Base { base_value: i64; init() { self.base_value = 0; } }\n",
        "class Derived extends Base {\n",
        "  first: Empty; second: User; maybe: User?; deep: User??;\n",
        "  values: User?[]; matrix: User[][]; final locked: i64;\n",
        "  init() {\n",
        "    super(); self.first = Empty(); self.second = User();\n",
        "    self.maybe = none; self.deep = none;\n",
        "    self.values = User?[](); self.matrix = User[][](); self.locked = 0;\n",
        "  }\n",
        "}\n",
        "class ConstructorMissing { init() {} }\n",
        "class AssignmentMissing { init() {} }\n",
        "class DirectFailure {\n",
        "  constructor: ConstructorMissing; assignment: AssignmentMissing;\n",
        "  init() { self.constructor = ConstructorMissing(); self.assignment = AssignmentMissing(); }\n",
        "}\n",
        "class Recursive { next: Recursive; init() {} }\n",
        "class ArrayFailure {\n",
        "  constructor_values: ConstructorMissing[];\n",
        "  assignment_values: AssignmentMissing[];\n",
        "  init() {\n",
        "    self.constructor_values = ConstructorMissing[]();\n",
        "    self.assignment_values = AssignmentMissing[]();\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    program.classes.entries_mut_for_test()[4].copy_constructor = ResolvedCopyOperation::Unavailable;
    program.classes.entries_mut_for_test()[5].copy_assignment = ResolvedCopyOperation::Unavailable;
    program
}

fn assert_resolved_lifecycle_matches_hir_plans(program: &crate::resolve::ResolvedProgram) {
    let resolved = ResolvedLifecycleCapabilities::compute(program);
    let hir = CopyCapabilities::compute(program);
    for class in program.classes.iter() {
        assert_eq!(
            resolved.constructor(class.id),
            hir.constructor(class.id).selected().is_some(),
            "copy-constructor capability diverged for {}",
            class.name
        );
        assert_eq!(
            resolved.assignment(class.id),
            hir.assignment(class.id).selected().is_some(),
            "copy-assignment capability diverged for {}",
            class.name
        );
        assert_eq!(
            resolved.constructor_failure(class.id),
            hir.constructor_failure(class.id)
        );
        assert_eq!(
            resolved.assignment_failure(class.id),
            hir.assignment_failure(class.id)
        );
    }
    for array in program.array_types.iter() {
        assert_eq!(
            resolved.array_copy(array.id),
            hir.array(array.id).lifecycle.copy.is_some()
        );
        assert_eq!(
            resolved.array_assignment(array.id),
            hir.array(array.id).lifecycle.assignment.is_some()
        );
    }
}

#[test]
fn capability_baseline_records_reconstruction_without_global_instrumentation() {
    let program = capability_baseline_fixture();
    let (hir, mut hir_report) = CopyCapabilities::compute_with_report(&program);
    let published_arrays = hir.array_types_with_report(&mut hir_report);
    let (_, neutral_report) = ResolvedLifecycleCapabilities::compute_with_report(&program);

    assert_eq!(published_arrays.len(), 5);
    assert_eq!(
        hir_report,
        CopyCapabilityComputationReport {
            neutral_computations: 1,
            neutral_lifecycle: LifecycleComputationReport {
                constructor_rounds: 2,
                assignment_rounds: 2,
                constructor_array_entry_evaluations: 10,
                assignment_array_entry_evaluations: 10,
                final_array_entry_evaluations: 10,
            },
            constructor_rounds: 2,
            assignment_rounds: 2,
            cloned_constructor_records: 36,
            cloned_assignment_records: 36,
            provisional_array_builds: 4,
            provisional_array_entries: 20,
            final_array_builds: 1,
            final_array_entries: 5,
            final_publication_clones: 1,
            final_publication_entries: 5,
            constructor_plan_constructions: 9,
            assignment_plan_constructions: 18,
        }
    );
    assert_eq!(
        neutral_report,
        LifecycleComputationReport {
            constructor_rounds: 2,
            assignment_rounds: 2,
            constructor_array_entry_evaluations: 10,
            assignment_array_entry_evaluations: 10,
            final_array_entry_evaluations: 10,
        }
    );
}

#[test]
fn capability_baseline_freezes_exact_facts_failure_paths_and_hir_plans() {
    let program = capability_baseline_fixture();
    let neutral = ResolvedLifecycleCapabilities::compute(&program);
    let hir = CopyCapabilities::compute(&program);

    for (index, name) in [
        "Empty",
        "User",
        "Base",
        "Derived",
        "ConstructorMissing",
        "AssignmentMissing",
        "DirectFailure",
        "Recursive",
        "ArrayFailure",
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(program.classes.iter().nth(index).unwrap().name, name);
        assert_eq!(
            program.classes.iter().nth(index).unwrap().id,
            ClassId::new(index)
        );
    }

    let expected_availability = [
        (true, true),
        (true, true),
        (true, true),
        (true, true),
        (false, true),
        (true, false),
        (false, false),
        (false, false),
        (false, false),
    ];
    for (index, (constructor, assignment)) in expected_availability.into_iter().enumerate() {
        let class = ClassId::new(index);
        assert_eq!(
            neutral.constructor(class),
            constructor,
            "constructor c{index}"
        );
        assert_eq!(neutral.assignment(class), assignment, "assignment c{index}");
        assert_eq!(hir.constructor(class).selected().is_some(), constructor);
        assert_eq!(hir.assignment(class).selected().is_some(), assignment);
        if constructor {
            assert_eq!(neutral.constructor_failure(class), None);
        }
        if assignment {
            assert_eq!(neutral.assignment_failure(class), None);
        }
    }

    assert_eq!(
        hir.constructor(ClassId::new(0)).selected(),
        Some(HirSelectedCopyOperation::Synthesized(ClassId::new(0)))
    );
    assert_eq!(
        hir.constructor(ClassId::new(1)).selected(),
        Some(HirSelectedCopyOperation::User(CopyConstructorId::new(
            ClassId::new(1),
            0,
        )))
    );
    assert_eq!(
        hir.assignment(ClassId::new(1)).selected(),
        Some(HirSelectedCopyOperation::User(CopyAssignmentId::new(
            ClassId::new(1),
            0,
        )))
    );

    assert_eq!(
        neutral.constructor_failure(ClassId::new(4)),
        Some([].as_slice())
    );
    assert_eq!(
        neutral.assignment_failure(ClassId::new(5)),
        Some([].as_slice())
    );
    assert_eq!(
        neutral.constructor_failure(ClassId::new(6)),
        Some(
            [LifecyclePathElement::Field(FieldId::new(
                ClassId::new(6),
                0
            ))]
            .as_slice()
        )
    );
    assert_eq!(
        neutral.assignment_failure(ClassId::new(6)),
        Some(
            [LifecyclePathElement::Field(FieldId::new(
                ClassId::new(6),
                1
            ))]
            .as_slice()
        )
    );
    assert_eq!(
        neutral.constructor_failure(ClassId::new(7)),
        Some(
            [LifecyclePathElement::Field(FieldId::new(
                ClassId::new(7),
                0
            ))]
            .as_slice()
        )
    );
    assert_eq!(
        neutral.assignment_failure(ClassId::new(7)),
        Some(
            [LifecyclePathElement::Field(FieldId::new(
                ClassId::new(7),
                0
            ))]
            .as_slice()
        )
    );
    assert_eq!(
        neutral.constructor_failure(ClassId::new(8)),
        Some(
            [LifecyclePathElement::Field(FieldId::new(
                ClassId::new(8),
                0
            ))]
            .as_slice()
        )
    );
    assert_eq!(
        neutral.assignment_failure(ClassId::new(8)),
        Some(
            [LifecyclePathElement::Field(FieldId::new(
                ClassId::new(8),
                1
            ))]
            .as_slice()
        )
    );
    assert!(std::ptr::eq(
        hir.constructor_failure(ClassId::new(6)).unwrap(),
        hir.lifecycle_for_test()
            .constructor_failure(ClassId::new(6))
            .unwrap(),
    ));
    assert!(std::ptr::eq(
        hir.assignment_failure(ClassId::new(6)).unwrap(),
        hir.lifecycle_for_test()
            .assignment_failure(ClassId::new(6))
            .unwrap(),
    ));

    let HirCopyCapability::Synthesized(constructor) = hir.constructor(ClassId::new(3)) else {
        panic!("derived constructor should be synthesized");
    };
    assert_eq!(
        constructor.base,
        Some(HirBaseCopy {
            base: ClassId::new(2),
            operation: HirSelectedCopyOperation::Synthesized(ClassId::new(2)),
        })
    );
    assert_eq!(
        constructor.fields,
        [
            HirSynthesizedFieldCopy::Class {
                field: FieldId::new(ClassId::new(3), 0),
                operation: HirSelectedCopyOperation::Synthesized(ClassId::new(0)),
            },
            HirSynthesizedFieldCopy::Class {
                field: FieldId::new(ClassId::new(3), 1),
                operation: HirSelectedCopyOperation::User(CopyConstructorId::new(
                    ClassId::new(1),
                    0,
                )),
            },
            HirSynthesizedFieldCopy::OptionalClass {
                field: FieldId::new(ClassId::new(3), 2),
                class: ClassId::new(1),
                operation: HirSelectedCopyOperation::User(CopyConstructorId::new(
                    ClassId::new(1),
                    0,
                )),
            },
            HirSynthesizedFieldCopy::Optional {
                field: FieldId::new(ClassId::new(3), 3),
                optional: crate::identity::OptionalTypeId::new(1),
            },
            HirSynthesizedFieldCopy::Array {
                field: FieldId::new(ClassId::new(3), 4),
                array: ArrayTypeId::new(0),
            },
            HirSynthesizedFieldCopy::Array {
                field: FieldId::new(ClassId::new(3), 5),
                array: ArrayTypeId::new(2),
            },
            HirSynthesizedFieldCopy::Scalar {
                field: FieldId::new(ClassId::new(3), 6),
            },
        ]
    );

    let HirCopyCapability::Synthesized(assignment) = hir.assignment(ClassId::new(3)) else {
        panic!("derived assignment should be synthesized");
    };
    assert_eq!(
        assignment.base,
        Some(HirBaseCopy {
            base: ClassId::new(2),
            operation: HirSelectedCopyOperation::Synthesized(ClassId::new(2)),
        })
    );
    assert_eq!(
        assignment.fields,
        [
            HirSynthesizedFieldCopy::Class {
                field: FieldId::new(ClassId::new(3), 0),
                operation: HirSelectedCopyOperation::Synthesized(ClassId::new(0)),
            },
            HirSynthesizedFieldCopy::Class {
                field: FieldId::new(ClassId::new(3), 1),
                operation: HirSelectedCopyOperation::User(CopyAssignmentId::new(
                    ClassId::new(1),
                    0,
                )),
            },
            HirSynthesizedFieldCopy::OptionalClass {
                field: FieldId::new(ClassId::new(3), 2),
                class: ClassId::new(1),
                operation: HirSelectedCopyOperation::User(CopyAssignmentId::new(
                    ClassId::new(1),
                    0,
                )),
            },
            HirSynthesizedFieldCopy::Optional {
                field: FieldId::new(ClassId::new(3), 3),
                optional: crate::identity::OptionalTypeId::new(1),
            },
            HirSynthesizedFieldCopy::Array {
                field: FieldId::new(ClassId::new(3), 4),
                array: ArrayTypeId::new(0),
            },
            HirSynthesizedFieldCopy::Array {
                field: FieldId::new(ClassId::new(3), 5),
                array: ArrayTypeId::new(2),
            },
            HirSynthesizedFieldCopy::Scalar {
                field: FieldId::new(ClassId::new(3), 6),
            },
        ]
    );
    assert_eq!(assignment.final_fields, [FieldId::new(ClassId::new(3), 6)]);

    assert_eq!(program.array_types.len(), 5);
    for (index, (copy, assignment)) in [
        (true, true),
        (true, true),
        (true, true),
        (false, true),
        (true, false),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(neutral.array_copy(ArrayTypeId::new(index)), copy);
        assert_eq!(
            neutral.array_assignment(ArrayTypeId::new(index)),
            assignment
        );
    }
    let optional_user = hir.array(ArrayTypeId::new(0));
    assert_eq!(
        optional_user.lifecycle.default,
        Some(HirArrayDefaultElement::OptionalAbsent)
    );
    assert!(
        matches!(optional_user.lifecycle.copy, Some(HirArrayCopyElement::OptionalClass { class, operation: HirSelectedCopyOperation::User(_) }) if class == ClassId::new(1))
    );
    assert!(
        matches!(optional_user.lifecycle.assignment, Some(HirArrayAssignElement::OptionalClass { class, copy_constructor: HirSelectedCopyOperation::User(_), copy_assignment: HirSelectedCopyOperation::User(_) }) if class == ClassId::new(1))
    );
    assert_eq!(
        optional_user.lifecycle.destruction,
        HirArrayDestroyElement::OptionalClass(ClassId::new(1))
    );

    let user = hir.array(ArrayTypeId::new(1));
    assert!(
        matches!(user.lifecycle.default, Some(HirArrayDefaultElement::Class { class, .. }) if class == ClassId::new(1))
    );
    assert!(
        matches!(user.lifecycle.copy, Some(HirArrayCopyElement::Class { class, .. }) if class == ClassId::new(1))
    );
    assert!(
        matches!(user.lifecycle.assignment, Some(HirArrayAssignElement::Class { class, .. }) if class == ClassId::new(1))
    );
    assert_eq!(
        user.lifecycle.destruction,
        HirArrayDestroyElement::Class(ClassId::new(1))
    );

    let nested = hir.array(ArrayTypeId::new(2));
    assert_eq!(
        nested.lifecycle.default,
        Some(HirArrayDefaultElement::ArrayEmpty(ArrayTypeId::new(1)))
    );
    assert_eq!(
        nested.lifecycle.copy,
        Some(HirArrayCopyElement::Array(ArrayTypeId::new(1)))
    );
    assert_eq!(
        nested.lifecycle.assignment,
        Some(HirArrayAssignElement::Array(ArrayTypeId::new(1)))
    );
    assert_eq!(
        nested.lifecycle.destruction,
        HirArrayDestroyElement::Array(ArrayTypeId::new(1))
    );

    assert_eq!(hir.array(ArrayTypeId::new(3)).lifecycle.copy, None);
    assert!(hir
        .array(ArrayTypeId::new(3))
        .lifecycle
        .assignment
        .is_some());
    assert!(hir.array(ArrayTypeId::new(4)).lifecycle.copy.is_some());
    assert_eq!(hir.array(ArrayTypeId::new(4)).lifecycle.assignment, None);
    assert_resolved_lifecycle_matches_hir_plans(&program);
}

#[test]
fn phase_neutral_lifecycle_facts_match_hir_plan_availability() {
    let mut resolved = resolve_text(concat!(
        "class Good { init() {} }\n",
        "class Bad { init() {} }\n",
        "class Aggregate {\n",
        "  direct: Good; optional: Good??; values: Good?[]; bad: Bad?[];\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let bad = resolved
        .classes
        .iter()
        .find(|class| class.name == "Bad")
        .unwrap()
        .id;
    resolved.classes.entries_mut_for_test()[bad.index()].copy_constructor =
        ResolvedCopyOperation::Unavailable;
    resolved.classes.entries_mut_for_test()[bad.index()].copy_assignment =
        ResolvedCopyOperation::Unavailable;

    assert_resolved_lifecycle_matches_hir_plans(&resolved);
}

#[test]
fn propagates_the_first_unavailable_field_path_without_affecting_the_other_operation() {
    let mut resolved = resolve_text(concat!(
        "class Parent { child: Child; init() { self.child = Child(); } }\n",
        "class Child { init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    resolved.classes.entries_mut_for_test()[1].copy_assignment = ResolvedCopyOperation::Unavailable;

    let capabilities = CopyCapabilities::compute(&resolved);
    assert!(matches!(
        capabilities.constructor(ClassId::new(0)),
        HirCopyCapability::Synthesized(_)
    ));
    assert_eq!(
        capabilities.assignment(ClassId::new(0)),
        &HirCopyCapability::Unavailable
    );
    assert_eq!(
        capabilities.assignment_failure(ClassId::new(0)),
        Some(
            [LifecyclePathElement::Field(FieldId::new(
                ClassId::new(0),
                0
            ))]
            .as_slice()
        )
    );
    assert_eq!(
        capabilities.assignment_failure(ClassId::new(1)),
        Some([].as_slice())
    );
}

#[test]
fn shared_edges_do_not_require_the_pointee_copy_capability() {
    let mut resolved = resolve_text(concat!(
        "class Child { init() {} }\n",
        "class Parent {\n",
        "  child: shared Child;\n",
        "  init(child: shared Child) { self.child = child; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    resolved.classes.entries_mut_for_test()[0].copy_constructor =
        ResolvedCopyOperation::Unavailable;
    resolved.classes.entries_mut_for_test()[0].copy_constructor =
        ResolvedCopyOperation::Unavailable;

    let capabilities = CopyCapabilities::compute(&resolved);
    assert!(matches!(
        capabilities.constructor(ClassId::new(1)),
        HirCopyCapability::Synthesized(_)
    ));
    assert!(matches!(
        capabilities.assignment(ClassId::new(1)),
        HirCopyCapability::Synthesized(_)
    ));
    assert_eq!(capabilities.constructor_failure(ClassId::new(1)), None);
    assert_eq!(capabilities.assignment_failure(ClassId::new(1)), None);
}

#[test]
fn optional_inline_edges_require_the_payload_copy_capability() {
    let mut resolved = resolve_text(concat!(
        "class Child { init() {} }\n",
        "class Parent { child: Child?; init() { self.child = none; } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    resolved.classes.entries_mut_for_test()[0].copy_assignment = ResolvedCopyOperation::Unavailable;

    let capabilities = CopyCapabilities::compute(&resolved);
    assert_eq!(
        capabilities.assignment(ClassId::new(1)),
        &HirCopyCapability::Unavailable
    );
    assert_eq!(
        capabilities.assignment_failure(ClassId::new(1)),
        Some(
            [LifecyclePathElement::Field(FieldId::new(
                ClassId::new(1),
                0
            ))]
            .as_slice()
        )
    );
}

#[test]
fn diagnoses_a_required_but_unavailable_nested_copy_operation() {
    let mut resolved = resolve_text(concat!(
        "class Child { init() {} }\n",
        "class Parent {\n",
        "  child: Child;\n",
        "  init() { self.child = Child(); }\n",
        "  assign(ref other: Parent) { self.child = other.child; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    resolved.classes.entries_mut_for_test()[0].copy_assignment = ResolvedCopyOperation::Unavailable;

    let output = type_check(&resolved);
    assert!(output.hir.is_none());
    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == COPY_OPERATION_UNAVAILABLE)
        .expect("nested copy assignment should require the child capability");
    assert!(diagnostic.message.contains("class `Child`"));
    assert!(diagnostic.message.contains("copy assignment"));
}

#[test]
fn recursive_synthesis_terminates_and_marks_the_capability_unavailable() {
    let resolved = resolve_text(concat!(
        "class Node { next: Node; init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_resolved_lifecycle_matches_hir_plans(&resolved);
    let capabilities = CopyCapabilities::compute(&resolved);
    assert_eq!(
        capabilities.constructor(ClassId::new(0)),
        &HirCopyCapability::Unavailable
    );
    assert_eq!(
        capabilities.constructor_failure(ClassId::new(0)),
        Some(
            [LifecyclePathElement::Field(FieldId::new(
                ClassId::new(0),
                0
            ))]
            .as_slice()
        )
    );
    assert_eq!(
        capabilities.assignment(ClassId::new(0)),
        &HirCopyCapability::Unavailable
    );
}
