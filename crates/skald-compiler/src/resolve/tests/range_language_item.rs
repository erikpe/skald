use super::*;
use crate::{
    identity::{InterfaceTemplateRequirementId, TypeParameterId},
    test_support::{
        canonical_standard_library_sources, load_module_sources,
        load_module_sources_with_standard_library_overrides, CANONICAL_RANGE_SOURCE,
    },
};

fn resolve_range_syntax(source: &str) -> (crate::module::ModuleGraph, ResolveOutput) {
    let mut sources = vec![("app.ska", source)];
    sources.extend(canonical_standard_library_sources(&[]));
    let (_workspace, graph) = load_module_sources("app", &sources);
    let output = resolve_module_graph(&graph);
    (graph, output)
}

const APP_IMPORT: &str = "import std::range;\nfn main() -> i64 { return 0; }\n";

fn resolve_range_module(source: &str) -> ResolveOutput {
    let (_workspace, graph) = load_module_sources_with_standard_library_overrides(
        "app",
        &[("app.ska", APP_IMPORT)],
        &[("std/range.ska", source)],
    );
    resolve_module_graph(&graph)
}

fn replace_once(source: &str, before: &str, after: &str) -> String {
    assert!(
        source.contains(before),
        "missing mutation target `{before}`"
    );
    source.replacen(before, after, 1)
}

#[test]
fn canonical_successor_retains_exact_identities_and_static_evidence() {
    let output = resolve_range_module(CANONICAL_RANGE_SOURCE);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let item = output
        .program
        .range_language_item
        .as_ref()
        .expect("reachable canonical successor must be recognized");

    assert_eq!(
        item.successor_output_parameter,
        TypeParameterId::new(item.successor_template, 0)
    );
    assert_eq!(
        item.successor_requirement,
        InterfaceTemplateRequirementId::new(item.successor_template, 0)
    );
    assert_eq!(item.requiring_spans.len(), 1);
    assert_eq!(
        item.range_parameter,
        TypeParameterId::new(item.range_template, 0)
    );
    assert_eq!(item.range_ordering_bound, 0);
    assert_eq!(item.range_successor_bound, 1);
    assert_eq!(item.range_iterable_claim, 0);

    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert!(
        dump.contains("RangeLanguageItem successor-template"),
        "{dump}"
    );
    assert!(dump.contains("range-template"), "{dump}");
    assert!(dump.contains("u8 canonical Successor<u8>"), "{dump}");
    assert!(dump.contains("u64 canonical Successor<u64>"), "{dump}");
    assert!(dump.contains("i64 canonical Successor<i64>"), "{dump}");
    assert!(!dump.contains("f64 canonical Successor"), "{dump}");
    assert!(!dump.contains("bool canonical Successor"), "{dump}");
}

#[test]
fn concise_integer_range_activates_canonical_module_and_retains_resolved_evidence() {
    let (graph, output) =
        resolve_range_syntax("fn main() -> i64 { for (item in 1u .. 3u) {} return 0; }\n");
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let app = graph
        .modules()
        .iter()
        .find(|module| module.provenance().module_path().to_string() == "app")
        .expect("entry module is loaded");
    let range_edge = app
        .imports()
        .iter()
        .find(|edge| {
            graph
                .module(edge.target())
                .is_some_and(|module| module.provenance().module_path().to_string() == "std::range")
        })
        .expect("range syntax creates a compiler dependency");
    assert!(range_edge.import_spans().is_empty());
    assert_eq!(
        range_edge
            .compiler_dependency_spans(crate::module::CompilerDependencyKind::RangeForSource)
            .len(),
        1
    );

    let main = output
        .program
        .definitions
        .get(output.program.entry_function.expect("main is selected"))
        .expect("main resolves");
    let ResolvedStatement::ForIn(loop_) = &main.body.statements[0] else {
        panic!("expected resolved for-in");
    };
    let ResolvedForInSource::Range(range) = &loop_.source else {
        panic!("expected resolved range evidence");
    };
    assert_eq!(range.endpoint_type, ResolvedTypeKind::U64);
    assert_eq!(
        range.range_template,
        output
            .program
            .range_language_item
            .as_ref()
            .unwrap()
            .range_template
    );
    assert_eq!(range.initializer.class(), range.range_class);
    assert!(matches!(
        range.ordering.realization,
        ResolvedRangeProtocolRealization::PrimitiveIntrinsic(ResolvedPrimitiveType::U64)
    ));
    assert!(matches!(
        range.successor.realization,
        ResolvedRangeProtocolRealization::PrimitiveIntrinsic(ResolvedPrimitiveType::U64)
    ));

    let dump = dump_resolved(&output.program);
    assert!(dump.contains("RangeSource template"), "{dump}");
    assert!(dump.contains("realization primitive-u64"), "{dump}");
    assert_eq!(dump, dump_resolved(&output.program));
}

#[test]
fn concise_range_rejects_mixed_exact_endpoint_types_before_hir() {
    let (_graph, output) =
        resolve_range_syntax("fn main() -> i64 { for (item in 1u .. 3) {} return 0; }\n");
    assert!(
        output.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == RANGE_ENDPOINT_TYPE_MISMATCH
                && diagnostic.message.contains("exactly the same static type")
        }),
        "{:?}",
        output.diagnostics
    );
    assert!(output
        .program
        .entry_function
        .and_then(|main| output.program.definitions.get(main))
        .is_some_and(|definition| {
            definition.body.statements.len() == 1
                && matches!(definition.body.statements[0], ResolvedStatement::Return(_))
        }));
}

#[test]
fn concise_class_range_selects_nominal_ordering_and_successor_witnesses() {
    let (_graph, output) = resolve_range_syntax(concat!(
        "from std::ops import OpLess;\n",
        "from std::range import Successor;\n",
        "class Value implements OpLess<Value>, Successor<Value> {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn op_less(ref rhs: Value) -> bool { return self.value < rhs.value; }\n",
        "  fn successor() -> Value { return Value(self.value + 1); }\n",
        "}\n",
        "fn main() -> i64 { for (item in Value(1) .. Value(3)) {} return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let main = output
        .program
        .definitions
        .get(output.program.entry_function.unwrap())
        .unwrap();
    let ResolvedStatement::ForIn(loop_) = &main.body.statements[0] else {
        panic!("expected for-in");
    };
    let ResolvedForInSource::Range(range) = &loop_.source else {
        panic!("expected concise range");
    };
    assert!(matches!(range.endpoint_type, ResolvedTypeKind::Class(_)));
    assert_eq!(
        range.ordering.realization,
        ResolvedRangeProtocolRealization::ClassWitness
    );
    assert_eq!(
        range.successor.realization,
        ResolvedRangeProtocolRealization::ClassWitness
    );
}

#[test]
fn generic_template_range_requests_close_for_each_concrete_endpoint_type() {
    let (_graph, output) = resolve_range_syntax(concat!(
        "from std::ops import OpLess;\n",
        "from std::range import Successor;\n",
        "class Scanner<T> where T: OpLess<T>, T: Successor<T> {\n",
        "  init() {}\n",
        "  fn scan(start: T, end: T) -> unit { for (item: T in start .. end) {} }\n",
        "}\n",
        "fn use(scanner: Scanner<u64>) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);
    assert!(
        dump.contains("Selection range endpoint template0:type0"),
        "{dump}"
    );
    assert!(dump.contains("RangeSource template"), "{dump}");
}

#[test]
fn concise_range_requests_follow_endpoint_function_result_types() {
    let (_graph, output) = resolve_range_syntax(concat!(
        "fn lower() -> u8 { return 1u8; }\n",
        "fn upper() -> u8 { return 3u8; }\n",
        "fn main() -> i64 { for (item in lower() .. upper()) {} return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);
    assert!(dump.contains("endpoint u8"), "{dump}");
    assert!(dump.contains("realization primitive-u8"), "{dump}");
}

#[test]
fn concise_range_requests_follow_imported_function_result_types() {
    let mut sources = vec![
        (
            "app.ska",
            "from util import lower, upper;\nfn main() -> i64 { for (item in lower() .. upper()) {} return 0; }\n",
        ),
        (
            "util.ska",
            "public fn lower() -> u64 { return 1u; }\npublic fn upper() -> u64 { return 3u; }\n",
        ),
    ];
    sources.extend(canonical_standard_library_sources(&[]));
    let (_workspace, graph) = load_module_sources("app", &sources);
    let output = resolve_module_graph(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);
    assert!(dump.contains("RangeSource template"), "{dump}");
    assert!(dump.contains("endpoint u64"), "{dump}");
}

#[test]
fn concise_range_requests_follow_method_result_types() {
    let (_graph, output) = resolve_range_syntax(concat!(
        "from std::ops import OpLess;\n",
        "from std::range import Successor;\n",
        "class Endpoint implements OpLess<Endpoint>, Successor<Endpoint> {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn op_less(ref rhs: Endpoint) -> bool { return self.value < rhs.value; }\n",
        "  fn successor() -> Endpoint { return Endpoint(self.value + 1); }\n",
        "}\n",
        "class Factory {\n",
        "  init() {}\n",
        "  fn endpoint(value: i64) -> Endpoint { return Endpoint(value); }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var factory: Factory = Factory();\n",
        "  for (item in factory.endpoint(1) .. factory.endpoint(3)) {}\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);
    assert!(dump.contains("RangeSource template"), "{dump}");
    assert!(dump.contains("realization class-witness"), "{dump}");
    let checked = crate::typeck::type_check(&output.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
}

#[test]
fn concise_range_requests_follow_overloaded_operator_result_types() {
    let (_graph, output) = resolve_range_syntax(concat!(
        "from std::ops import OpAdd, OpLess;\n",
        "from std::range import Successor;\n",
        "class Endpoint implements OpLess<Endpoint>, Successor<Endpoint> {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn op_less(ref rhs: Endpoint) -> bool { return self.value < rhs.value; }\n",
        "  fn successor() -> Endpoint { return Endpoint(self.value + 1); }\n",
        "}\n",
        "class Factory implements OpAdd<Factory, Endpoint> {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn op_add(ref rhs: Factory) -> Endpoint { return Endpoint(self.value + rhs.value); }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var lower: Factory = Factory(1);\n",
        "  var upper: Factory = Factory(2);\n",
        "  for (item in (lower + lower) .. (upper + upper)) {}\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);
    assert!(dump.contains("RangeSource template"), "{dump}");
    assert!(dump.contains("realization class-witness"), "{dump}");
    let checked = crate::typeck::type_check(&output.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
}

#[test]
fn nested_concise_ranges_reuse_one_completed_semantic_specialization() {
    let (_graph, output) = resolve_range_syntax(concat!(
        "fn main() -> i64 {\n",
        "  var total: i64 = 0;\n",
        "  for (outer in 0 .. 2) {\n",
        "    for (inner in 1 .. 3) { total = total + outer + inner; }\n",
        "  }\n",
        "  return total;\n",
        "}\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let range_template = output
        .program
        .range_language_item
        .as_ref()
        .unwrap()
        .range_template;
    let specialization = output
        .program
        .generic_specializations
        .iter()
        .find(|specialization| {
            specialization.key.template == range_template
                && specialization.key.arguments.as_slice() == [ResolvedTypeKind::I64]
        })
        .expect("nested ranges reuse Range<i64>");
    assert_eq!(specialization.provenance.origins.len(), 2);

    let dump = dump_resolved(&output.program);
    assert_eq!(dump.matches("RangeSource template").count(), 2, "{dump}");
}

#[test]
fn deeply_nested_concise_ranges_discover_new_endpoint_types_to_a_fixpoint() {
    let (_graph, output) = resolve_range_syntax(concat!(
        "fn main() -> i64 {\n",
        "  for (signed in 0 .. 1) {\n",
        "    for (wide in 0u .. 1u) {\n",
        "      for (byte in 0u8 .. 1u8) {}\n",
        "    }\n",
        "  }\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let dump = dump_resolved(&output.program);
    assert_eq!(dump.matches("RangeSource template").count(), 3, "{dump}");
    assert!(dump.contains("endpoint i64"), "{dump}");
    assert!(dump.contains("endpoint u64"), "{dump}");
    assert!(dump.contains("endpoint u8"), "{dump}");
}

#[test]
fn failed_outer_range_does_not_request_ranges_from_its_unresolved_body() {
    let (_graph, output) = resolve_range_syntax(concat!(
        "fn main() -> i64 {\n",
        "  for (outer in 0.0 .. 1.0) {\n",
        "    for (inner in false .. true) {}\n",
        "  }\n",
        "  return 0;\n",
        "}\n",
    ));
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == UNSATISFIED_GENERIC_REQUIREMENT)
            .count(),
        1,
        "{:?}",
        output.diagnostics
    );
    assert!(
        !output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == UNSUPPORTED_RANGE_APPLICATION),
        "{:?}",
        output.diagnostics
    );
}

#[test]
fn semantic_range_request_probe_does_not_publish_body_diagnostics() {
    let (_graph, output) = resolve_range_syntax(concat!(
        "class Factory { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var factory: Factory = Factory();\n",
        "  for (item in factory.missing() .. factory.missing()) {}\n",
        "  return 0;\n",
        "}\n",
    ));
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == UNKNOWN_MEMBER)
            .count(),
        2,
        "{:?}",
        output.diagnostics
    );
    assert!(!output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == UNSUPPORTED_RANGE_APPLICATION));
}

#[test]
fn unsupported_primitive_range_reports_the_failed_canonical_bound_without_a_resolver_cascade() {
    let (_graph, output) =
        resolve_range_syntax("fn main() -> i64 { for (item in 1.0 .. 3.0) {} return 0; }\n");
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == UNSATISFIED_GENERIC_REQUIREMENT }),
        "{:?}",
        output.diagnostics
    );
    assert!(!output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == UNSUPPORTED_RANGE_APPLICATION));
}

#[test]
fn explicit_unsupported_ranges_do_not_publish_dependent_bound_member_diagnostics() {
    let (_graph, output) = resolve_range_syntax(concat!(
        "from std::range import Range;\n",
        "fn reject_float(ref values: Range<f64>) -> unit {}\n",
        "fn reject_bool(ref values: Range<bool>) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == UNSATISFIED_GENERIC_REQUIREMENT),
        "{:?}",
        output.diagnostics
    );
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != INVALID_MEMBER_SELECTION),
        "{:?}",
        output.diagnostics
    );
}

#[test]
fn concise_syntax_validates_replacement_canonical_range_declarations() {
    let broken = replace_once(CANONICAL_RANGE_SOURCE, "public class Range", "class Range");
    let (_workspace, graph) = load_module_sources_with_standard_library_overrides(
        "app",
        &[(
            "app.ska",
            "fn main() -> i64 { for (item in 1u .. 3u) {} return 0; }\n",
        )],
        &[("std/range.ska", &broken)],
    );
    let output = resolve_module_graph(&graph);
    assert!(
        output.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == INVALID_RANGE_LANGUAGE_ITEM
                && diagnostic.message.contains("must be public")
        }),
        "{:?}",
        output.diagnostics
    );
}

#[test]
fn canonical_range_bundle_has_only_its_two_foundational_dependencies() {
    let (_workspace, graph) = load_module_sources_with_standard_library_overrides(
        "std::range",
        &[],
        &[("std/range.ska", CANONICAL_RANGE_SOURCE)],
    );
    assert_eq!(graph.modules().len(), 3);
    let range = graph
        .modules()
        .iter()
        .find(|module| module.provenance().module_path().to_string() == "std::range")
        .expect("canonical range module must be loaded");
    assert_eq!(range.imports().len(), 2);

    let output = resolve_module_graph(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(output.program.range_language_item.is_some());
}

#[test]
fn successor_identities_ignore_source_creation_order() {
    let mut sources = vec![("app.ska", APP_IMPORT)];
    sources.extend(canonical_standard_library_sources(&[]));
    let (_first_workspace, first_graph) = load_module_sources("app", &sources);
    sources.reverse();
    let (_second_workspace, second_graph) = load_module_sources("app", &sources);
    let first = resolve_module_graph(&first_graph);
    let second = resolve_module_graph(&second_graph);

    assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
    assert!(second.diagnostics.is_empty(), "{:?}", second.diagnostics);
    assert_eq!(
        first.program.range_language_item,
        second.program.range_language_item
    );
    assert_eq!(
        dump_resolved(&first.program),
        dump_resolved(&second.program)
    );
}

#[test]
fn unreachable_and_same_named_foreign_successors_are_unrelated() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            ("app.ska", "fn main() -> i64 { return 0; }\n"),
            ("std/range.ska", "public interface Successor<T> {}\n"),
            ("other.ska", CANONICAL_RANGE_SOURCE),
        ],
    );
    assert_eq!(graph.modules().len(), 1);
    let output = resolve_module_graph(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(output.program.range_language_item.is_none());
}

#[test]
fn malformed_successor_components_are_rejected_structurally() {
    let declaration = CANONICAL_RANGE_SOURCE;
    let mutations = [
        (
            "missing declaration",
            String::new(),
            "does not declare the required `Successor`",
        ),
        (
            "duplicate declaration",
            format!(
                "{declaration}\npublic interface Successor<Output> {{ fn successor() -> Output; }}\n"
            ),
            "must declare `Successor` exactly once",
        ),
        (
            "private declaration",
            replace_once(declaration, "public interface", "interface"),
            "must be public",
        ),
        (
            "wrong declaration kind",
            "public class Successor<Output> {}\n".to_owned(),
            "must be a generic interface",
        ),
        (
            "wrong arity",
            replace_once(declaration, "Successor<Output>", "Successor<Left, Output>"),
            "must declare exactly one type parameter",
        ),
        (
            "wrong parameter name",
            replace_once(declaration, "Successor<Output>", "Successor<Value>"),
            "parameter must be named `Output`",
        ),
        (
            "generic bound",
            "interface Marker {}\npublic interface Successor<Output> where Output: Marker { fn successor() -> Output; }\n".to_owned(),
            "must not declare generic bounds",
        ),
        (
            "missing requirement",
            "public interface Successor<Output> {}\n".to_owned(),
            "must declare exactly one requirement",
        ),
        (
            "extra requirement",
            replace_once(
                declaration,
                "    fn successor() -> Output;",
                "    fn successor() -> Output;\n    fn extra() -> unit;",
            ),
            "must declare exactly one requirement",
        ),
        (
            "wrong requirement name",
            replace_once(declaration, "fn successor()", "fn next()"),
            "requirement must be named `successor`",
        ),
        (
            "mutable receiver",
            replace_once(declaration, "fn successor()", "mut fn successor()"),
            "must have a read-only receiver",
        ),
        (
            "unexpected parameter",
            replace_once(declaration, "successor()", "successor(value: Output)"),
            "must declare no parameters",
        ),
        (
            "wrong result",
            replace_once(declaration, "-> Output", "-> bool"),
            "must return `Output`",
        ),
    ];

    for (name, source, expected) in mutations {
        let output = resolve_range_module(&source);
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == INVALID_RANGE_LANGUAGE_ITEM
                    && diagnostic.message.contains(expected)
            }),
            "{name} must report a focused range language-item diagnostic: {:?}",
            output.diagnostics
        );
        assert!(
            output.program.range_language_item.is_none(),
            "{name} must not publish a malformed range language item"
        );
    }
}

#[test]
fn malformed_range_class_components_are_rejected_structurally() {
    let declaration = CANONICAL_RANGE_SOURCE;
    let class_offset = declaration
        .find("public class Range")
        .expect("canonical fixture declares Range");
    let protocol = &declaration[..class_offset];
    let mutations = [
        (
            "missing declaration",
            protocol.to_owned(),
            "does not declare the required `Range` class",
        ),
        (
            "duplicate declaration",
            format!("{declaration}\npublic class Range<X> {{ init() {{}} }}\n"),
            "must declare `Range` exactly once",
        ),
        (
            "private declaration",
            replace_once(declaration, "public class Range", "class Range"),
            "must be public",
        ),
        (
            "wrong declaration kind",
            format!("{protocol}public interface Range<T> {{}}\n"),
            "must be a generic class",
        ),
        (
            "wrong arity",
            replace_once(declaration, "class Range<T>", "class Range<T, Extra>"),
            "must declare exactly one type parameter",
        ),
        (
            "wrong parameter name",
            declaration.replace('T', "Value"),
            "parameter must be named `T`",
        ),
        (
            "direct base",
            replace_once(
                &replace_once(
                    declaration,
                    "public interface Successor",
                    "class Base { init() {} }\npublic interface Successor",
                ),
                "class Range<T> implements",
                "class Range<T> extends Base implements",
            ),
            "must not declare a direct base class",
        ),
        (
            "missing bound",
            replace_once(declaration, ", T: Successor<T>", ""),
            "must declare exactly two generic bounds",
        ),
        (
            "wrong first bound",
            replace_once(declaration, "T: OpLess<T>", "T: Successor<T>"),
            "first `std::range::Range` bound must be `T: OpLess<T>`",
        ),
        (
            "wrong second bound",
            replace_once(
                &replace_once(
                    declaration,
                    "from std::ops import OpLess;",
                    "from std::ops import OpLess, OpEq;",
                ),
                "T: Successor<T>",
                "T: OpEq<T>",
            ),
            "second `std::range::Range` bound must be `T: Successor<T>`",
        ),
        (
            "missing iterable claim",
            replace_once(declaration, " implements Iterable<T, T>", ""),
            "must declare exactly one interface claim",
        ),
        (
            "wrong iterable claim",
            replace_once(declaration, "Iterable<T, T>", "Iterable<T, u64>"),
            "must implement exactly `Iterable<T, T>`",
        ),
        (
            "missing initializer",
            replace_once(
                declaration,
                "    init(start: T, end: T) {\n        self._start = start;\n        self._end = end;\n    }\n\n",
                "",
            ),
            "must declare exactly one initializer",
        ),
        (
            "private initializer",
            replace_once(declaration, "    init(start", "    private init(start"),
            "must be public",
        ),
        (
            "initializer arity",
            replace_once(declaration, "init(start: T, end: T)", "init(start: T)"),
            "must declare exactly two parameters",
        ),
        (
            "initializer parameter name",
            replace_once(declaration, "init(start: T", "init(first: T"),
            "parameter must be named `start`",
        ),
        (
            "initializer binding mode",
            replace_once(declaration, "init(start: T", "init(ref start: T"),
            "must use owning value binding",
        ),
        (
            "initializer parameter type",
            replace_once(declaration, "init(start: T", "init(start: u64"),
            "parameters must have type `T`",
        ),
    ];

    for (name, source, expected) in mutations {
        let output = resolve_range_module(&source);
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == INVALID_RANGE_LANGUAGE_ITEM
                    && diagnostic.message.contains(expected)
            }),
            "{name} must report a focused range language-item diagnostic: {:?}",
            output.diagnostics
        );
        assert!(
            output.program.range_language_item.is_none(),
            "{name} must not publish a malformed range language item"
        );
    }
}
