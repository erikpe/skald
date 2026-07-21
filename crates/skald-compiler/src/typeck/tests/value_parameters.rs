use super::*;
use crate::hir::{HirCallArgument, HirParameterMode};

#[test]
fn selects_explicit_copy_arguments_and_owning_parameter_places() {
    let checked = check_text(concat!(
        "class Value {\n",
        "  field: i64;\n",
        "  init(field: i64) { self.field = field; }\n",
        "}\n",
        "fn consume(value: Value, ref alias: Value, marker: i64) -> i64 {\n",
        "  value = alias;\n",
        "  inspect(value);\n",
        "  return value.field + marker;\n",
        "}\n",
        "fn inspect(ref value: Value) -> unit {}\n",
        "fn main() -> i64 {\n",
        "  var source: Value = Value(2);\n",
        "  return consume(source, source, 40);\n",
        "}\n",
    ));
    assert!(checked.diagnostics.is_empty());
    let hir = checked.hir.unwrap();
    let consume = hir.declarations.get(FunctionId::new(0)).unwrap();
    assert_eq!(consume.parameters[0].mode, HirParameterMode::Value);
    assert_eq!(
        consume.parameters[0].ty,
        Type::Class(crate::identity::ClassId::new(0))
    );

    let main = hir.definitions.get(hir.entry_function).unwrap();
    let HirExpressionKind::DirectCall { arguments, .. } = &returned_expression(main).kind else {
        panic!("expected direct call return");
    };
    assert!(matches!(arguments[0], HirCallArgument::Copy(_)));
    assert!(matches!(arguments[1], HirCallArgument::Place(_)));
    assert!(matches!(arguments[2], HirCallArgument::Value(_)));

    let dump = dump_hir(&hir);
    assert!(dump.contains("CopyArgument"));
    assert!(dump.contains("Operation Synthesized"));
    assert_eq!(dump, dump_hir(&hir));
}

#[test]
fn rejects_non_place_object_arguments_and_external_object_signatures() {
    let produced = check_text(concat!(
        "class Value { init() {} }\n",
        "fn consume(value: Value) -> unit {}\n",
        "fn main() -> i64 { consume(Value()); return 0; }\n",
    ));
    assert!(produced.hir.is_none());
    assert!(produced
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_OBJECT_CONTEXT));

    let wrong_class = check_text(concat!(
        "class Expected { init() {} }\n",
        "class Actual { init() {} }\n",
        "fn consume(value: Expected) -> unit {}\n",
        "fn main() -> i64 {\n",
        "  var actual: Actual = Actual();\n",
        "  consume(actual);\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(wrong_class.hir.is_none());
    assert!(wrong_class.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_OBJECT_CONTEXT && diagnostic.message.contains("same class")
    }));

    let external = check_text(concat!(
        "class Value { init() {} }\n",
        "extern fn consume(value: Value) -> unit;\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(external.hir.is_none());
    assert!(external
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_EXTERNAL_DECLARATION));
}
