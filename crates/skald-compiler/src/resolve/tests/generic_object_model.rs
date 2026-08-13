use super::*;
use crate::resolve::{
    ResolvedExpression, ResolvedInterfaceReceiver, INHERITANCE_CYCLE, INVALID_OVERRIDE,
    PRIVATE_MEMBER_ACCESS, UNSATISFIED_GENERIC_REQUIREMENT, UNSUPPORTED_GENERIC_SYNTAX,
};

fn non_gate_diagnostics(output: &ResolveOutput) -> Vec<&crate::diagnostics::Diagnostic> {
    output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code != UNSUPPORTED_GENERIC_SYNTAX)
        .collect()
}

#[test]
fn inherited_nominal_bound_selects_the_interface_requirement_in_generated_bodies() {
    let output = resolve_text(
        "interface Ranked { fn rank() -> i64; }\n\
         class Base implements Ranked {\n\
           init() {}\n\
           fn rank() -> i64 { return 1; }\n\
         }\n\
         class Derived extends Base { init() { super(); } }\n\
         class Reader<T> where T: Ranked {\n\
           value: T;\n\
           init(ref value: T) { self.value = value; }\n\
           fn read() -> i64 { return self.value.rank(); }\n\
         }\n\
         fn use(ref value: Reader<Derived>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(
        non_gate_diagnostics(&output).is_empty(),
        "{:?}",
        output.diagnostics
    );
    let generated = output
        .program
        .classes
        .iter()
        .find(|class| class.name.starts_with("Reader<"))
        .expect("the bounded application must generate a class");
    let definition = output.program.class_definitions.get(generated.id).unwrap();
    let expression = return_value(definition.methods[0].body.statements.last().unwrap());
    let ResolvedExpression::InterfaceCall(call) = expression else {
        panic!("bound-selected member must remain an interface call: {expression:?}")
    };
    assert!(matches!(
        call.receiver,
        ResolvedInterfaceReceiver::Object(_)
    ));
    assert_eq!(call.interface.index(), 0);
    assert_eq!(call.requirement.index(), 0);
}

#[test]
fn bounds_reject_duck_types_shared_owners_and_bare_interface_views_conjunctively() {
    let output = resolve_text(
        "interface Left { fn left() -> i64; }\n\
         interface Right { fn right() -> i64; }\n\
         class OnlyLeft implements Left {\n\
           init() {}\n\
           fn left() -> i64 { return 1; }\n\
         }\n\
         class Duck {\n\
           init() {}\n\
           fn left() -> i64 { return 1; }\n\
           fn right() -> i64 { return 2; }\n\
         }\n\
         class Both<T> where T: Left, T: Right { init() {} }\n\
         fn missing_right(ref value: Both<OnlyLeft>) -> unit {}\n\
         fn structural(ref value: Both<Duck>) -> unit {}\n\
         fn owner(ref value: Both<shared OnlyLeft>) -> unit {}\n\
         fn view(ref value: Both<Left>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let failures = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == UNSATISFIED_GENERIC_REQUIREMENT)
        .collect::<Vec<_>>();
    assert_eq!(failures.len(), 7, "{:?}", output.diagnostics);
    assert!(failures.iter().all(|diagnostic| {
        diagnostic
            .labels
            .iter()
            .any(|label| label.message == "bound declared here")
            && diagnostic
                .labels
                .iter()
                .any(|label| label.message == "template declared here")
    }));
    assert!(failures
        .iter()
        .any(|diagnostic| diagnostic.labels[0].message.contains("shared-owner")));
    assert!(failures.iter().any(|diagnostic| diagnostic.labels[0]
        .message
        .contains("non-owning interface")));
}

#[test]
fn generated_classes_receive_ordinary_cycle_override_and_privacy_validation() {
    let invalid_override = resolve_text(
        "class Base { init() {} fn value() -> i64 { return 0; } }\n\
         class Invalid<T> extends Base {\n\
           init() { super(); }\n\
           override fn value() -> i64 { return 1; }\n\
         }\n\
         fn use(ref value: Invalid<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(invalid_override
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_OVERRIDE));

    let cycle = resolve_text(
        "class Cycle<T> extends Cycle<T> { init() { super(); } }\n\
         fn use(ref value: Cycle<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(cycle
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INHERITANCE_CYCLE));

    let privacy = resolve_text(
        "class GenericBase<T> {\n\
           private value: i64;\n\
           init() { self.value = 1; }\n\
         }\n\
         class Derived extends GenericBase<i64> {\n\
           init() { super(); }\n\
           fn read() -> i64 { return self.value; }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(
        privacy
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == PRIVATE_MEMBER_ACCESS),
        "{:?}",
        privacy.diagnostics
    );
}
