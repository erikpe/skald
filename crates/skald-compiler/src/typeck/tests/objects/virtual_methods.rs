use super::*;
use crate::{
    hir::{
        HirExpression, HirExpressionKind, HirMethodCallTarget, HirMethodDispatch, HirObjectOrigin,
        HirReturnValue, HirStatement, HirViewTarget,
    },
    identity::{BindingId, ClassId, FunctionId, MethodId, VirtualFamilyId, VirtualSlotId},
    mir::HirLoweringError,
    typeck::INVALID_OVERRIDE_SIGNATURE,
};

const VIRTUAL_CALLS: &str = concat!(
    "class Root {\n",
    "  init() {}\n",
    "  virtual fn read() -> i64 { return 1; }\n",
    "  fn relay() -> i64 { return self.read(); }\n",
    "  virtual mut fn update() -> unit {}\n",
    "}\n",
    "class Middle extends Root {\n",
    "  init() { super(); }\n",
    "  override fn read() -> i64 { return 2; }\n",
    "  override mut fn update() -> unit {}\n",
    "}\n",
    "class Leaf extends Middle {\n",
    "  init() { super(); }\n",
    "  override fn read() -> i64 { return self.read(); }\n",
    "  override mut fn update() -> unit {}\n",
    "}\n",
    "fn through_root(ref value: Root) -> i64 { return value.read(); }\n",
    "fn through_middle(ref value: Middle) -> i64 { return value.read(); }\n",
    "fn forward(ref value: Root) -> i64 { return through_root(value); }\n",
    "fn mutate(mut ref value: Root) -> unit { value.update(); }\n",
    "fn exact(value: Leaf) -> i64 { return value.read(); }\n",
    "fn sliced(value: Leaf) -> i64 { var root: Root = value; return root.read(); }\n",
    "fn main() -> i64 { return 0; }\n",
);

fn returned_expression_for(hir: &crate::hir::HirProgram, function: usize) -> &HirExpression {
    let definition = hir.definitions.get(FunctionId::new(function)).unwrap();
    let HirStatement::Return(return_) = definition.body.statements.last().unwrap() else {
        panic!("expected a return statement");
    };
    let Some(HirReturnValue::Scalar(expression)) = &return_.value else {
        panic!("expected a scalar return value");
    };
    expression
}

fn method_call(
    expression: &HirExpression,
) -> (&crate::hir::HirMethodReceiver, HirMethodCallTarget) {
    let HirExpressionKind::MethodCall {
        receiver, target, ..
    } = &expression.kind
    else {
        panic!("expected a method call");
    };
    (receiver, *target)
}

#[test]
fn exact_override_signatures_allow_different_parameter_names() {
    let output = check_text(concat!(
        "class Value { init() {} }\n",
        "class Base {\n",
        "  init() {}\n",
        "  virtual fn inspect(ref original: Value, count: i64) -> bool { return true; }\n",
        "}\n",
        "class Derived extends Base {\n",
        "  init() { super(); }\n",
        "  override fn inspect(ref renamed: Value, amount: i64) -> bool { return false; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let hir = output.hir.expect("valid overrides must produce HIR");
    let root = MethodId::new(ClassId::new(1), 0);
    assert_eq!(
        hir.classes.get(ClassId::new(1)).unwrap().methods[0].dispatch,
        HirMethodDispatch::VirtualRoot {
            family: VirtualFamilyId::new(0),
            slot: VirtualSlotId::new(0),
        }
    );
    assert_eq!(
        hir.classes.get(ClassId::new(2)).unwrap().methods[0].dispatch,
        HirMethodDispatch::Override {
            family: VirtualFamilyId::new(0),
            slot: VirtualSlotId::new(0),
            root,
            overridden: root,
        }
    );
}

#[test]
fn override_signature_diagnostics_follow_declaration_order_and_one_rule_per_method() {
    let output = check_text(concat!(
        "class Value { init() {} }\n",
        "class Base {\n",
        "  init() {}\n",
        "  virtual mut fn receiver() -> unit {}\n",
        "  virtual fn count(value: i64) -> unit {}\n",
        "  virtual fn mode(ref value: Value) -> unit {}\n",
        "  virtual fn parameter(value: i64) -> unit {}\n",
        "  virtual fn result() -> i64 { return 0; }\n",
        "}\n",
        "class Derived extends Base {\n",
        "  init() { super(); }\n",
        "  override fn receiver() -> unit {}\n",
        "  override fn count() -> unit {}\n",
        "  override fn mode(value: Value) -> unit {}\n",
        "  override fn parameter(value: bool) -> unit {}\n",
        "  override fn result() -> bool { return false; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert_eq!(
        output
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        [INVALID_OVERRIDE_SIGNATURE; 5]
    );
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.notes.len() == 1));
}

#[test]
fn hir_virtual_declaration_dump_is_exact_and_identity_based() {
    let output = check_text(concat!(
        "class Base { init() {} virtual fn read() -> i64 { return 1; } }\n",
        "class Derived extends Base { init() { super(); } override fn read() -> i64 { return 2; } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let dump = dump_hir(output.hir.as_ref().unwrap());
    let relevant_lines = dump
        .lines()
        .filter(|line| {
            line.contains("Method ") || line.contains("Dispatch ") || line.contains("Family ")
        })
        .map(|line| line.split(" @").next().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        relevant_lines,
        [
            "        Method c0:method0 \"read\" readonly -> i64",
            "          Dispatch VirtualRoot vf0 slot vs0",
            "        Method c1:method0 \"read\" readonly -> i64",
            "          Dispatch Override vf0 slot vs0 root c0:method0 overridden c0:method0",
            "    Family vf0 slot vs0 root c0:method0",
        ]
    );
}

#[test]
fn virtual_calls_retain_family_selection_and_forwarded_dynamic_origin() {
    let output = check_text(VIRTUAL_CALLS);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();

    let (root_receiver, root_target) = method_call(returned_expression_for(&hir, 0));
    assert_eq!(
        root_target,
        HirMethodCallTarget::Virtual {
            family: VirtualFamilyId::new(0),
            slot: VirtualSlotId::new(0),
            selected: MethodId::new(ClassId::new(0), 0),
        }
    );
    assert!(matches!(
        root_receiver.origin.as_ref(),
        HirObjectOrigin::Forwarded {
            binding: BindingId::Parameter(_),
            static_target: HirViewTarget::Class(class),
            dispatch_limit: None,
            ..
        } if *class == ClassId::new(0)
    ));

    let (middle_receiver, middle_target) = method_call(returned_expression_for(&hir, 1));
    assert_eq!(
        middle_target,
        HirMethodCallTarget::Virtual {
            family: VirtualFamilyId::new(0),
            slot: VirtualSlotId::new(0),
            selected: MethodId::new(ClassId::new(1), 0),
        }
    );
    assert_eq!(middle_receiver.place.class(), ClassId::new(1));

    let mutate = hir.definitions.get(FunctionId::new(3)).unwrap();
    let HirStatement::Call(call) = &mutate.body.statements[0] else {
        panic!("mutate must retain its call statement");
    };
    let (receiver, target) = method_call(&call.call);
    assert_eq!(
        target,
        HirMethodCallTarget::Virtual {
            family: VirtualFamilyId::new(1),
            slot: VirtualSlotId::new(1),
            selected: MethodId::new(ClassId::new(0), 2),
        }
    );
    assert_eq!(receiver.place.access, crate::hir::HirAccess::Mutable);
}

#[test]
fn self_redispatch_and_alias_forwarding_preserve_dynamic_metadata() {
    let output = check_text(VIRTUAL_CALLS);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();

    let relay = hir
        .member_definition(MethodId::new(ClassId::new(0), 1).into())
        .unwrap();
    let HirStatement::Return(return_) = &relay.body.statements[0] else {
        panic!("relay must return its virtual self call");
    };
    let Some(HirReturnValue::Scalar(expression)) = &return_.value else {
        panic!("relay must return a scalar");
    };
    let (receiver, target) = method_call(expression);
    assert!(matches!(target, HirMethodCallTarget::Virtual { .. }));
    assert!(matches!(
        receiver.origin.as_ref(),
        HirObjectOrigin::Forwarded {
            binding: BindingId::Receiver(_),
            dispatch_limit: None,
            ..
        }
    ));

    let recursive = hir
        .member_definition(MethodId::new(ClassId::new(2), 0).into())
        .unwrap();
    let HirStatement::Return(return_) = &recursive.body.statements[0] else {
        panic!("override must retain its recursive return");
    };
    let Some(HirReturnValue::Scalar(expression)) = &return_.value else {
        panic!("recursive override must return a scalar");
    };
    let (receiver, target) = method_call(expression);
    assert!(matches!(
        target,
        HirMethodCallTarget::Virtual {
            selected,
            ..
        } if selected == MethodId::new(ClassId::new(2), 0)
    ));
    assert!(matches!(
        receiver.origin.as_ref(),
        HirObjectOrigin::Forwarded {
            binding: BindingId::Receiver(_),
            ..
        }
    ));

    let forward = hir.definitions.get(FunctionId::new(2)).unwrap();
    let HirStatement::Return(return_) = &forward.body.statements[0] else {
        panic!("forward must return its nested call");
    };
    let Some(HirReturnValue::Scalar(expression)) = &return_.value else {
        panic!("forward must return a scalar");
    };
    let HirExpressionKind::DirectCall { arguments, .. } = &expression.kind else {
        panic!("forward must retain the direct outer call");
    };
    let (view, _) = class_alias_view(&arguments[0]);
    assert!(matches!(
        view.origin.as_ref(),
        HirObjectOrigin::Forwarded {
            binding: BindingId::Parameter(_),
            ..
        }
    ));
}

#[test]
fn exact_and_sliced_owning_receivers_select_static_calls() {
    let output = check_text(VIRTUAL_CALLS);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();

    let (exact_receiver, exact_target) = method_call(returned_expression_for(&hir, 4));
    assert_eq!(
        exact_target,
        HirMethodCallTarget::Direct(MethodId::new(ClassId::new(2), 0))
    );
    assert!(matches!(
        exact_receiver.origin.as_ref(),
        HirObjectOrigin::Exact {
            dynamic_class,
            ..
        } if *dynamic_class == ClassId::new(2)
    ));

    let (slice_receiver, slice_target) = method_call(returned_expression_for(&hir, 5));
    assert_eq!(
        slice_target,
        HirMethodCallTarget::Direct(MethodId::new(ClassId::new(0), 0))
    );
    assert!(matches!(
        slice_receiver.origin.as_ref(),
        HirObjectOrigin::Exact {
            dynamic_class,
            ..
        } if *dynamic_class == ClassId::new(0)
    ));
}

#[test]
fn mutable_virtual_calls_use_existing_receiver_access_diagnostics() {
    let output = check_text(concat!(
        "class Root { init() {} virtual mut fn update() -> unit {} }\n",
        "fn invalid(ref value: Root) -> unit { value.update(); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.hir.is_none());
    assert_eq!(
        output
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        [crate::typeck::READ_ONLY_RECEIVER]
    );
}

#[test]
fn destructor_self_calls_use_the_frozen_dispatch_limit() {
    let output = check_text(concat!(
        "class Root {\n",
        "  init() {}\n",
        "  virtual fn read() -> i64 { return 1; }\n",
        "  destroy { var observed: i64 = self.read(); }\n",
        "}\n",
        "class Leaf extends Root {\n",
        "  init() { super(); }\n",
        "  override fn read() -> i64 { return 2; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let destructor = hir
        .member_definition(crate::identity::DestructorId::new(ClassId::new(0), 0).into())
        .unwrap();
    let HirStatement::Local(local) = &destructor.body.statements[0] else {
        panic!("destructor must retain its local");
    };
    let crate::hir::HirLocalInitializer::Value(expression) = &local.initializer else {
        panic!("destructor local must have a scalar value");
    };
    let (receiver, target) = method_call(expression);
    assert_eq!(
        target,
        HirMethodCallTarget::Direct(MethodId::new(ClassId::new(0), 0))
    );
    assert!(matches!(
        receiver.origin.as_ref(),
        HirObjectOrigin::Forwarded {
            binding: BindingId::Receiver(_),
            dispatch_limit: Some(class),
            ..
        } if *class == ClassId::new(0)
    ));
}

#[test]
fn object_result_virtual_calls_use_the_same_explicit_target() {
    let output = check_text(concat!(
        "class Value { init() {} }\n",
        "class Root {\n",
        "  init() {}\n",
        "  virtual fn make() -> Value { return Value(); }\n",
        "}\n",
        "fn through(ref value: Root) -> Value { return value.make(); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let through = hir.definitions.get(FunctionId::new(0)).unwrap();
    let HirStatement::Return(return_) = &through.body.statements[0] else {
        panic!("through must return the virtual object call");
    };
    let Some(HirReturnValue::Object(crate::hir::HirObjectReturn::Copy {
        source: crate::hir::HirObjectSource::Produced(producer),
        ..
    })) = &return_.value
    else {
        panic!("virtual object result must remain an explicit producer");
    };
    let crate::hir::HirObjectProducer::Call(call) = producer else {
        panic!("object producer must be a method call");
    };
    assert!(matches!(
        call.target,
        crate::hir::HirObjectCallTarget::Method {
            target: HirMethodCallTarget::Virtual {
                family,
                selected,
                ..
            },
            ..
        } if family == VirtualFamilyId::new(0)
            && selected == MethodId::new(ClassId::new(1), 0)
    ));
    assert!(matches!(
        lower_hir(&hir),
        Err(HirLoweringError::VirtualDispatchNotRepresented { .. })
    ));
}

#[test]
fn virtual_call_dump_is_exact_and_mir_rejects_the_pending_boundary_cleanly() {
    let output = check_text(VIRTUAL_CALLS);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let dump = dump_hir(&hir);
    let lines = dump
        .lines()
        .filter(|line| {
            (line.starts_with("          MethodCall Virtual")
                && !line.starts_with("           MethodCall Virtual")
                && line.contains("selected c0:method0"))
                || line.contains("Origin Forwarded f0:p0")
                || line.contains("ObjectPlace f0:p0")
        })
        .map(|line| line.split(" @").next().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        lines,
        [
            "          MethodCall Virtual vf0 slot vs0 selected c0:method0 : i64",
            "              ObjectPlace f0:p0 : class c0 readonly",
            "              Origin Forwarded f0:p0 : class c0 readonly",
        ]
    );
    assert!(matches!(
        lower_hir(&hir),
        Err(HirLoweringError::VirtualDispatchNotRepresented { .. })
    ));
}
