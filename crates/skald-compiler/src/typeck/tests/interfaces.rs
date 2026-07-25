use super::*;
use crate::{
    hir::{dump_hir, HirCallArgument, HirExpressionKind, HirViewSource, HirViewTarget},
    identity::{ClassId, InterfaceId, InterfaceRequirementId, MethodId},
};

const VALID_PROGRAM: &str = "\
interface Runner { fn run(value: u64) -> u64; }\n\
class Base implements Runner {\n\
  init() {}\n\
  virtual fn run(value: u64) -> u64 { return value; }\n\
}\n\
class Worker extends Base {\n\
  init() { super(); }\n\
  override fn run(value: u64) -> u64 { return value; }\n\
}\n\
fn invoke(ref runner: Runner, value: u64) -> u64 { return runner.run(value); }\n\
fn forward(ref runner: Runner, value: u64) -> u64 { return invoke(runner, value); }\n\
fn erase(ref runner: Runner) -> unit { any(runner); }\n\
fn any(ref value: Obj) -> unit {}\n\
fn main() -> i64 {\n\
  var worker: Worker = Worker();\n\
  var result: u64 = invoke(worker, 7u);\n\
  return 0;\n\
}\n";

#[test]
fn selects_inherited_conformance_and_interface_calls_by_identity() {
    let output = check_text(VALID_PROGRAM);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let worker = hir.class(ClassId::new(1)).unwrap();
    assert_eq!(worker.conformances.len(), 1);
    assert_eq!(worker.conformances[0].interface, InterfaceId::new(0));
    assert_eq!(
        worker.conformances[0].implementations[0].requirement,
        InterfaceRequirementId::new(InterfaceId::new(0), 0)
    );
    assert_eq!(
        worker.conformances[0].implementations[0].method,
        MethodId::new(ClassId::new(1), 0)
    );

    let invoke = hir
        .definitions
        .get(crate::identity::FunctionId::new(0))
        .unwrap();
    let HirStatement::Return(returned) = &invoke.body.statements[0] else {
        panic!("expected return");
    };
    let crate::hir::HirReturnValue::Scalar(expression) = returned.value.as_ref().unwrap() else {
        panic!("expected scalar interface-call result");
    };
    let HirExpressionKind::InterfaceCall {
        receiver, target, ..
    } = &expression.kind
    else {
        panic!("expected interface call");
    };
    assert_eq!(target.interface, InterfaceId::new(0));
    let crate::hir::HirInterfaceReceiver::View(receiver) = receiver else {
        panic!("ordinary interface parameter must remain an unchecked view");
    };
    assert_eq!(
        receiver.target,
        HirViewTarget::Interface(InterfaceId::new(0))
    );

    let main = hir
        .definitions
        .get(crate::identity::FunctionId::new(4))
        .unwrap();
    let HirStatement::Local(result) = &main.body.statements[1] else {
        panic!("expected result local");
    };
    let crate::hir::HirLocalInitializer::Value(call) = &result.initializer else {
        panic!("expected scalar call");
    };
    let HirExpressionKind::DirectCall { arguments, .. } = &call.kind else {
        panic!("expected direct call");
    };
    assert!(matches!(
        arguments[0],
        HirCallArgument::View(ref view)
            if view.target == HirViewTarget::Interface(InterfaceId::new(0))
    ));

    let forward = hir
        .definitions
        .get(crate::identity::FunctionId::new(1))
        .unwrap();
    let HirStatement::Return(forwarded) = &forward.body.statements[0] else {
        panic!("expected forwarding return");
    };
    let crate::hir::HirReturnValue::Scalar(forwarded) = forwarded.value.as_ref().unwrap() else {
        panic!("expected scalar forwarded call");
    };
    let HirExpressionKind::DirectCall { arguments, .. } = &forwarded.kind else {
        panic!("expected forwarding call");
    };
    assert!(matches!(
        arguments[0],
        HirCallArgument::View(ref view)
            if matches!(
                view.source,
                HirViewSource::Forwarded {
                    target: HirViewTarget::Interface(_),
                    ..
                }
            )
    ));

    let erase = hir
        .definitions
        .get(crate::identity::FunctionId::new(2))
        .unwrap();
    let HirStatement::Call(erase) = &erase.body.statements[0] else {
        panic!("expected erase call");
    };
    let HirExpressionKind::DirectCall { arguments, .. } = &erase.call.kind else {
        panic!("expected direct erase call");
    };
    assert!(matches!(
        arguments[0],
        HirCallArgument::View(ref view) if view.target == HirViewTarget::Obj
    ));

    let dump = dump_hir(&hir);
    assert!(dump.contains("Requirement i0:requirement0 readonly \"run\" -> u64"));
    assert!(dump.contains("i0:requirement0 -> c1:method0"));
    assert!(dump.contains("InterfaceCall i0 i0:requirement0"));
    assert!(dump.contains("ViewArgument -> interface i0 readonly"));
    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).expect("lowered interface MIR must verify");
}

#[test]
fn preserves_interface_and_requirement_source_order_independently_of_methods() {
    let output = check_text(
        "interface First { fn one(ref other: First) -> unit; fn two() -> unit; }\n\
         interface Second { fn three() -> unit; }\n\
         class Both implements Second, First {\n\
           init() {}\n\
           fn two() -> unit {}\n\
           fn three() -> unit {}\n\
           fn one(ref other: First) -> unit {}\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let class = output.hir.unwrap().class(ClassId::new(0)).unwrap().clone();
    assert_eq!(
        class
            .conformances
            .iter()
            .map(|conformance| conformance.interface)
            .collect::<Vec<_>>(),
        [InterfaceId::new(1), InterfaceId::new(0)]
    );
    assert_eq!(
        class.conformances[1]
            .implementations
            .iter()
            .map(|implementation| implementation.method)
            .collect::<Vec<_>>(),
        [
            MethodId::new(ClassId::new(0), 2),
            MethodId::new(ClassId::new(0), 0)
        ]
    );
}

#[test]
fn diagnoses_missing_and_inexact_implementations() {
    let output = check_text(
        "interface Shape { mut fn resize(ref other: Shape) -> bool; fn area() -> u64; }\n\
         class Bad implements Shape {\n\
           init() {}\n\
           fn resize(ref other: Shape) -> bool { return true; }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    let messages = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(messages
        .iter()
        .any(|message| message.contains("does not exactly implement")));
    assert!(messages
        .iter()
        .any(|message| message.contains("does not implement requirement `Shape.area`")));
    assert!(output.hir.is_none());
}

#[test]
fn rejects_duplicate_requirements_redundant_claims_and_interface_storage() {
    let output = check_text(
        "interface Named { fn name() -> u64; fn name() -> u64; }\n\
         class Base implements Named { init() {} fn name() -> u64 { return 1u; } }\n\
         class Derived extends Base implements Named { stored: Named; init() { super(); } }\n\
         fn invalid(value: Named) -> Named { var local: Named = value; return; }\n\
         fn main() -> i64 { return 0; }\n",
    );
    let text = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("duplicate requirement `name`"));
    assert!(text.contains("redundantly implements interface `Named`"));
    assert!(text.contains("field `stored` cannot have type"));
    assert!(text.contains("parameter `value` requires a stored value type"));
    assert!(text.contains("cannot return a non-owning view"));
    assert!(text.contains("local `local` cannot store a non-owning view"));
}

#[test]
fn enforces_mutable_interface_receiver_access() {
    let output = check_text(
        "interface Mutable { mut fn update() -> unit; }\n\
         class Item implements Mutable { init() {} mut fn update() -> unit {} }\n\
         fn bad(ref item: Mutable) -> unit { item.update(); }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(output.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("requires mutable receiver access")));

    let valid = check_text(
        "interface Mutable { mut fn update() -> unit; }\n\
         class Item implements Mutable { init() {} mut fn update() -> unit {} }\n\
         fn good(mut ref item: Mutable) -> unit { item.update(); }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(!valid.has_errors(), "{:?}", valid.diagnostics);
    let mir = crate::mir::lower_hir(valid.hir.as_ref().unwrap());
    crate::mir::verify_mir(&mir).unwrap();
}
