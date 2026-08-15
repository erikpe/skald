use super::*;
use crate::hir::{HirCallArgument, HirObjectProducer};

fn indirect_call(expression: &HirExpression) -> &crate::hir::HirIndirectCall {
    let HirExpressionKind::IndirectCall(call) = &expression.kind else {
        panic!("expected an indirect call, got {expression:?}");
    };
    call
}

#[test]
fn callee_forms_are_explicit_receiverless_and_declaration_calls_remain_direct() {
    let output = check_text(concat!(
        "fn increment(value: i64) -> i64 { return value + 1; }\n",
        "class Holder {\n",
        "  callback: fn(i64) -> i64;\n",
        "  static fallback: fn(i64) -> i64 = increment;\n",
        "  init(callback: fn(i64) -> i64) { self.callback = callback; }\n",
        "  fn invoke(value: i64) -> i64 { return self.callback(value); }\n",
        "}\n",
        "fn choose() -> fn(i64) -> i64 { return increment; }\n",
        "fn make() -> Holder { return Holder(increment); }\n",
        "fn local(callback: fn(i64) -> i64) -> i64 { return callback(1); }\n",
        "fn returned() -> i64 { return choose()(2); }\n",
        "fn produced() -> i64 { return make().callback(3); }\n",
        "fn main() -> i64 { return Holder.fallback(4) + produced(); }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let dump = dump_hir(&hir);

    assert_eq!(dump.matches("IndirectCall type").count(), 5, "{dump}");
    assert!(dump.contains("DirectCall f1"), "{dump}");
    assert!(dump.contains("ObjectCall function f2"), "{dump}");
    assert!(dump.contains("StaticRead c0:static0"), "{dump}");
    assert!(dump.contains("ProducedView"), "{dump}");
    assert_eq!(dump, dump_hir(&hir));
}

#[test]
fn indirect_calls_reuse_every_ordinary_argument_plan() {
    let output = check_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn identity(value: i64) -> i64 { return value; }\n",
        "fn target(\n",
        "  value: i64, ref readonly: Item, mut ref writable: Item, copied: Item,\n",
        "  values: i64[], maybe: i64?, maybe_item: Item?, nested: i64??,\n",
        "  maybe_values: (i64[])?, maybe_owner: shared? Item, owner: shared Item,\n",
        "  callback: fn(i64) -> i64\n",
        ") -> i64 { return callback(value); }\n",
        "fn invoke(\n",
        "  callback: fn(i64, ref Item, mut ref Item, Item, i64[], i64?, Item?, i64??, (i64[])?, shared? Item, shared Item, fn(i64) -> i64) -> i64,\n",
        "  mut ref item: Item, values: i64[], maybe: i64?, maybe_item: Item?,\n",
        "  nested: i64??, maybe_values: (i64[])?, maybe_owner: shared? Item, owner: shared Item\n",
        ") -> i64 {\n",
        "  return callback(1, item, item, item, values, maybe, maybe_item, nested, maybe_values, maybe_owner, owner, identity);\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var item: Item = Item(1);\n",
        "  var values: i64[] = i64[]{1};\n",
        "  var maybe: i64? = 1;\n",
        "  var owner: shared Item = new Item(2);\n",
        "  var maybe_item: Item? = item;\n",
        "  var nested: i64?? = maybe;\n",
        "  var maybe_values: (i64[])? = values;\n",
        "  var maybe_owner: shared? Item = owner;\n",
        "  return invoke(target, item, values, maybe, maybe_item, nested, maybe_values, maybe_owner, owner);\n",
        "}\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let invoke = hir.definitions.get(FunctionId::new(2)).unwrap();
    let call = indirect_call(returned_expression(invoke));

    assert_eq!(call.arguments.len(), 12);
    assert!(matches!(call.arguments[0], HirCallArgument::Value(_)));
    assert!(matches!(
        call.arguments[1],
        HirCallArgument::Place(_) | HirCallArgument::View(_) | HirCallArgument::CheckedView(_)
    ));
    assert!(matches!(
        call.arguments[2],
        HirCallArgument::Place(_) | HirCallArgument::View(_) | HirCallArgument::CheckedView(_)
    ));
    assert!(matches!(call.arguments[3], HirCallArgument::Copy(_)));
    assert!(matches!(call.arguments[4], HirCallArgument::Array(_)));
    assert!(matches!(
        call.arguments[5],
        HirCallArgument::Optional { .. }
    ));
    assert!(matches!(
        call.arguments[6],
        HirCallArgument::ClassOptional(_)
    ));
    assert!(matches!(
        call.arguments[7],
        HirCallArgument::AggregateOptional(_)
    ));
    assert!(matches!(
        call.arguments[8],
        HirCallArgument::AggregateOptional(_)
    ));
    assert!(matches!(
        call.arguments[9],
        HirCallArgument::OptionalShared(_)
    ));
    assert!(matches!(call.arguments[10], HirCallArgument::Shared(_)));
    assert!(matches!(call.arguments[11], HirCallArgument::Value(_)));
}

#[test]
fn indirect_results_reuse_scalar_aggregate_optional_shared_and_function_plans() {
    let output = check_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn identity(value: i64) -> i64 { return value; }\n",
        "fn choose() -> fn(i64) -> i64 { return identity; }\n",
        "fn make_item() -> Item { return Item(1); }\n",
        "fn make_values() -> i64[] { return i64[]{1}; }\n",
        "fn make_maybe() -> i64? { return 1; }\n",
        "fn make_owner() -> shared Item { return new Item(2); }\n",
        "fn make_item_optional() -> Item? { return Item(3); }\n",
        "fn make_nested() -> i64?? { var value: i64? = 1; return value; }\n",
        "fn make_values_optional() -> (i64[])? { return i64[]{2}; }\n",
        "fn make_owner_optional() -> shared? Item { return new Item(4); }\n",
        "fn scalar(callback: fn(i64) -> i64) -> i64 { return callback(3); }\n",
        "fn function_result(callback: fn() -> fn(i64) -> i64) -> i64 { return callback()(4); }\n",
        "fn object_result(callback: fn() -> Item) -> Item { return callback(); }\n",
        "fn array_result(callback: fn() -> i64[]) -> i64[] { return callback(); }\n",
        "fn optional_result(callback: fn() -> i64?) -> i64? { return callback(); }\n",
        "fn class_optional_result(callback: fn() -> Item?) -> Item? { return callback(); }\n",
        "fn nested_result(callback: fn() -> i64??) -> i64?? { return callback(); }\n",
        "fn optional_array_result(callback: fn() -> (i64[])?) -> (i64[])? { return callback(); }\n",
        "fn optional_shared_result(callback: fn() -> shared? Item) -> shared? Item { return callback(); }\n",
        "fn shared_result(callback: fn() -> shared Item) -> shared Item { return callback(); }\n",
        "fn noop() -> unit {}\n",
        "fn unit_result(callback: fn() -> unit) -> unit { callback(); }\n",
        "fn main() -> i64 { return scalar(identity); }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let dump = dump_hir(&hir);

    assert!(dump.matches("IndirectCall type").count() >= 11, "{dump}");
    assert!(dump.contains("IndirectObjectCall type"), "{dump}");
    assert!(dump.contains("ArrayInitialization adopt"), "{dump}");
    assert!(dump.contains("Optional"), "{dump}");
    assert!(dump.contains("SharedTransfer Adopt"), "{dump}");

    let object = hir.definitions.get(FunctionId::new(12)).unwrap();
    let HirStatement::Return(returned) = &object.body.statements[0] else {
        panic!("expected object return");
    };
    let Some(crate::hir::HirReturnValue::Object(crate::hir::HirObjectReturn::Copy {
        source, ..
    })) = &returned.value
    else {
        panic!("expected caller-owned object result");
    };
    assert!(matches!(
        source.as_ref(),
        crate::hir::HirObjectSource::Produced(HirObjectProducer::IndirectCall(_))
    ));
}

#[test]
fn indirect_call_failures_report_arity_type_and_alias_mode_without_target_inference() {
    let output = check_text(concat!(
        "class Item { init() {} }\n",
        "fn invalid(callback: fn(i64, mut ref Item) -> i64, ref item: Item) -> i64 {\n",
        "  var wrong_arity: i64 = callback(1);\n",
        "  var wrong_type: i64 = callback(true, item);\n",
        "  return callback(1, item);\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == super::super::program::WRONG_ARGUMENT_COUNT));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == super::super::program::TYPE_MISMATCH));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == super::super::program::INSUFFICIENT_ALIAS_ACCESS));
}

#[test]
fn closed_generic_static_method_values_call_through_their_specialized_signatures() {
    let hir = check_generic_source(concat!(
        "class Identity<T> { init() {} static fn apply(value: T) -> T { return value; } }\n",
        "fn main() -> i64 {\n",
        "  var integer: fn(i64) -> i64 = Identity<i64>::apply;\n",
        "  var boolean: fn(bool) -> bool = Identity<bool>::apply;\n",
        "  var value: i64 = integer(7);\n",
        "  if (boolean(true)) { return value; }\n",
        "  return 0;\n",
        "}\n",
    ));
    let main = hir.definitions.get(hir.entry_function).unwrap();
    let calls = main
        .body
        .statements
        .iter()
        .filter_map(|statement| match statement {
            HirStatement::Local(local) => match &local.initializer {
                HirLocalInitializer::Value(value) => match &value.kind {
                    HirExpressionKind::IndirectCall(call) => Some(call.as_ref()),
                    _ => None,
                },
                _ => None,
            },
            HirStatement::Conditional(conditional) => match &conditional.arms[0].condition.kind {
                HirExpressionKind::IndirectCall(call) => Some(call.as_ref()),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert_ne!(calls[0].function_type, calls[1].function_type);
    assert_eq!(calls[0].result, Type::I64);
    assert_eq!(calls[1].result, Type::Bool);
    assert_eq!(calls[0].callee.ty, Type::Function(calls[0].function_type));
    assert_eq!(calls[1].callee.ty, Type::Function(calls[1].function_type));
}
