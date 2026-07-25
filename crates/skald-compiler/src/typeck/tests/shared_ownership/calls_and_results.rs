use super::*;

#[test]
fn shared_calls_record_copy_and_adopt_at_argument_and_result_boundaries() {
    let output = type_check_source(concat!(
        "class Base { init() {} }\n",
        "class Dog extends Base { init() { super(); } }\n",
        "fn make() -> shared Dog { return new Dog(); }\n",
        "fn consume(value: shared Base) -> i64 { return 0; }\n",
        "fn main() -> i64 {\n",
        "  var dog: shared Dog = make();\n",
        "  var first: i64 = consume(dog);\n",
        "  return consume(new Dog());\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output.hir.unwrap();
    let main = hir.definitions.get(FunctionId::new(2)).unwrap();

    let HirStatement::Local(dog) = &main.body.statements[0] else {
        panic!("expected shared call result local");
    };
    let HirLocalInitializer::Shared(dog) = &dog.initializer else {
        panic!("expected shared local");
    };
    assert_eq!(dog.operation, HirOwnerTransfer::Adopt);
    assert!(matches!(
        dog.source,
        HirSharedSource::Produced(HirSharedProducer::Call(_))
    ));

    let arguments = direct_call_arguments(&main.body.statements[1]);
    let HirCallArgument::Shared(argument) = &arguments[0] else {
        panic!("expected shared argument");
    };
    assert_eq!(argument.operation, HirOwnerTransfer::Copy);

    let HirStatement::Return(result) = &main.body.statements[2] else {
        panic!("expected scalar return");
    };
    let Some(HirReturnValue::Scalar(result)) = &result.value else {
        panic!("expected scalar return value");
    };
    let crate::hir::HirExpressionKind::DirectCall { arguments, .. } = &result.kind else {
        panic!("expected direct call");
    };
    let HirCallArgument::Shared(argument) = &arguments[0] else {
        panic!("expected shared argument");
    };
    assert_eq!(argument.operation, HirOwnerTransfer::Adopt);
}

fn direct_call_arguments(statement: &HirStatement) -> &[HirCallArgument] {
    let HirStatement::Local(local) = statement else {
        panic!("expected scalar local");
    };
    let HirLocalInitializer::Value(value) = &local.initializer else {
        panic!("expected scalar local value");
    };
    let crate::hir::HirExpressionKind::DirectCall { arguments, .. } = &value.kind else {
        panic!("expected direct call");
    };
    arguments
}
