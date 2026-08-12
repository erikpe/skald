use super::*;

#[test]
fn generic_declarations_are_explicitly_gated_before_template_resolution_exists() {
    let output = resolve_text(
        "class Box<T> where T: Comparable { value: T; }\n\
         class Comparable {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.has_errors());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == UNSUPPORTED_GENERIC_SYNTAX));
    assert_eq!(output.program.classes.len(), 1);
    assert_eq!(
        output.program.classes.get(ClassId::new(0)).unwrap().name,
        "Comparable"
    );
}

#[test]
fn closed_generic_applications_are_not_resolved_as_raw_class_names() {
    let output = resolve_text(
        "class Box { init() {} }\n\
         fn inspect() -> unit { Box<i64>(); new Box<i64>(); return; }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.has_errors());
    assert!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == UNSUPPORTED_GENERIC_SYNTAX)
            .count()
            >= 2,
        "{:?}",
        output.diagnostics
    );
}
