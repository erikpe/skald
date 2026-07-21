use super::*;
use crate::{
    hir::HirCopyCapability,
    identity::{ClassId, FieldId},
    resolve::ResolvedCopyOperation,
    typeck::{capabilities::CopyCapabilities, type_check, COPY_OPERATION_UNAVAILABLE},
};

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
        Some([FieldId::new(ClassId::new(0), 0)].as_slice())
    );
    assert_eq!(
        capabilities.assignment_failure(ClassId::new(1)),
        Some([].as_slice())
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

    let capabilities = CopyCapabilities::compute(&resolved);
    assert_eq!(
        capabilities.constructor(ClassId::new(0)),
        &HirCopyCapability::Unavailable
    );
    assert_eq!(
        capabilities.constructor_failure(ClassId::new(0)),
        Some([FieldId::new(ClassId::new(0), 0)].as_slice())
    );
    assert_eq!(
        capabilities.assignment(ClassId::new(0)),
        &HirCopyCapability::Unavailable
    );
}
