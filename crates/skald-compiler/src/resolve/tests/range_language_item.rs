use super::*;
use crate::{
    identity::{InterfaceTemplateRequirementId, TypeParameterId},
    test_support::{load_module_sources, CANONICAL_RANGE_SOURCE},
};

const APP_IMPORT: &str = "import std::range;\nfn main() -> i64 { return 0; }\n";

fn resolve_range_module(source: &str) -> ResolveOutput {
    let (_workspace, graph) =
        load_module_sources("app", &[("app.ska", APP_IMPORT), ("std/range.ska", source)]);
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

    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert!(
        dump.contains("RangeLanguageItem successor-template"),
        "{dump}"
    );
    assert!(dump.contains("u8 canonical Successor<u8>"), "{dump}");
    assert!(dump.contains("u64 canonical Successor<u64>"), "{dump}");
    assert!(dump.contains("i64 canonical Successor<i64>"), "{dump}");
    assert!(!dump.contains("f64 canonical Successor"), "{dump}");
    assert!(!dump.contains("bool canonical Successor"), "{dump}");
}

#[test]
fn canonical_range_protocol_is_dependency_free_and_valid_as_an_entry() {
    let (_workspace, graph) =
        load_module_sources("std::range", &[("std/range.ska", CANONICAL_RANGE_SOURCE)]);
    assert_eq!(graph.modules().len(), 1);
    assert!(graph.modules()[0].imports().is_empty());

    let output = resolve_module_graph(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(output.program.range_language_item.is_some());
}

#[test]
fn successor_identities_ignore_source_creation_order() {
    let sources = [
        ("app.ska", APP_IMPORT),
        ("std/range.ska", CANONICAL_RANGE_SOURCE),
    ];
    let (_first_workspace, first_graph) = load_module_sources("app", &sources);
    let (_second_workspace, second_graph) = load_module_sources("app", &[sources[1], sources[0]]);
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
            format!("{declaration}\n{declaration}"),
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
