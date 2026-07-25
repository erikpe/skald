use super::*;
use crate::{
    hir::{
        HirExpressionKind, HirLocalInitializer, HirObjectReturn, HirObjectSource, HirReturnValue,
        HirTypeTestKind, HirViewSource, HirViewTarget,
    },
    identity::{ClassId, FunctionId, InterfaceId},
    mir::{dump_mir, lower_hir, verify_mir},
    resolve::{ResolvedExpression, ResolvedStatement, ResolvedTypeKind},
};

const TYPES: &str = "\
interface Tag { fn mark() -> unit; }\n\
class Base implements Tag {\n\
  init() {}\n\
  fn mark() -> unit {}\n\
}\n\
class Derived extends Base { init() { super(); } }\n\
class Other { init() {} }\n";

#[test]
fn checked_object_casts_support_direct_receivers_alias_arguments_and_fields() {
    let output = check_text(
        "class Base { init() {} virtual fn read() -> i64 { return 1; } }\n\
         class Leaf extends Base {\n\
           value: i64;\n\
           init(value: i64) { super(); self.value = value; }\n\
           override fn read() -> i64 { return self.value; }\n\
         }\n\
         fn take(ref leaf: Leaf) -> i64 { return leaf.value; }\n\
         fn inspect(ref value: Obj) -> i64 {\n\
           var from_method: i64 = ((Leaf) value).read();\n\
           var from_field: i64 = ((Leaf) value).value;\n\
           return take((Leaf) value) + from_method + from_field;\n\
         }\n\
         fn replace(mut ref value: Obj) -> unit {\n\
           ((Leaf) value).value = 9;\n\
         }\n\
         fn produced() -> i64 { return ((Leaf) Leaf(4)).read(); }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let definition = hir.definitions.get(FunctionId::new(1)).unwrap();
    let HirStatement::Local(method) = &definition.body.statements[0] else {
        panic!("expected method-result local");
    };
    let HirLocalInitializer::Value(method) = &method.initializer else {
        panic!("expected scalar initializer");
    };
    let HirExpressionKind::MethodCall { receiver, .. } = &method.kind else {
        panic!("expected method call");
    };
    assert!(receiver.checked_cast.is_some());

    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("lowered checked casts must verify");
    let dump = dump_mir(&mir);
    assert!(dump.contains("checked-cast"));
    assert!(dump.contains("end-checked-view"));
}

#[test]
fn checked_interface_casts_support_requirement_receivers() {
    let output = check_text(&format!(
        "{TYPES}\
         fn inspect(ref value: Obj) -> unit {{ ((Tag) value).mark(); }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let definition = hir.definitions.get(FunctionId::new(0)).unwrap();
    let HirStatement::Call(call) = &definition.body.statements[0] else {
        panic!("expected interface call statement");
    };
    let HirExpressionKind::InterfaceCall { receiver, .. } = &call.call.kind else {
        panic!("expected interface call");
    };
    assert!(matches!(
        receiver,
        crate::hir::HirInterfaceReceiver::Checked(_)
    ));
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("interface cast MIR must verify");
}

#[test]
fn checked_class_casts_feed_every_owning_copy_context() {
    let output = check_text(
        "class Base {\n\
           value: i64;\n\
           init(value: i64) { self.value = value; }\n\
         }\n\
         class Leaf extends Base {\n\
           extra: i64;\n\
           init(value: i64, extra: i64) { super(value); self.extra = extra; }\n\
         }\n\
         class Holder {\n\
           item: Leaf;\n\
           init(ref source: Obj) { self.item = (Leaf) source; }\n\
           mut fn replace(ref source: Obj) -> unit { self.item = (Leaf) source; }\n\
         }\n\
         fn consume(value: Leaf) -> i64 { return value.value + value.extra; }\n\
         fn copied(ref source: Obj) -> Leaf { return (Leaf) source; }\n\
         fn exercise(destination: Leaf, ref source: Obj) -> i64 {\n\
           var local: Leaf = (Leaf) source;\n\
           var sliced: Base = (Leaf) source;\n\
           var produced: Base = (Base) Leaf(3, 4);\n\
           destination = (Leaf) source;\n\
           return consume((Leaf) source) + local.value + sliced.value + produced.value;\n\
         }\n\
         fn field_copied(ref source: Obj) -> Leaf { return ((Holder) source).item; }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let exercise = hir.definitions.get(FunctionId::new(2)).unwrap();
    let HirStatement::Local(local) = &exercise.body.statements[0] else {
        panic!("expected copied local");
    };
    let HirLocalInitializer::Copy(local) = &local.initializer else {
        panic!("cast local must use copy construction");
    };
    assert!(matches!(local.source, HirObjectSource::Checked(_)));

    let HirStatement::Local(sliced) = &exercise.body.statements[1] else {
        panic!("expected sliced local");
    };
    let HirLocalInitializer::Copy(sliced) = &sliced.initializer else {
        panic!("cast slice must use copy construction");
    };
    assert!(matches!(sliced.source, HirObjectSource::Slice(_)));

    let copied = hir.definitions.get(FunctionId::new(1)).unwrap();
    let HirStatement::Return(returned) = &copied.body.statements[0] else {
        panic!("expected object return");
    };
    let Some(HirReturnValue::Object(HirObjectReturn::Copy { source, .. })) = &returned.value else {
        panic!("cast return must use copy construction");
    };
    assert!(matches!(&**source, HirObjectSource::Checked(_)));

    let mir = lower_hir(&hir);
    let dump = dump_mir(&mir);
    if let Err(errors) = verify_mir(&mir) {
        panic!("owning cast-source MIR must verify: {errors}\n{dump}");
    }
    assert!(dump.contains("checked-cast"));
    assert!(dump.contains("copy-construct"));
    assert!(dump.contains("copy-assign"));
    assert!(dump_hir(&hir).contains("CheckedSource runtime-terminate"));
}

#[test]
fn interface_and_obj_casts_cannot_supply_inline_storage() {
    let output = check_text(&format!(
        "{TYPES}\
         fn invalid(ref erased: Obj) -> unit {{\n\
           var from_interface: Base = (Tag) erased;\n\
           var from_obj: Base = (Obj) erased;\n\
         }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    ));

    assert!(output.has_errors());
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == INVALID_OBJECT_CONTEXT)
        .collect();
    assert_eq!(errors.len(), 2, "{:?}", output.diagnostics);
    assert!(errors
        .iter()
        .all(|diagnostic| diagnostic.message.contains("class cast")));
}

#[test]
fn checked_cast_copy_sources_use_ordinary_capability_diagnostics() {
    let mut resolved = resolve_text(
        "class Leaf { init() {} }\n\
         fn copied(ref source: Obj) -> Leaf { return (Leaf) source; }\n\
         fn main() -> i64 { return 0; }\n",
    );
    resolved.classes.entries_mut_for_test()[0].copy_constructor =
        crate::resolve::ResolvedCopyOperation::Unavailable;

    let output = type_check(&resolved);
    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == COPY_OPERATION_UNAVAILABLE
            && diagnostic.message.contains("copy construction")
    }));
}

#[test]
fn rejects_shared_impossible_value_target_and_unconsumed_casts() {
    let output = check_text(&format!(
        "{TYPES}\
         fn invalid(ref erased: Obj, ref other: Other) -> unit {{\n\
           (shared Derived) erased;\n\
           ((Base) other).mark();\n\
           (Derived) erased;\n\
         }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    ));

    assert!(output.has_errors());
    let cast_errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == crate::typeck::program::INVALID_OBJECT_CAST)
        .collect();
    assert_eq!(cast_errors.len(), 3, "{:?}", output.diagnostics);
    assert!(cast_errors
        .iter()
        .any(|diagnostic| diagnostic.message.contains("shared-owner")));
    assert!(cast_errors
        .iter()
        .any(|diagnostic| diagnostic.message.contains("never succeed")));
}

#[test]
fn classifies_type_tests_from_exact_and_forwarded_class_obj_and_interface_views() {
    let output = check_text(&format!(
        "{TYPES}\
         fn classify(ref base: Base, ref erased: Obj, ref tag: Tag) -> bool {{\n\
           var derived: Derived = Derived();\n\
           var other: Other = Other();\n\
           var exact_upcast: bool = derived is Base;\n\
           var exact_failure: bool = other is Base;\n\
           var guaranteed_interface: bool = base is Tag;\n\
           var dynamic_obj: bool = erased is Derived;\n\
           var nested: bool = accept((erased is Derived));\n\
           return tag is Derived;\n\
         }}\n\
         fn accept(value: bool) -> bool {{ return value; }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let definition = hir.definitions.get(FunctionId::new(0)).unwrap();
    let expected = [
        HirTypeTestKind::StaticSuccess,
        HirTypeTestKind::StaticFailure,
        HirTypeTestKind::StaticSuccess,
        HirTypeTestKind::Runtime,
    ];
    for (statement, expected) in definition.body.statements[2..6].iter().zip(expected) {
        let HirStatement::Local(local) = statement else {
            panic!("expected type-test local");
        };
        let HirLocalInitializer::Value(expression) = &local.initializer else {
            panic!("expected scalar initializer");
        };
        let HirExpressionKind::TypeTest(test) = &expression.kind else {
            panic!("expected explicit type-test HIR");
        };
        assert_eq!(test.kind, expected);
        assert_eq!(expression.ty, Type::Bool);
    }
    let HirStatement::Local(exact_failure) = &definition.body.statements[3] else {
        panic!("expected exact-failure local");
    };
    let HirLocalInitializer::Value(exact_failure) = &exact_failure.initializer else {
        panic!("expected scalar initializer");
    };
    let HirExpressionKind::TypeTest(exact_failure) = &exact_failure.kind else {
        panic!("expected type test");
    };
    assert_eq!(
        exact_failure.source.target,
        HirViewTarget::Class(ClassId::new(2))
    );
    assert_eq!(exact_failure.target, HirViewTarget::Class(ClassId::new(0)));

    let returned = returned_expression(definition);
    let HirExpressionKind::TypeTest(test) = &returned.kind else {
        panic!("expected interface type test");
    };
    assert_eq!(test.kind, HirTypeTestKind::Runtime);
    assert_eq!(test.target, HirViewTarget::Class(ClassId::new(1)));
    assert!(matches!(
        test.source.source,
        HirViewSource::Forwarded { .. }
    ));

    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("lowered type operations must verify");
    let dump = dump_mir(&mir);
    assert!(dump.contains("const.bool true"));
    assert!(dump.contains("const.bool false"));
    assert!(dump.contains("type-test"));
}

#[test]
fn rejects_invalid_type_operation_targets_after_resolution() {
    let mut resolved = resolve_text(&format!(
        "{TYPES}\
         fn corrupt(ref erased: Obj) -> bool {{ return erased is Derived; }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    ));
    let definition = resolved
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let ResolvedStatement::Return(returned) = &mut definition.body.statements[0] else {
        panic!("expected return");
    };
    let ResolvedExpression::TypeTest(test) = returned.value.as_mut().unwrap() else {
        panic!("expected type test");
    };
    test.target.kind = ResolvedTypeKind::I64;
    let output = type_check(&resolved);
    assert!(output.hir.is_none());
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        INVALID_TYPE_TEST
    );
}

#[test]
fn supports_deep_base_and_interface_targets_without_losing_static_information() {
    let output = check_text(
        "interface Marker { fn mark() -> unit; }\n\
         class Root implements Marker { init() {} fn mark() -> unit {} }\n\
         class Middle extends Root { init() { super(); } }\n\
         class Leaf extends Middle { init() { super(); } }\n\
         fn inspect(ref leaf: Leaf) -> bool { return leaf is Marker; }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let test = returned_expression(hir.definitions.get(FunctionId::new(0)).unwrap());
    let HirExpressionKind::TypeTest(test) = &test.kind else {
        panic!("expected type test");
    };
    assert_eq!(test.kind, HirTypeTestKind::StaticSuccess);
    assert_eq!(test.target, HirViewTarget::Interface(InterfaceId::new(0)));
}
