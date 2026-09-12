use super::*;
use crate::{
    hir::HirCopyCapability,
    identity::{ClassId, FieldId},
    resolve::ResolvedCopyOperation,
    type_capabilities::{LifecyclePathElement, ResolvedLifecycleCapabilities},
    typeck::{capabilities::CopyCapabilities, type_check, COPY_OPERATION_UNAVAILABLE},
};

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
