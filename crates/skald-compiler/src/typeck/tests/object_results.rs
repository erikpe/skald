use super::*;
use crate::hir::{
    HirLocalInitializer, HirObjectCallTarget, HirObjectProducer, HirObjectReturn, HirObjectSource,
    HirReturnValue, HirSelectedCopyOperation,
};
use crate::identity::ClassId;

#[test]
fn selects_copy_returns_and_destination_oriented_object_calls() {
    let output = check_text(concat!(
        "class Value {\n",
        "  field: i64;\n",
        "  init(field: i64) { self.field = field; }\n",
        "  fn duplicate() -> Value { return self; }\n",
        "}\n",
        "fn copy(ref source: Value) -> Value { return source; }\n",
        "fn main() -> i64 {\n",
        "  var source: Value = Value(7);\n",
        "  var first: Value = copy(source);\n",
        "  var second: Value = first.duplicate();\n",
        "  return second.field;\n",
        "}\n",
    ));
    assert!(output.diagnostics.is_empty());
    let hir = output.hir.unwrap();

    let copy = hir.definitions.get(FunctionId::new(0)).unwrap();
    let HirStatement::Return(result) = &copy.body.statements[0] else {
        panic!("expected object return");
    };
    let Some(HirReturnValue::Object(result)) = &result.value else {
        panic!("expected explicit object return value");
    };
    let HirObjectReturn::Copy {
        operation, class, ..
    } = result
    else {
        panic!("expected copy return");
    };
    assert_eq!(*class, ClassId::new(0));
    assert_eq!(
        *operation,
        HirSelectedCopyOperation::Synthesized(ClassId::new(0))
    );

    let main = hir.definitions.get(hir.entry_function).unwrap();
    let HirStatement::Local(first) = &main.body.statements[1] else {
        panic!("expected first result local");
    };
    let HirLocalInitializer::Object(first) = &first.initializer else {
        panic!("expected destination-oriented call");
    };
    let HirObjectProducer::Call(first) = &first.producer else {
        panic!("expected call producer");
    };
    assert!(matches!(
        first.target,
        HirObjectCallTarget::Direct(function) if function == FunctionId::new(0)
    ));

    let HirStatement::Local(second) = &main.body.statements[2] else {
        panic!("expected method result local");
    };
    let HirLocalInitializer::Object(second) = &second.initializer else {
        panic!("expected destination-oriented method call");
    };
    let HirObjectProducer::Call(second) = &second.producer else {
        panic!("expected method-call producer");
    };
    assert!(matches!(second.target, HirObjectCallTarget::Method { .. }));

    let dump = dump_hir(&hir);
    assert!(dump.contains("ObjectResult c0"));
    assert!(dump.contains("ObjectCall function f0 -> c0"));
    assert!(dump.contains("ObjectCall method Direct c0:method0 -> c0"));
    assert_eq!(dump, dump_hir(&hir));
}

#[test]
fn records_constructor_elision_as_typed_destination_selection() {
    let output = check_text(concat!(
        "class Value { init() {} }\n",
        "fn direct() -> Value { return Value(); }\n",
        "fn grouped() -> Value { return (Value()); }\n",
        "fn main() -> i64 {\n",
        "  var direct: Value = Value();\n",
        "  var grouped: Value = (Value());\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();

    let direct = hir.definitions.get(FunctionId::new(0)).unwrap();
    let HirStatement::Return(result) = &direct.body.statements[0] else {
        panic!("expected direct return");
    };
    assert!(matches!(
        &result.value,
        Some(HirReturnValue::Object(HirObjectReturn::Construct { .. }))
    ));

    let grouped = hir.definitions.get(FunctionId::new(1)).unwrap();
    let HirStatement::Return(result) = &grouped.body.statements[0] else {
        panic!("expected grouped return");
    };
    let Some(HirReturnValue::Object(HirObjectReturn::Copy { source, .. })) = &result.value else {
        panic!("grouped return must copy a produced construction");
    };
    assert!(matches!(
        &**source,
        HirObjectSource::Produced(HirObjectProducer::Construct(_))
    ));

    let main = hir.definitions.get(hir.entry_function).unwrap();
    let HirStatement::Local(direct) = &main.body.statements[0] else {
        panic!("expected direct local");
    };
    assert!(matches!(
        &direct.initializer,
        HirLocalInitializer::Object(_)
    ));
    let HirStatement::Local(grouped) = &main.body.statements[1] else {
        panic!("expected grouped local");
    };
    assert!(matches!(&grouped.initializer, HirLocalInitializer::Copy(_)));
}

#[test]
fn diagnoses_invalid_object_result_sources_and_external_results() {
    let output = check_text(concat!(
        "class Expected { init() {} }\n",
        "class Actual { init() {} }\n",
        "fn wrong(ref actual: Actual) -> Expected { return actual; }\n",
        "fn missing() -> Expected { return; }\n",
        "fn produced(ref value: Expected) -> Expected { return identity(value); }\n",
        "fn identity(ref value: Expected) -> Expected { return value; }\n",
        "fn consume(value: Expected) -> unit {}\n",
        "fn produced_argument(ref value: Expected) -> unit { consume(identity(value)); }\n",
        "extern fn external() -> Expected;\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_OBJECT_CONTEXT && diagnostic.message.contains("same class")
    }));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_RETURN));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_EXTERNAL_DECLARATION));
}
