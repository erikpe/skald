use super::*;

const ITERATION_SOURCE: &str = concat!(
    "from std::iter import Iterable;\n",
    "class Counter implements Iterable<u64, u64> {\n",
    "  limit: u64;\n",
    "  init(limit: u64) { self.limit = limit; }\n",
    "  fn iter_state() -> u64 { return 0u; }\n",
    "  fn iter_next(mut ref state: u64) -> u64? {\n",
    "    if (state < self.limit) {\n",
    "      var item: u64 = state;\n",
    "      state = state + 1u;\n",
    "      return item;\n",
    "    }\n",
    "    return none;\n",
    "  }\n",
    "}\n",
    "class InheritedCounter extends Counter {\n",
    "  init(limit: u64) { super(limit); }\n",
    "}\n",
    "class Sum<Source> where Source: Iterable<u64, u64> {\n",
    "  init() {}\n",
    "  fn read(ref values: Source) -> u64 {\n",
    "    var result: u64 = 0u;\n",
    "    for (item in values) { result = result + item; }\n",
    "    return result;\n",
    "  }\n",
    "}\n",
    "fn main() -> i64 {\n",
    "  var values: InheritedCounter = InheritedCounter(4u);\n",
    "  var sum: Sum<InheritedCounter> = Sum<InheritedCounter>();\n",
    "  return (i64) sum.read(values);\n",
    "}\n",
);

#[test]
fn general_iteration_crosses_the_target_boundary_as_ordinary_verified_operations() {
    let (_workspace, graph) = crate::test_support::load_module_sources_with_standard_library(
        "app",
        &[("app.ska", ITERATION_SOURCE)],
    );
    let resolved = crate::resolve::resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(!checked.has_errors(), "{:?}", checked.diagnostics);
    let mir = crate::test_support::lower_hir_to_final_mir(
        &checked
            .hir
            .expect("valid iteration source must produce HIR"),
    );
    verify_mir(&mir).expect("compiler-generated iteration MIR must verify");

    let dump = crate::mir::dump_mir(&mir);
    assert!(dump.contains("call interface"), "{dump}");
    assert!(dump.contains("optional-presence"), "{dump}");
    assert!(!dump.contains("ForIn"), "{dump}");

    let first = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    let second = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    assert_eq!(first, second);
    assert!(first.matches("call r11").count() >= 2, "{first}");
    assert!(!first.contains("for_in"), "{first}");
    assert!(!first.contains("iterator"), "{first}");
    assert_system_assembler_accepts(&first);
    assert_eq!(run_native_assembly(&first).code(), Some(6));
}
