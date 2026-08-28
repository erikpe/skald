use crate::{
    hir::dump_hir,
    mir::{dump_mir, lower_hir, verify_mir},
    resolve::{dump_resolved, resolve_module_graph, ResolvedCopyOperation},
    test_support::load_module_sources_with_standard_library,
    typeck::{type_check, COPY_OPERATION_UNAVAILABLE, RANGE_HIR_PENDING},
};

fn resolve_range_source(source: &str) -> crate::resolve::ResolveOutput {
    let (_workspace, graph) =
        load_module_sources_with_standard_library("app", &[("app.ska", source)]);
    resolve_module_graph(&graph)
}

#[test]
fn concise_range_stops_at_the_explicit_frontend_hir_gate() {
    let resolved =
        resolve_range_source("fn main() -> i64 { for (item in 1u .. 3u) {} return 0; }\n");
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );

    let checked = type_check(&resolved.program);
    assert!(checked.hir.is_none());
    assert_eq!(
        checked
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == RANGE_HIR_PENDING)
            .count(),
        1,
        "{:?}",
        checked.diagnostics,
    );
}

fn check_range_source(source: &str) -> crate::hir::HirProgram {
    let resolved = resolve_range_source(source);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    checked.hir.expect("valid explicit ranges must produce HIR")
}

#[test]
fn primitive_ranges_use_static_bounds_and_the_ordinary_iteration_plan() {
    let source = concat!(
        "from std::range import Range;\n",
        "fn scan() -> unit {\n",
        "  for (byte in Range<u8>(1u8, 3u8)) {}\n",
        "  for (wide in Range<u64>(1u, 3u)) {}\n",
        "  for (signed in Range<i64>(-2, 1)) {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let resolved = resolve_range_source(source);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let resolved_dump = dump_resolved(&resolved.program);
    for operation in [
        "LessU8",
        "LessU64",
        "LessI64",
        "AddOneU8",
        "AddOneU64",
        "AddOneI64",
    ] {
        assert!(
            resolved_dump.contains(operation),
            "missing {operation}:\n{resolved_dump}"
        );
    }

    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.expect("primitive ranges must produce HIR");
    let hir_dump = dump_hir(&hir);
    assert_eq!(hir_dump.matches("ForIn").count(), 3, "{hir_dump}");
    assert!(hir_dump.contains("Requirements iter_state="), "{hir_dump}");
    assert!(hir_dump.contains("Receiver iterable=class"), "{hir_dump}");
    assert!(!hir_dump.contains("RangeLoop"), "{hir_dump}");

    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("ordinary primitive range iteration must produce valid MIR");
    let mir_dump = dump_mir(&mir);
    assert!(mir_dump.contains("call interface"), "{mir_dump}");
    assert!(!mir_dump.contains("skald_rt_range"), "{mir_dump}");
}

#[test]
fn class_ranges_and_generic_consumers_retain_witness_dispatch() {
    let hir = check_range_source(concat!(
        "from std::iter import Iterable;\n",
        "from std::ops import OpLess;\n",
        "from std::range import Range, Successor;\n",
        "class Value implements OpLess<Value>, Successor<Value> {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn op_less(ref rhs: Value) -> bool { return self.value < rhs.value; }\n",
        "  fn successor() -> Value { return Value(self.value + 1); }\n",
        "}\n",
        "class Scanner<R> where R: Iterable<Value, Value> {\n",
        "  init() {}\n",
        "  fn scan(values: R) -> unit { for (item in values) {} }\n",
        "}\n",
        "fn use(ref scanner: Scanner<Range<Value>>) -> unit {}\n",
        "fn main() -> i64 {\n",
        "  for (item in Range<Value>(Value(1), Value(3))) {}\n",
        "  return 0;\n",
        "}\n",
    ));
    let dump = dump_hir(&hir);
    assert!(dump.contains("ForIn"), "{dump}");
    assert!(dump.contains("ObjectCall interface"), "{dump}");
    verify_mir(&lower_hir(&hir)).expect("class ranges must remain ordinary verified iteration");
}

#[test]
fn range_capabilities_fail_at_the_ordinary_generic_use_sites() {
    let mut resolved = resolve_range_source(concat!(
        "from std::ops import OpLess;\n",
        "from std::range import Range, Successor;\n",
        "class Value implements OpLess<Value>, Successor<Value> {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn op_less(ref rhs: Value) -> bool { return self.value < rhs.value; }\n",
        "  fn successor() -> Value { return Value(self.value + 1); }\n",
        "}\n",
        "fn use(ref values: Range<Value>) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let value = resolved
        .program
        .classes
        .iter()
        .find(|class| class.name == "Value")
        .expect("fixture declares Value")
        .id;
    let declaration = &mut resolved.program.classes.entries_mut_for_test()[value.index()];
    declaration.copy_constructor = ResolvedCopyOperation::Unavailable;
    declaration.copy_assignment = ResolvedCopyOperation::Unavailable;

    let checked = type_check(&resolved.program);
    assert!(checked.hir.is_none());
    let failures = checked
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == COPY_OPERATION_UNAVAILABLE)
        .collect::<Vec<_>>();
    assert!(
        failures
            .iter()
            .any(|diagnostic| diagnostic.message.contains("copy construction")),
        "{:?}",
        checked.diagnostics
    );
    assert!(
        failures
            .iter()
            .any(|diagnostic| diagnostic.message.contains("copy assignment")),
        "{:?}",
        checked.diagnostics
    );
}
