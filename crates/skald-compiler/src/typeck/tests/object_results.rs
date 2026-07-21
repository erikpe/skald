use super::*;
use crate::hir::{
    HirLocalInitializer, HirObjectCallTarget, HirReturnValue, HirSelectedCopyOperation,
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
    assert_eq!(result.class, ClassId::new(0));
    assert_eq!(
        result.operation,
        HirSelectedCopyOperation::Synthesized(ClassId::new(0))
    );

    let main = hir.definitions.get(hir.entry_function).unwrap();
    let HirStatement::Local(first) = &main.body.statements[1] else {
        panic!("expected first result local");
    };
    let HirLocalInitializer::Call(first) = &first.initializer else {
        panic!("expected destination-oriented call");
    };
    assert!(matches!(
        first.target,
        HirObjectCallTarget::Direct(function) if function == FunctionId::new(0)
    ));

    let HirStatement::Local(second) = &main.body.statements[2] else {
        panic!("expected method result local");
    };
    let HirLocalInitializer::Call(second) = &second.initializer else {
        panic!("expected destination-oriented method call");
    };
    assert!(matches!(second.target, HirObjectCallTarget::Method { .. }));

    let dump = dump_hir(&hir);
    assert!(dump.contains("ObjectResult c0"));
    assert!(dump.contains("ObjectCall function f0 -> c0"));
    assert!(dump.contains("ObjectCall method c0:method0 -> c0"));
    assert_eq!(dump, dump_hir(&hir));
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
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_OBJECT_CONTEXT
            && diagnostic.message.contains("existing object place")
    }));
    assert!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_OBJECT_CONTEXT)
            .count()
            >= 3
    );
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_EXTERNAL_DECLARATION));
}
