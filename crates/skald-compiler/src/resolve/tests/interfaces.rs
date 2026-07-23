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
