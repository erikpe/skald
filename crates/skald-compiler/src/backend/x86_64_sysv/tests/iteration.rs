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

#[test]
fn immediate_integer_ranges_match_handwritten_while_instruction_shapes() {
    let source = concat!(
        "fn range_u8(start: u8, end: u8) -> unit { for (item in start .. end) {} }\n",
        "fn while_u8(start: u8, end: u8) -> unit {\n",
        "  var current: u8 = start; var boundary: u8 = end;\n",
        "  while (current < boundary) { var item: u8 = current; current = current + 1u8; }\n",
        "}\n",
        "fn range_u64(start: u64, end: u64) -> unit { for (item in start .. end) {} }\n",
        "fn while_u64(start: u64, end: u64) -> unit {\n",
        "  var current: u64 = start; var boundary: u64 = end;\n",
        "  while (current < boundary) { var item: u64 = current; current = current + 1u; }\n",
        "}\n",
        "fn range_i64(start: i64, end: i64) -> unit { for (item in start .. end) {} }\n",
        "fn while_i64(start: i64, end: i64) -> unit {\n",
        "  var current: i64 = start; var boundary: i64 = end;\n",
        "  while (current < boundary) { var item: i64 = current; current = current + 1; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  range_u8(1u8, 3u8); while_u8(1u8, 3u8);\n",
        "  range_u64(1u, 3u); while_u64(1u, 3u);\n",
        "  range_i64(1, 3); while_i64(1, 3);\n",
        "  return 0;\n",
        "}\n",
    );
    let output = range_assembly(source);
    assert_eq!(output, range_assembly(source));

    for (range, while_) in [
        ("range_u8", "while_u8"),
        ("range_u64", "while_u64"),
        ("range_i64", "while_i64"),
    ] {
        let range_body = named_source_function_assembly(&output, range);
        let while_body = named_source_function_assembly(&output, while_);
        let range_profile = instruction_profile(range_body);
        let while_profile = instruction_profile(while_body);
        assert_eq!(
            profile_without_unconditional_jumps(&range_profile),
            profile_without_unconditional_jumps(&while_profile),
            "{range}\n{range_body}\n{while_body}"
        );
        assert_eq!(
            range_profile
                .iter()
                .copied()
                .filter(|mnemonic| *mnemonic == "jmp")
                .count(),
            while_profile
                .iter()
                .copied()
                .filter(|mnemonic| *mnemonic == "jmp")
                .count()
                + 1,
            "fusion may add only its cold scalar-cleanup edge"
        );
        assert_eq!(
            range_profile
                .iter()
                .copied()
                .filter(|mnemonic| *mnemonic == "cmp")
                .count(),
            1
        );
        assert_eq!(
            range_profile
                .iter()
                .copied()
                .filter(|mnemonic| *mnemonic == "add")
                .count(),
            1
        );
        assert!(!range_profile
            .iter()
            .copied()
            .any(|mnemonic| mnemonic == "call"));
    }
    assert!(!output.contains("ska_rt_range"));
    assert!(
        !output.contains("Range_x3c_"),
        "fused-only range use must not retain erased canonical classes\n{output}"
    );
    assert!(
        include_str!("../../../../../../runtime/include/skald_runtime.h")
            .contains("#define SKALD_RUNTIME_ABI_VERSION UINT64_C(9)")
    );
    assert_system_assembler_accepts(&output);
}

#[test]
fn ordinary_range_execution_retains_canonical_artifacts() {
    let source = concat!(
        "from std::range import Range;\n",
        "fn main() -> i64 {\n",
        "  var values: Range<u64> = Range<u64>(1u, 4u);\n",
        "  var sum: u64 = 0u;\n",
        "  for (item in values) { sum = sum + item; }\n",
        "  return (i64) sum;\n",
        "}\n",
    );
    let output = standard_library_assembly(source);

    assert!(output.contains("Range_x3c_u64_x3e_"), "{output}");
    assert!(output.contains("method.iter_next"), "{output}");
    assert_system_assembler_accepts(&output);
    assert_eq!(run_native_assembly(&output).code(), Some(6));
}

#[test]
fn direct_class_range_execution_retains_canonical_artifacts() {
    let source = concat!(
        "from std::ops import OpLess;\n",
        "from std::range import Successor;\n",
        "class Value implements OpLess<Value>, Successor<Value> {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn op_less(ref rhs: Value) -> bool { return self.value < rhs.value; }\n",
        "  fn successor() -> Value { return Value(self.value + 1); }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var sum: i64 = 0;\n",
        "  for (item in Value(1) .. Value(4)) { sum = sum + item.value; }\n",
        "  return sum;\n",
        "}\n",
    );
    let output = standard_library_assembly(source);

    assert!(output.contains("Range_x3c_"), "{output}");
    assert!(output.contains("method.iter_next"), "{output}");
    assert_system_assembler_accepts(&output);
    assert_eq!(run_native_assembly(&output).code(), Some(6));
}

fn named_source_function_assembly<'a>(assembly: &'a str, name: &str) -> &'a str {
    let prefix = format!(".Lska.fn.app.{name}.f");
    let symbol = assembly
        .lines()
        .filter_map(|line| {
            line.strip_prefix(".type ")
                .and_then(|line| line.strip_suffix(", @function"))
        })
        .find(|symbol| symbol.starts_with(&prefix))
        .unwrap_or_else(|| panic!("assembly contains no source function named `{name}`"));
    function_assembly(assembly, symbol)
}

fn instruction_profile(function: &str) -> Vec<&str> {
    function
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            (!line.is_empty() && !line.starts_with('.') && !line.ends_with(':'))
                .then(|| line.split_ascii_whitespace().next().unwrap())
        })
        .collect()
}

fn profile_without_unconditional_jumps<'a>(profile: &[&'a str]) -> Vec<&'a str> {
    profile
        .iter()
        .copied()
        .filter(|mnemonic| *mnemonic != "jmp")
        .collect()
}

fn range_assembly(source: &str) -> String {
    let (hir, mir) = standard_library_hir_and_mir(source);
    assert_eq!(
        crate::hir::dump_hir(&hir).matches("PrimitiveRange").count(),
        3
    );
    let verified = crate::passes::run_mir_pipeline(mir).unwrap();
    crate::backend::emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&verified).with_reachable_artifacts_only(),
    )
    .unwrap()
}

fn standard_library_assembly(source: &str) -> String {
    let (_, mir) = standard_library_hir_and_mir(source);
    let verified = crate::passes::run_mir_pipeline(mir).unwrap();
    crate::backend::emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&verified).with_reachable_artifacts_only(),
    )
    .unwrap()
}

fn standard_library_hir_and_mir(source: &str) -> (crate::hir::HirProgram, crate::mir::MirProgram) {
    let (_workspace, graph) = crate::test_support::load_module_sources_with_standard_library(
        "app",
        &[("app.ska", source)],
    );
    let resolved = crate::resolve::resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(!checked.has_errors(), "{:?}", checked.diagnostics);
    let hir = checked.hir.expect("valid ranges must produce HIR");
    let mir = crate::test_support::lower_hir_to_final_mir(&hir);
    verify_mir(&mir).expect("matched range and while MIR must verify");
    (hir, mir)
}
