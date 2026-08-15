use super::*;

const PRODUCED_RECEIVERS: &str = concat!(
    "class Item {\n",
    "  value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  static fn make_static(value: i64) -> Item { return Item(value); }\n",
    "  fn make_instance(value: i64) -> Item { return Item(value); }\n",
    "  fn read(extra: i64) -> i64 { return self.value + extra; }\n",
    "}\n",
    "interface Producer { fn produce(value: i64) -> Item; }\n",
    "fn make_direct(value: i64) -> Item { return Item(value); }\n",
    "fn constructed() -> i64 { return Item(1).read(2); }\n",
    "fn direct() -> i64 { return make_direct(3).read(4); }\n",
    "fn static_result() -> i64 { return Item.make_static(5).read(6); }\n",
    "fn instance_result(ref item: Item) -> i64 { return item.make_instance(7).read(8); }\n",
    "fn interface_result(ref producer: Producer) -> i64 { return producer.produce(9).read(10); }\n",
    "fn grouped() -> i64 { return ((Item(11))).read(12); }\n",
    "fn main() -> i64 { return 0; }\n",
);

#[test]
fn exact_class_producers_resolve_once_as_explicit_receivers() {
    let output = resolve_text(PRODUCED_RECEIVERS);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    for index in 1..=6 {
        let definition = output
            .program
            .definitions
            .get(FunctionId::new(index))
            .unwrap();
        let ResolvedExpression::MethodCall(call) = return_value(&definition.body.statements[0])
        else {
            panic!("expected produced method call in function {index}");
        };
        let ResolvedObjectReceiver::Produced {
            producer,
            exact_class,
            class,
            ..
        } = &call.receiver
        else {
            panic!("expected explicit produced receiver in function {index}");
        };
        assert_eq!(*exact_class, ClassId::new(0));
        assert_eq!(*class, ClassId::new(0));
        assert!(matches!(
            &**producer,
            ResolvedExpression::Construct(_)
                | ResolvedExpression::DirectCall(_)
                | ResolvedExpression::StaticCall(_)
                | ResolvedExpression::MethodCall(_)
                | ResolvedExpression::InterfaceCall(_)
        ));
    }

    let dump = dump_resolved(&output.program);
    assert_eq!(
        dump.matches("ProducedReceiver class c0 complete c0")
            .count(),
        6
    );
    assert_eq!(
        dump,
        dump_resolved(&resolve_text(PRODUCED_RECEIVERS).program)
    );
}

#[test]
fn non_exact_call_results_remain_invalid_receivers() {
    let output = resolve_text(concat!(
        "class Item { value: i64; init() { self.value = 1; } fn read() -> i64 { return 1; } }\n",
        "fn primitive() -> i64 { return 1; }\n",
        "fn nothing() -> unit {}\n",
        "fn optional() -> Item? { return none; }\n",
        "fn array_value() -> Item[] { return Item[](); }\n",
        "fn shared_value() -> shared Item { return new Item(); }\n",
        "fn bad() -> unit {\n",
        "  primitive().read();\n",
        "  nothing().read();\n",
        "  optional().read();\n",
        "  array_value().read();\n",
        "  shared_value().read();\n",
        "  new Item().read();\n",
        "  Item().value;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let diagnostics: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == INVALID_MEMBER_SELECTION)
        .collect();
    assert_eq!(diagnostics.len(), 4, "{:?}", output.diagnostics);
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message.contains("not an exact inline class"))
            .count(),
        4
    );
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == crate::resolve::IMPLICIT_SHARED_DEREFERENCE)
            .count(),
        2
    );
}
