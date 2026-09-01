use crate::{
    hir::dump_hir,
    mir::{dump_mir, lower_hir, verify_mir},
    resolve::{dump_resolved, resolve_module_graph, ResolvedCopyOperation},
    test_support::load_module_sources_with_standard_library,
    typeck::{type_check, COPY_OPERATION_UNAVAILABLE, INVALID_RANGE_CONSTRUCTION_ORIGIN},
};

fn resolve_range_source(source: &str) -> crate::resolve::ResolveOutput {
    let (_workspace, graph) =
        load_module_sources_with_standard_library("app", &[("app.ska", source)]);
    resolve_module_graph(&graph)
}

#[test]
fn immediate_integer_ranges_select_the_fused_structured_plan() {
    let resolved = resolve_range_source(concat!(
        "fn main() -> i64 {\n",
        "  for (byte in 1u8 .. 3u8) {}\n",
        "  for (wide in (1u) .. (3u)) {}\n",
        "  for (signed in -2 .. 1) {}\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );

    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let dump = dump_hir(&checked.hir.expect("concise range must produce HIR"));
    assert_eq!(
        dump.matches("PrimitiveRange endpoint=").count(),
        3,
        "{dump}"
    );
    assert_eq!(dump.matches("CanonicalRangeSyntax").count(), 3, "{dump}");
    assert!(!dump.contains("Protocol interface="), "{dump}");
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
    checked.hir.expect("valid ranges must produce HIR")
}

#[test]
fn explicit_range_values_and_direct_range_sources_share_ordinary_semantics() {
    let hir = check_range_source(concat!(
        "from std::range import Range;\n",
        "class Holder {\n",
        "  values: Range<u64>;\n",
        "  init(start: u64, end: u64) { self.values = Range<u64>(start, end); }\n",
        "}\n",
        "fn produce(start: u64, end: u64) -> Range<u64> { return Range<u64>(start, end); }\n",
        "fn consume(values: Range<u64>) -> u64 {\n",
        "  var total: u64 = 0u; for (value in values) { total = total + value; } return total;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var stored: Range<u64> = Range<u64>(1u, 4u);\n",
        "  var holder: Holder = Holder(2u, 5u);\n",
        "  var direct: u64 = consume(Range<u64>(3u, 6u));\n",
        "  var produced: u64 = consume(produce(4u, 7u));\n",
        "  var field: u64 = consume(holder.values);\n",
        "  for (value in (5u) .. (8u)) {}\n",
        "  return 0;\n",
        "}\n",
    ));
    let dump = dump_hir(&hir);
    assert_eq!(dump.matches("CanonicalRangeSyntax").count(), 1, "{dump}");
    assert!(dump.contains("ForIn"), "{dump}");
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("concise range consumers must lower through ordinary verified MIR");
    let mir_dump = dump_mir(&mir);
    assert!(!mir_dump.contains("RangeLoop"));
    assert!(!mir_dump.contains("skald_rt_range"));
}

#[test]
fn concise_class_ranges_retain_ordinary_witness_and_lifecycle_plans() {
    let hir = check_range_source(concat!(
        "from std::ops import OpLess;\n",
        "from std::range import Successor;\n",
        "class Value implements OpLess<Value>, Successor<Value> {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn op_less(ref rhs: Value) -> bool { return self.value < rhs.value; }\n",
        "  fn successor() -> Value { return Value(self.value + 1); }\n",
        "}\n",
        "fn main() -> i64 { for (value in Value(1) .. Value(4)) {} return 0; }\n",
    ));
    let dump = dump_hir(&hir);
    assert!(dump.contains("realization=class-witness"), "{dump}");
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("class range must lower through ordinary witness iteration");
    assert!(dump_mir(&mir).contains("call interface"));
}

#[test]
fn fusion_excludes_every_non_immediate_or_nonprimitive_iteration_boundary() {
    let hir = check_range_source(concat!(
        "from std::iter import Iterable;\n",
        "from std::ops import OpLess;\n",
        "from std::range import Range, Successor;\n",
        "class Value implements OpLess<Value>, Successor<Value> {\n",
        "  value: i64; init(value: i64) { self.value = value; }\n",
        "  fn op_less(ref rhs: Value) -> bool { return self.value < rhs.value; }\n",
        "  fn successor() -> Value { return Value(self.value + 1); }\n",
        "}\n",
        "class Counter implements Iterable<u64, u64> {\n",
        "  init() {} fn iter_state() -> u64 { return 0u; }\n",
        "  fn iter_next(mut ref state: u64) -> u64? { return none; }\n",
        "}\n",
        "class Derived extends Counter { init() { super(); } }\n",
        "class Scanner<T> where T: OpLess<T>, T: Successor<T> {\n",
        "  init() {} fn scan(start: T, end: T) -> unit { for (item in start .. end) {} }\n",
        "  fn concrete() -> unit { for (item in 4u .. 6u) {} }\n",
        "}\n",
        "fn retain_specialization(ref scanner: Scanner<u64>) -> unit {}\n",
        "fn interface_loop(ref values: Iterable<u64, u64>) -> unit {\n",
        "  for (item in values) {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  for (item in 0u .. 2u) {}\n",
        "  for (item in Range<u64>(0u, 2u)) {}\n",
        "  var stored: Range<u64> = Range<u64>(0u, 2u); for (item in stored) {}\n",
        "  for (item in Value(0) .. Value(2)) {}\n",
        "  var counter: Counter = Counter(); for (item in counter) {}\n",
        "  var derived: Derived = Derived(); for (item in derived) {}\n",
        "  return 0;\n",
        "}\n",
    ));
    let dump = dump_hir(&hir);
    assert_eq!(
        dump.matches("PrimitiveRange endpoint=").count(),
        2,
        "{dump}"
    );
    assert_eq!(dump.matches("Protocol interface=").count(), 7, "{dump}");
    assert_eq!(
        dump.matches("CanonicalRangeSyntax").count(),
        4,
        "direct primitive, class, and generic syntax ranges must retain provenance: {dump}"
    );
}

#[test]
fn specialized_generic_ranges_fuse_only_independently_concrete_endpoints() {
    let hir = check_range_source(concat!(
        "from std::ops import OpAdd;\n",
        "class Source implements OpAdd<Source, u64> {\n",
        "  raw: u64; init(value: u64) { self.raw = value; }\n",
        "  fn op_add(ref rhs: Source) -> u64 { return self.raw + rhs.raw; }\n",
        "}\n",
        "class Scanner<T> where T: OpAdd<T, u64> {\n",
        "  init() {}\n",
        "  fn dependent(ref lower: T, ref upper: T) -> unit {\n",
        "    for (item in (lower + upper) .. (upper + lower)) {}\n",
        "  }\n",
        "  fn transitive(ref lower: T, ref upper: T) -> unit {\n",
        "    var endpoint: u64 = 0u; endpoint = lower + upper;\n",
        "    for (item in endpoint .. endpoint) {}\n",
        "  }\n",
        "  fn concrete() -> unit { for (item in 1u .. 3u) {} }\n",
        "}\n",
        "fn retain(ref scanner: Scanner<Source>) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    verify_mir(&lower_hir(&hir))
        .expect("mixed specialized range plans must lower through verified MIR");
    let dump = dump_hir(&hir);
    assert_eq!(
        dump.matches("PrimitiveRange endpoint=").count(),
        1,
        "{dump}"
    );
    assert_eq!(dump.matches("Protocol interface=").count(), 2, "{dump}");
    assert!(
        dump.contains("provenance=(specialization-dependent, specialization-dependent)"),
        "{dump}"
    );
    assert!(
        dump.contains("provenance=(independent, independent)"),
        "{dump}"
    );
}

#[test]
fn canonical_range_origin_rejects_missing_forged_and_inconsistent_evidence() {
    use crate::resolve::{
        ResolvedConstructionOrigin, ResolvedExpression, ResolvedRangeProtocolRealization,
        ResolvedStatement, ResolvedTypeKind,
    };

    let resolved = resolve_range_source(concat!(
        "from std::range import Range;\n",
        "fn main() -> i64 {\n",
        "  for (concise in 1u .. 3u) {}\n",
        "  var explicit: Range<u64> = Range<u64>(1u, 3u);\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let entry = resolved.program.entry_function.unwrap();

    let assert_rejected = |mutate: &dyn Fn(&mut crate::resolve::ResolvedProgram)| {
        let mut program = resolved.program.clone();
        mutate(&mut program);
        let checked = type_check(&program);
        assert!(checked.hir.is_none());
        assert!(
            checked
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == INVALID_RANGE_CONSTRUCTION_ORIGIN),
            "{:?}",
            checked.diagnostics,
        );
    };

    fn construction_at(
        program: &mut crate::resolve::ResolvedProgram,
        entry: crate::identity::FunctionId,
        statement: usize,
    ) -> &mut crate::resolve::ResolvedConstructExpr {
        let definition = program.definitions.get_mut_for_test(entry).unwrap();
        let expression = match &mut definition.body.statements[statement] {
            ResolvedStatement::ForIn(loop_) => &mut loop_.iterable,
            ResolvedStatement::Local(local) => &mut local.initializer,
            _ => panic!("expected range-producing statement"),
        };
        let ResolvedExpression::Construct(construction) = expression else {
            panic!("expected construction");
        };
        construction
    }

    let missing = move |program: &mut crate::resolve::ResolvedProgram| {
        construction_at(program, entry, 0).origin = ResolvedConstructionOrigin::Explicit;
    };
    assert_rejected(&missing);

    let wrong_endpoint = move |program: &mut crate::resolve::ResolvedProgram| {
        let ResolvedConstructionOrigin::CanonicalRangeSyntax(origin) =
            &mut construction_at(program, entry, 0).origin
        else {
            unreachable!()
        };
        origin.endpoint_type = ResolvedTypeKind::U8;
    };
    assert_rejected(&wrong_endpoint);

    let wrong_realization = move |program: &mut crate::resolve::ResolvedProgram| {
        let ResolvedConstructionOrigin::CanonicalRangeSyntax(origin) =
            &mut construction_at(program, entry, 0).origin
        else {
            unreachable!()
        };
        origin.successor.realization = ResolvedRangeProtocolRealization::ClassWitness;
    };
    assert_rejected(&wrong_realization);

    let forged_provenance = move |program: &mut crate::resolve::ResolvedProgram| {
        let ResolvedConstructionOrigin::CanonicalRangeSyntax(origin) =
            &mut construction_at(program, entry, 0).origin
        else {
            unreachable!()
        };
        origin.endpoint_provenance[0] =
            crate::resolve::ResolvedRangeEndpointProvenance::SpecializationDependent;
    };
    assert_rejected(&forged_provenance);

    let forged = move |program: &mut crate::resolve::ResolvedProgram| {
        let origin = construction_at(program, entry, 0).origin.clone();
        construction_at(program, entry, 1).origin = origin;
    };
    assert_rejected(&forged);
}

#[test]
fn primitive_range_plan_rejects_shape_operation_and_realization_mutations() {
    use crate::hir::{
        HirBinaryOperation, HirComparisonPredicate, HirRangeProtocolRealization, HirStatement, Type,
    };

    let hir = check_range_source("fn main() -> i64 { for (value in 1u .. 3u) {} return 0; }\n");
    let definition = hir.definitions.get(hir.entry_function).unwrap();
    let HirStatement::ForIn(loop_) = &definition.body.statements[0] else {
        panic!("expected range loop");
    };
    let plan = loop_
        .primitive_range_plan()
        .expect("immediate integer syntax must select primitive range iteration")
        .clone();

    let assert_rejected =
        |name: &str, mutate: fn(&mut crate::hir::HirPrimitiveRangeIterationPlan)| {
            let mut candidate = plan.clone();
            mutate(&mut candidate);
            let rejected = std::panic::catch_unwind(|| {
                crate::hir::HirForIn::new_primitive_range(
                    loop_.loop_id,
                    loop_.binding,
                    candidate,
                    loop_.body.clone(),
                    loop_.spans,
                )
            });
            assert!(rejected.is_err(), "{name} must be rejected before MIR");
        };
    assert_rejected("wrong endpoint type", |candidate| {
        candidate.origin.endpoint_type = Type::U8;
    });
    assert_rejected("wrong primitive realization", |candidate| {
        candidate.origin.successor.realization =
            HirRangeProtocolRealization::PrimitiveIntrinsic(Type::U8);
    });
    assert_rejected("specialization-dependent endpoint", |candidate| {
        candidate.origin.endpoint_provenance[0] =
            crate::resolve::ResolvedRangeEndpointProvenance::SpecializationDependent;
    });
    assert_rejected("wrong comparison", |candidate| {
        candidate.comparison.predicate = HirComparisonPredicate::LessEqual;
    });
    assert_rejected("wrong increment", |candidate| {
        candidate.increment = HirBinaryOperation::SubtractU64;
    });
    assert_rejected("wrong item epoch type", |candidate| {
        candidate.item.value.ty = Type::U8;
    });
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
