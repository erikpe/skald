use super::*;

const PRODUCER_PRESSURE_SOURCE: &str = concat!(
    "class Product {\n",
    "  value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  static fn make_static(value: i64) -> Product { return Product(value); }\n",
    "  fn make_instance(value: i64) -> Product { return Product(value); }\n",
    "  fn next(amount: i64) -> Product { return Product(self.value + amount); }\n",
    "  fn pressured(flag: bool, a: i64, b: i64, c: i64, d: i64, e: i64,\n",
    "      f: i64, g: i64, x: f64, y: f64, z: f64) -> i64 {\n",
    "    if (flag) { return self.pressured(false, a, b, c, d, e, f, g, x, y, z); }\n",
    "    return self.value + a + b + c + d + e + f + g;\n",
    "  }\n",
    "}\n",
    "interface Producer { fn produce(value: i64) -> Product; }\n",
    "class Factory implements Producer {\n",
    "  init() {}\n",
    "  fn produce(value: i64) -> Product { return Product(value); }\n",
    "}\n",
    "fn make_direct(value: i64) -> Product { return Product(value); }\n",
    "fn zero_pressure(ref factory: Producer, ref seed: Product) -> i64 {\n",
    "  return make_direct(5).next(3).pressured(true, 0, 0, 0, 0, 0, 0, 0, 1.0, 2.0, 3.0)\n",
    "    + Product.make_static(9).pressured(true, 0, 0, 0, 0, 0, 0, 0, 1.0, 2.0, 3.0)\n",
    "    + seed.make_instance(10).pressured(true, 0, 0, 0, 0, 0, 0, 0, 1.0, 2.0, 3.0)\n",
    "    + factory.produce(15).pressured(true, 0, 0, 0, 0, 0, 0, 0, 1.0, 2.0, 3.0);\n",
    "}\n",
    "fn main() -> i64 {\n",
    "  var factory: Factory = Factory();\n",
    "  var seed: Product = Product(0);\n",
    "  return zero_pressure(factory, seed);\n",
    "}\n",
);

#[test]
fn exact_class_construction_and_call_results_execute_as_readonly_receivers() {
    let source = concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  static fn make_static(value: i64) -> Item { return Item(value); }\n",
        "  fn make_instance(value: i64) -> Item { return Item(value); }\n",
        "  fn read(extra: i64) -> i64 { return self.value + extra; }\n",
        "}\n",
        "interface Producer { fn produce(value: i64) -> Item; }\n",
        "class Factory implements Producer {\n",
        "  init() {}\n",
        "  fn produce(value: i64) -> Item { return Item(value); }\n",
        "}\n",
        "fn make_direct(value: i64) -> Item { return Item(value); }\n",
        "fn through_interface(ref producer: Producer) -> i64 {\n",
        "  return producer.produce(9).read(10);\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var item: Item = Item(0);\n",
        "  var factory: Factory = Factory();\n",
        "  return Item(1).read(2)\n",
        "    + make_direct(3).read(4)\n",
        "    + Item.make_static(5).read(6)\n",
        "    + item.make_instance(7).read(8)\n",
        "    + through_interface(factory);\n",
        "}\n",
    );
    let assembly = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();

    assert_system_assembler_accepts(&assembly);
    assert_eq!(run_native_assembly(&assembly).code(), Some(55));
}

#[test]
fn motivating_string_literal_and_vec_result_receivers_execute_natively() {
    let assembly = standard_library_assembly(concat!(
        "from std::str import Str;\n",
        "from std::vec import Vec;\n",
        "fn main() -> i64 {\n",
        "  var values: Vec<Str> = Vec<Str>();\n",
        "  values.push(\"tail\");\n",
        "  var generated: Str = \"item-\".concat(Str.from_i64(7));\n",
        "  return (i64) generated.index_get(5) - 55\n",
        "    + (i64) \"abc\".index_get(1) + (i64) values.last().index_get(0);\n",
        "}\n",
    ));

    assert_system_assembler_accepts(&assembly);
    assert_eq!(run_native_assembly(&assembly).code(), Some(214));
}

#[test]
fn chained_receiver_and_argument_temporaries_trace_exact_lifetime_order() {
    let source = concat!(
        "extern fn test_record_i64(value: i64) -> unit;\n",
        "class Trace {\n",
        "  marker: i64;\n",
        "  init(marker: i64) { self.marker = marker; }\n",
        "  fn next(marker: i64) -> Trace {\n",
        "    test_record_i64(self.marker);\n",
        "    return make_trace(marker);\n",
        "  }\n",
        "  fn combine(ref value: Trace) -> i64 {\n",
        "    test_record_i64(self.marker);\n",
        "    return self.marker + value.marker;\n",
        "  }\n",
        "  destroy { test_record_i64(self.marker); }\n",
        "}\n",
        "fn make_trace(marker: i64) -> Trace {\n",
        "  test_record_i64(marker); return Trace(marker);\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  return make_trace(1).next(2).combine(make_trace(3).next(4));\n",
        "}\n",
    );
    let mut assembly = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();
    assembly.push_str(trace_digit_recorder());

    let result = run_native_assembly_output(&assembly);
    assert_eq!(result.status.code(), Some(6));
    assert_eq!(result.stdout, b"1\n1\n2\n2\n3\n3\n4\n4\n2\n4\n3\n2\n1\n");
}

#[test]
fn selected_conditions_loop_epochs_returns_and_nested_arguments_trace_path_local_cleanup() {
    let source = concat!(
        "extern fn test_record_i64(value: i64) -> unit;\n",
        "class Trace {\n",
        "  marker: i64; truth: bool;\n",
        "  init(marker: i64, truth: bool) {\n",
        "    self.marker = marker; self.truth = truth;\n",
        "  }\n",
        "  fn read() -> bool { test_record_i64(self.marker); return self.truth; }\n",
        "  fn combine(value: bool) -> bool {\n",
        "    test_record_i64(self.marker); return value;\n",
        "  }\n",
        "  destroy { test_record_i64(self.marker); }\n",
        "}\n",
        "fn make_trace(marker: i64, truth: bool) -> Trace {\n",
        "  test_record_i64(marker); return Trace(marker, truth);\n",
        "}\n",
        "fn returned() -> bool { return make_trace(1, true).read(); }\n",
        "fn main() -> i64 {\n",
        "  if (false && make_trace(1, true).read()) {}\n",
        "  if (true || make_trace(2, true).read()) {}\n",
        "  if (true && make_trace(3, true).read()) {}\n",
        "  if (false || make_trace(4, true).read()) {}\n",
        "  if (make_trace(5, false).read()) {}\n",
        "  elif (make_trace(6, true).read()) {}\n",
        "  var count: i64 = 0;\n",
        "  while (make_trace(7 + count, count < 2).read()) { count = count + 1; }\n",
        "  if (returned()) {}\n",
        "  if (make_trace(2, true).combine(make_trace(3, true).read())) {}\n",
        "  return 0;\n",
        "}\n",
    );
    let mut assembly = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();
    assembly.push_str(trace_digit_recorder());

    let result = run_native_assembly_output(&assembly);
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(
        result.stdout,
        concat!(
            "3\n3\n3\n",
            "4\n4\n4\n",
            "5\n5\n5\n",
            "6\n6\n6\n",
            "7\n7\n7\n",
            "8\n8\n8\n",
            "9\n9\n9\n",
            "1\n1\n1\n",
            "2\n3\n3\n2\n3\n2\n",
        )
        .as_bytes()
    );
}

#[test]
fn terminating_production_and_arguments_preserve_non_unwinding_failure_order() {
    let trace_class = concat!(
        "extern fn test_record_i64(value: i64) -> unit;\n",
        "class Trace {\n",
        "  marker: i64;\n",
        "  init(marker: i64) { self.marker = marker; }\n",
        "  fn read(value: i64) -> i64 {\n",
        "    test_record_i64(self.marker); return self.marker + value;\n",
        "  }\n",
        "  destroy { test_record_i64(self.marker); }\n",
        "}\n",
        "fn make_trace(marker: i64) -> Trace {\n",
        "  test_record_i64(marker); return Trace(marker);\n",
        "}\n",
        "fn effect() -> i64 { test_record_i64(2); return 2; }\n",
    );
    let cases = [
        (
            concat!(
                "fn main() -> i64 {\n",
                "  var zero: i64 = 0;\n",
                "  return make_trace(1 / zero).read(effect());\n",
                "}\n",
            ),
            b"".as_slice(),
        ),
        (
            concat!(
                "fn main() -> i64 {\n",
                "  var zero: i64 = 0;\n",
                "  return make_trace(1).read(10 / zero);\n",
                "}\n",
            ),
            b"1\n".as_slice(),
        ),
    ];

    for (main, expected_stdout) in cases {
        let source = format!("{trace_class}{main}");
        let mut assembly = lower_source_to_assembly(&source, Target::X86_64SysV).unwrap();
        assembly.push_str(trace_digit_recorder());
        assembly.push_str(native_panic_reporter());
        let result = run_native_assembly_output(&assembly);

        assert!(!result.status.success());
        assert_eq!(result.stdout, expected_stdout);
        assert_eq!(result.stderr, b"panic: integer division by zero\n");
    }
}

#[test]
fn produced_derived_receivers_preserve_complete_identity_for_each_dispatch_form() {
    let source = concat!(
        "interface Readable { fn dynamic(extra: i64) -> i64; }\n",
        "class Root implements Readable {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn inherited(extra: i64) -> i64 { return self.value + extra; }\n",
        "  virtual fn dynamic(extra: i64) -> i64 { return self.value + extra; }\n",
        "}\n",
        "class Leaf extends Root {\n",
        "  additional: i64;\n",
        "  init(value: i64, additional: i64) { super(value); self.additional = additional; }\n",
        "  fn exact(extra: i64) -> i64 { return self.value + self.additional + extra; }\n",
        "  override fn dynamic(extra: i64) -> i64 {\n",
        "    return self.value + self.additional + extra;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  return Leaf(10, 5).exact(1)\n",
        "    + Leaf(10, 5).inherited(2)\n",
        "    + Leaf(10, 5).dynamic(3);\n",
        "}\n",
    );
    let assembly = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();

    assert!(assembly.contains("method.exact"), "{assembly}");
    assert!(assembly.contains("method.inherited"), "{assembly}");
    assert!(
        assembly.contains("call .Lska.class.main.Leaf.c1.method.dynamic.m1"),
        "{assembly}"
    );
    assert!(
        assembly.contains("lea rdx, [rip + .Lska.class.main.Leaf.c1.dispatch]"),
        "{assembly}"
    );
    assert_system_assembler_accepts(&assembly);
    assert_eq!(run_native_assembly(&assembly).code(), Some(46));
}

#[test]
fn every_call_result_producer_survives_recursion_and_register_stack_pressure() {
    let first = lower_source_to_assembly(PRODUCER_PRESSURE_SOURCE, Target::X86_64SysV).unwrap();
    let second = lower_source_to_assembly(PRODUCER_PRESSURE_SOURCE, Target::X86_64SysV).unwrap();

    assert_eq!(first, second);
    assert!(first.contains("call r11"), "{first}");
    assert_system_assembler_accepts(&first);
    assert_eq!(run_native_assembly(&first).code(), Some(42));
}

#[test]
fn canonical_string_producers_cover_raw_bytes_composition_slicing_and_parsing() {
    let assembly = standard_library_assembly(concat!(
        "from std::str import Str;\n",
        "fn main() -> i64 {\n",
        "  if (\"A\\0\\x80\\xff\".index_get(1) != 0u8) { return 1; }\n",
        "  if (\"A\\0\\x80\\xff\".slice_get(1, -1).index_get(-1) != 128u8) { return 2; }\n",
        "  if (\"A\".concat(\"\\xff\").index_get(1) != 255u8) { return 3; }\n",
        "  if (Str.from_i64(-42).to_i64()! != -42) { return 4; }\n",
        "  var bytes: u8[] = u8[]{65u8, 0u8, 255u8};\n",
        "  if (Str.from_bytes(bytes).index_get(-1) != 255u8) { return 5; }\n",
        "  return 42;\n",
        "}\n",
    ));

    assert_system_assembler_accepts(&assembly);
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn closed_generic_exact_results_execute_for_vec_nested_results_and_bounds() {
    let assembly = standard_library_assembly(concat!(
        "from std::str import Str;\n",
        "from std::vec import Vec;\n",
        "interface Readable { fn read(extra: i64) -> i64; }\n",
        "class Item implements Readable {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Item) { self.value = source.value; }\n",
        "  assign(ref source: Item) { self.value = source.value; }\n",
        "  fn read(extra: i64) -> i64 { return self.value + extra; }\n",
        "}\n",
        "class Box<T> {\n",
        "  value: T;\n",
        "  init(value: T) { self.value = value; }\n",
        "  copy(ref source: Box<T>) { self.value = source.value; }\n",
        "  assign(ref source: Box<T>) { self.value = source.value; }\n",
        "  fn get() -> T { return self.value; }\n",
        "}\n",
        "class Outer<T> {\n",
        "  value: T;\n",
        "  init(value: T) { self.value = value; }\n",
        "  fn produce() -> Box<T> { return Box<T>(self.value); }\n",
        "}\n",
        "class Invoke<T> where T: Readable {\n",
        "  value: T;\n",
        "  init(value: T) { self.value = value; }\n",
        "  fn produce() -> T { return self.value; }\n",
        "  fn run() -> i64 { return self.produce().read(0); }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var values: Vec<Str> = Vec<Str>();\n",
        "  values.push(\"tail\");\n",
        "  var outer: Outer<Str> = Outer<Str>(\"A\");\n",
        "  var invoke: Invoke<Item> = Invoke<Item>(Item(42));\n",
        "  return (i64) values.last().index_get(0) - 116\n",
        "    + (i64) outer.produce().get().index_get(0) - 65\n",
        "    + invoke.run();\n",
        "}\n",
    ));

    assert!(assembly.contains("call r11"), "{assembly}");
    assert_system_assembler_accepts(&assembly);
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn owning_result_is_secured_before_receiver_cleanup_and_outlives_it() {
    let source = concat!(
        "extern fn test_record_i64(value: i64) -> unit;\n",
        "class Result {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn read() -> i64 { test_record_i64(4); return self.value; }\n",
        "  destroy { test_record_i64(5); }\n",
        "}\n",
        "class Trace {\n",
        "  marker: i64;\n",
        "  init(marker: i64) { self.marker = marker; }\n",
        "  fn make(value: i64) -> Result { test_record_i64(3); return Result(value + 40); }\n",
        "  destroy { test_record_i64(self.marker); }\n",
        "}\n",
        "fn effect() -> i64 { test_record_i64(2); return 2; }\n",
        "fn main() -> i64 {\n",
        "  var result: Result = Trace(1).make(effect());\n",
        "  var value: i64 = result.read();\n",
        "  return value;\n",
        "}\n",
    );
    let mut assembly = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();
    assembly.push_str(trace_digit_recorder());

    let result = run_native_assembly_output(&assembly);
    assert_eq!(result.status.code(), Some(42));
    assert_eq!(result.stdout, b"2\n3\n1\n4\n5\n");
}

#[test]
fn produced_receivers_keep_the_ordinary_layout_runtime_surface_and_abi_marker() {
    let program = lower_source_to_final_mir(PRODUCER_PRESSURE_SOURCE);
    let layouts = super::super::layout::DataLayout::compute(&program).unwrap();
    let product = program
        .classes
        .iter()
        .find(|class| class.name == "Product")
        .expect("fixture Product class must exist");
    assert_eq!(layouts.ty(MirType::Class(product.id)).unwrap().size(), 8);

    let assembly = emit_assembly(Target::X86_64SysV, &program).unwrap();
    let runtime_calls = assembly
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("call ska_rt_"))
        .collect::<Vec<_>>();
    assert_eq!(runtime_calls, ["call ska_rt_abi_v9"]);
    assert!(!assembly.contains("produced_receiver"));
    assert!(!assembly.contains("produced.receiver"));
    assert!(assembly.contains("call r11"), "{assembly}");
    assert_system_assembler_accepts(&assembly);

    let runtime_header = include_str!("../../../../../../runtime/include/skald_runtime.h");
    assert!(runtime_header.contains("#define SKALD_RUNTIME_ABI_VERSION UINT64_C(9)"));
    assert!(runtime_header.contains("#define SKALD_RUNTIME_ABI_MARKER ska_rt_abi_v9"));
}

fn standard_library_assembly(source: &str) -> String {
    let (_workspace, graph) = crate::test_support::load_module_sources_with_standard_library(
        "app",
        &[("app.ska", source)],
    );
    let resolved = crate::resolve::resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(!checked.has_errors(), "{:?}", checked.diagnostics);
    let mir = crate::test_support::lower_hir_to_final_mir(&checked.hir.unwrap());
    let mut assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    assembly.push_str(native_allocator());
    assembly
}

fn trace_digit_recorder() -> &'static str {
    concat!(
        ".text\n",
        ".globl test_record_i64\n",
        ".type test_record_i64, @function\n",
        "test_record_i64:\n",
        "    sub rsp, 8\n",
        "    add dil, 48\n",
        "    mov byte ptr [rsp], dil\n",
        "    mov byte ptr [rsp + 1], 10\n",
        "    mov eax, 1\n",
        "    mov edi, 1\n",
        "    mov rsi, rsp\n",
        "    mov edx, 2\n",
        "    syscall\n",
        "    add rsp, 8\n",
        "    ret\n",
        ".size test_record_i64, .-test_record_i64\n",
    )
}
