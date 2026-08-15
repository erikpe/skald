use super::*;

#[test]
fn materializes_exact_callable_symbols_and_emits_receiverless_indirect_calls() {
    let source = concat!(
        "fn add(value: i64) -> i64 { return value + 1; }\n",
        "class Math {\n",
        "  init() {}\n",
        "  static fn double(value: i64) -> i64 { return value * 2; }\n",
        "}\n",
        "fn invoke(callback: fn(i64) -> i64, value: i64) -> i64 { return callback(value); }\n",
        "fn main() -> i64 {\n",
        "  var first: fn(i64) -> i64 = add;\n",
        "  var second: fn(i64) -> i64 = Math.double;\n",
        "  return invoke(first, 20) + invoke(second, 10);\n",
        "}\n",
    );

    let first = assembly(source);
    let second = assembly(source);

    assert_eq!(first, second);
    assert!(first.contains("lea rax, [rip + .Lska.fn.main.add.f0]"));
    assert!(first.contains("lea rax, [rip + .Lska.class.main.Math.c0.method.double.m0]"));
    assert!(first.contains("mov r11, qword ptr [rbp - 24]\n    call r11"));
    assert!(!first.contains("call r11\n    call r11"));
    let runtime_calls = first
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("call ska_rt_"))
        .collect::<Vec<_>>();
    assert_eq!(runtime_calls, ["call ska_rt_abi_v9"]);
    let runtime_header = include_str!("../../../../../../runtime/include/skald_runtime.h");
    assert!(runtime_header.contains("#define SKALD_RUNTIME_ABI_MARKER ska_rt_abi_v9"));
    assert_system_assembler_accepts(&first);
    assert_eq!(run_native_assembly(&first).code(), Some(41));
}

#[test]
fn preserves_mixed_register_classes_stack_pressure_and_function_results() {
    let output = assembly(concat!(
        "fn pressure(\n",
        "  a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64,\n",
        "  x0: f64, x1: f64, x2: f64, x3: f64, x4: f64, x5: f64, x6: f64, x7: f64, x8: f64\n",
        ") -> f64 { return x8 + 0.5; }\n",
        "fn choose() -> fn(i64, i64, i64, i64, i64, i64, i64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 {\n",
        "  return pressure;\n",
        "}\n",
        "fn invoke(callback: fn(i64, i64, i64, i64, i64, i64, i64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64) -> f64 {\n",
        "  return callback(1, 2, 3, 4, 5, 6, 7, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var callback: fn(i64, i64, i64, i64, i64, i64, i64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = choose();\n",
        "  if (invoke(callback) == 9.5) { return 42; }\n",
        "  return 1;\n",
        "}\n",
    ));

    assert!(output.contains("mov qword ptr [rsp], rax"), "{output}");
    assert!(
        output.contains("movsd qword ptr [rsp + 8], xmm14"),
        "{output}"
    );
    assert!(output.contains("call r11\n    add rsp"), "{output}");
    assert_system_assembler_accepts(&output);
    assert_eq!(run_native_assembly(&output).code(), Some(42));
}

#[test]
fn reuses_alias_aggregate_optional_shared_and_function_result_conventions() {
    let mut output = assembly(concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "}\n",
        "fn increment(value: i64) -> i64 { return value + 1; }\n",
        "fn mutate(mut ref item: Item, amount: i64) -> i64 {\n",
        "  item.value = item.value + amount;\n",
        "  return item.value;\n",
        "}\n",
        "fn make_item() -> Item { return Item(7); }\n",
        "fn make_values() -> i64[] { return i64[]{11, 13}; }\n",
        "fn make_maybe() -> i64? { return 17; }\n",
        "fn make_owner() -> shared Item { return new Item(19); }\n",
        "fn choose() -> fn(i64) -> i64 { return increment; }\n",
        "fn call_alias(callback: fn(mut ref Item, i64) -> i64, mut ref item: Item) -> i64 { return callback(item, 5); }\n",
        "fn call_item(callback: fn() -> Item) -> Item { return callback(); }\n",
        "fn call_values(callback: fn() -> i64[]) -> i64[] { return callback(); }\n",
        "fn call_maybe(callback: fn() -> i64?) -> i64? { return callback(); }\n",
        "fn call_owner(callback: fn() -> shared Item) -> shared Item { return callback(); }\n",
        "fn call_choice(callback: fn() -> fn(i64) -> i64) -> i64 { return callback()(20); }\n",
        "fn main() -> i64 {\n",
        "  var item: Item = Item(1);\n",
        "  var made: Item = call_item(make_item);\n",
        "  var values: i64[] = call_values(make_values);\n",
        "  var maybe: i64? = call_maybe(make_maybe);\n",
        "  var owner: shared Item = call_owner(make_owner);\n",
        "  var sum: i64 = call_alias(mutate, item) + made.value + values[1] + maybe! + owner->value + call_choice(choose);\n",
        "  return sum;\n",
        "}\n",
    ));
    output.push_str(native_allocator());

    assert_system_assembler_accepts(&output);
    assert_eq!(run_native_assembly(&output).code(), Some(83), "{output}");
}

#[test]
fn transports_inline_array_optional_and_shared_arguments_through_the_ordinary_abi() {
    let mut output = assembly(concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "}\n",
        "fn inspect(item: Item, values: i64[], maybe: i64?, owner: shared Item) -> i64 {\n",
        "  return item.value + values[1] + maybe! + owner->value;\n",
        "}\n",
        "fn invoke(\n",
        "  callback: fn(Item, i64[], i64?, shared Item) -> i64,\n",
        "  item: Item, values: i64[], maybe: i64?, owner: shared Item\n",
        ") -> i64 {\n",
        "  return callback(item, values, maybe, owner);\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var item: Item = Item(5);\n",
        "  var values: i64[] = i64[]{3, 7};\n",
        "  var maybe: i64? = 11;\n",
        "  var owner: shared Item = new Item(19);\n",
        "  return invoke(inspect, item, values, maybe, owner);\n",
        "}\n",
    ));
    output.push_str(native_allocator());

    assert_system_assembler_accepts(&output);
    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
}

#[test]
fn executes_fields_statics_and_closed_generic_static_targets() {
    let output = assembly(concat!(
        "fn add_one(value: i64) -> i64 { return value + 1; }\n",
        "class Holder {\n",
        "  callback: fn(i64) -> i64;\n",
        "  init(callback: fn(i64) -> i64) { self.callback = callback; }\n",
        "  fn apply(value: i64) -> i64 { return self.callback(value); }\n",
        "}\n",
        "class Registry {\n",
        "  static callback: fn(i64) -> i64 = add_one;\n",
        "  init() {}\n",
        "}\n",
        "class Identity<T> {\n",
        "  init() {}\n",
        "  static fn apply(value: T) -> T { return value; }\n",
        "}\n",
        "class Marker<T> {\n",
        "  init() {}\n",
        "  static fn apply(value: i64) -> i64 { return value; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var original: Holder = Holder(add_one);\n",
        "  var copy: Holder = original;\n",
        "  var specialized: fn(i64) -> i64 = Identity<i64>::apply;\n",
        "  var first_marker: fn(i64) -> i64 = Marker<i64>::apply;\n",
        "  var second_marker: fn(i64) -> i64 = Marker<bool>::apply;\n",
        "  return copy.apply(10) + Registry.callback(10) + specialized(20) + first_marker(0) + second_marker(0);\n",
        "}\n",
    ));

    assert!(output.contains(".Lska.class.main.Registry.c1.static.s0:\n    .zero 8"));
    assert!(output.contains("lea rax, [rip + .Lska.class.main.Identity"));
    assert_eq!(
        output
            .matches("lea rax, [rip + .Lska.class.main.Marker")
            .count(),
        2,
        "{output}"
    );
    assert!(output.matches("call r11").count() >= 5, "{output}");
    assert_system_assembler_accepts(&output);
    assert_eq!(run_native_assembly(&output).code(), Some(42));
}

#[test]
fn emits_address_taken_bodies_without_a_direct_or_indirect_call_edge() {
    let output = assembly(concat!(
        "fn retained_function() -> i64 { return 1; }\n",
        "class Utility {\n",
        "  init() {}\n",
        "  static fn retained_method() -> i64 { return 2; }\n",
        "}\n",
        "class Generic<T> {\n",
        "  init() {}\n",
        "  static fn retained_method() -> i64 { return 3; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var first: fn() -> i64 = retained_function;\n",
        "  var second: fn() -> i64 = Utility.retained_method;\n",
        "  var third: fn() -> i64 = Generic<i64>::retained_method;\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(output.contains(".type .Lska.fn.main.retained_function.f0, @function"));
    assert!(
        output.contains(".type .Lska.class.main.Utility.c0.method.retained_method.m0, @function")
    );
    assert!(output.contains(".type .Lska.class.main.Generic"));
    assert!(output.contains(".method.retained_method.m0, @function"));
    assert_eq!(output.matches("call r11").count(), 0, "{output}");
    assert_system_assembler_accepts(&output);
    assert_eq!(run_native_assembly(&output).code(), Some(0));
}
