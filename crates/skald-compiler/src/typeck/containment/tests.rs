use super::*;
use crate::{diagnostics::Diagnostics, test_support::resolve_source};

fn containment_diagnostics(source: &str) -> Diagnostics {
    let resolved = resolve_source(source);
    assert!(
        resolved.diagnostics.is_empty(),
        "containment fixture must resolve cleanly: {:?}",
        resolved.diagnostics
    );
    let mut diagnostics = Diagnostics::new();
    validate_containment(&resolved.program, &mut diagnostics);
    diagnostics
}

#[test]
fn diagnoses_direct_self_containment() {
    let diagnostics = containment_diagnostics(concat!(
        "class Node { next: Node; init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_eq!(diagnostics.len(), 1);
    let diagnostic = diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, RECURSIVE_INLINE_CONTAINMENT);
    assert_eq!(
        diagnostic.message,
        "recursive inline containment: `Node.next -> Node`"
    );
    assert_eq!(diagnostic.labels.len(), 1);
}

#[test]
fn renders_an_indirect_cycle_in_declaration_order() {
    let diagnostics = containment_diagnostics(concat!(
        "class Alpha { beta: Beta; init() {} }\n",
        "class Beta { gamma: Gamma; init() {} }\n",
        "class Gamma { alpha: Alpha; init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_eq!(diagnostics.len(), 1);
    let diagnostic = diagnostics.iter().next().unwrap();
    assert_eq!(
        diagnostic.message,
        "recursive inline containment: `Alpha.beta -> Beta.gamma -> Gamma.alpha -> Alpha`"
    );
    assert_eq!(diagnostic.labels.len(), 3);
}

#[test]
fn emits_one_diagnostic_per_recursive_component_in_source_order() {
    let diagnostics = containment_diagnostics(concat!(
        "class First { second: Second; third: Third; init() {} }\n",
        "class Second { first: First; init() {} }\n",
        "class Third { first: First; init() {} }\n",
        "class Last { last: Last; init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let messages: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert_eq!(
        messages,
        [
            "recursive inline containment: `First.second -> Second.first -> First`",
            "recursive inline containment: `Last.last -> Last`",
        ]
    );
}

#[test]
fn accepts_forward_references_repeated_types_diamonds_and_empty_classes() {
    let diagnostics = containment_diagnostics(concat!(
        "class Root { left: Branch; right: Branch; empty: Empty; init() {} }\n",
        "class Branch { leaf: Leaf; init() {} }\n",
        "class Leaf { init() {} }\n",
        "class Empty { init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(diagnostics.is_empty());
}

#[test]
fn includes_base_subobjects_in_finite_containment_validation() {
    let diagnostics = containment_diagnostics(concat!(
        "class Base { derived: Derived; init() {} }\n",
        "class Derived extends Base { init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_eq!(diagnostics.len(), 1);
    let diagnostic = diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, RECURSIVE_INLINE_CONTAINMENT);
    assert_eq!(
        diagnostic.message,
        "recursive inline containment: `Base.derived -> Derived extends Base -> Base`"
    );
    assert_eq!(diagnostic.labels.len(), 2);
}

#[test]
fn accepts_acyclic_base_and_field_containment_diamonds() {
    let diagnostics = containment_diagnostics(concat!(
        "class Root { init() {} }\n",
        "class Branch extends Root { init() {} }\n",
        "class Holder { root: Root; branch: Branch; init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(diagnostics.is_empty());
}
