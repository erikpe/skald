use super::*;
use crate::identity::{InterfaceId, InterfaceRequirementId};

#[test]
fn assigns_stable_interface_and_requirement_identities() {
    let output = resolve_text(
        "interface First { fn run(value: u64) -> unit; }\n\
         interface Second { mut fn stop() -> bool; }\n\
         class Worker implements Second, First {}",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let first = output.program.interface(InterfaceId::new(0)).unwrap();
    assert_eq!(
        first.requirements[0].id,
        InterfaceRequirementId::new(first.id, 0)
    );
    let claims = &output
        .program
        .class(ClassId::new(0))
        .unwrap()
        .implemented_interfaces;
    assert_eq!(
        claims
            .iter()
            .map(|claim| claim.interface)
            .collect::<Vec<_>>(),
        [InterfaceId::new(1), InterfaceId::new(0)]
    );
}

#[test]
fn rejects_duplicate_unknown_and_wrong_kind_claims_in_source_order() {
    let output = resolve_text(
        "interface Known {}\nfn helper() -> unit {}\n\
         class Broken implements Known, Known, Missing, helper {}",
    );
    let messages = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        [
            "duplicate interface `Known`",
            "unknown interface `Missing`",
            "`helper` does not name an interface",
        ]
    );
}

#[test]
fn rejects_interface_construction_without_claiming_interface_calls_are_unavailable() {
    let output = resolve_text(
        "interface Readable { fn read() -> i64; }\n\
         fn main() -> i64 { Readable(); return 0; }\n",
    );
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.message, "interface `Readable` is not callable");
    assert_eq!(
        diagnostic.labels[0].message,
        "interfaces describe non-owning views and cannot be constructed"
    );
}

#[test]
fn generic_interface_declarations_are_gated_before_ordinary_identity_assignment() {
    let output = resolve_text("interface Producer<T> where T: Marker { fn produce() -> T; }");

    let diagnostics = output.diagnostics.iter().collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(diagnostics[0].code, UNSUPPORTED_GENERIC_INTERFACE);
    assert_eq!(
        diagnostics[0].message,
        "generic interface `Producer` is not yet supported"
    );
    assert!(output.program.interface(InterfaceId::new(0)).is_none());
}

#[test]
fn generic_interface_claims_and_bounds_are_gated_without_name_only_resolution() {
    let output = resolve_text(concat!(
        "interface Plain {}\n",
        "class Ordinary implements Plain<i64> {}\n",
        "class Generic<T> implements Plain<T> where T: Plain<T> {}\n",
    ));

    let diagnostics = output.diagnostics.iter().collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 3, "{:?}", output.diagnostics);
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == UNSUPPORTED_GENERIC_INTERFACE));
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.message == "generic interface application `Plain` is not yet supported"
    }));
    assert!(output
        .program
        .class(ClassId::new(0))
        .unwrap()
        .implemented_interfaces
        .is_empty());
}
