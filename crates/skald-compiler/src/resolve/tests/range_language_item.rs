use super::*;
use crate::{
    identity::{InterfaceTemplateRequirementId, TypeParameterId},
    test_support::{
        canonical_standard_library_sources, load_module_sources,
        load_module_sources_with_standard_library_overrides, CANONICAL_RANGE_SOURCE,
    },
};

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
