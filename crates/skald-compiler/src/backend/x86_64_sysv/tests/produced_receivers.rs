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
