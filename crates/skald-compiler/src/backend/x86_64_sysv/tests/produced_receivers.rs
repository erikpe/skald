use super::*;

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
    let (_workspace, graph) = crate::test_support::load_module_sources_with_standard_library(
        "app",
        &[(
            "app.ska",
            concat!(
                "from std::str import Str;\n",
                "from std::vec import Vec;\n",
                "fn main() -> i64 {\n",
                "  var values: Vec<Str> = Vec<Str>();\n",
                "  values.push(\"tail\");\n",
                "  var generated: Str = \"item-\".concat(Str.from_i64(7));\n",
                "  return (i64) generated.byte(5) - 55\n",
                "    + (i64) \"abc\".byte(1) + (i64) values.last().byte(0);\n",
                "}\n",
            ),
        )],
    );
    let resolved = crate::resolve::resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(!checked.has_errors(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let mir = crate::test_support::lower_hir_to_final_mir(&hir);
    let mut assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    assembly.push_str(native_allocator());

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
