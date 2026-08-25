use super::*;
use crate::{
    identity::{InterfaceTemplateId, InterfaceTemplateRequirementId, TypeParameterId},
    test_support::{load_module_sources, CANONICAL_ITER_SOURCE},
};

const APP: &str = "import std::iter;\nfn main() -> i64 { return 0; }\n";

fn resolve_iteration_module(source: &str) -> ResolveOutput {
    let (_workspace, graph) =
        load_module_sources("app", &[("app.ska", APP), ("std/iter.ska", source)]);
    resolve_module_graph(&graph)
}

#[test]
fn canonical_iterable_language_item_retains_exact_template_identities() {
    let output = resolve_iteration_module(CANONICAL_ITER_SOURCE);
    assert!(
        output.diagnostics.is_empty(),
        "canonical iteration protocol must resolve: {:?}",
        output.diagnostics
    );

    let item = output
        .program
        .iterable_language_item
        .as_ref()
        .expect("an imported canonical iteration protocol must be recognized");
    assert_eq!(item.template, InterfaceTemplateId::new(0));
    assert_eq!(item.item_parameter, TypeParameterId::new(item.template, 0));
    assert_eq!(item.state_parameter, TypeParameterId::new(item.template, 1));
    assert_eq!(
        item.iter_state_requirement,
        InterfaceTemplateRequirementId::new(item.template, 0)
    );
    assert_eq!(
        item.iter_next_requirement,
        InterfaceTemplateRequirementId::new(item.template, 1)
    );
    assert_eq!(item.requiring_spans.len(), 1);
    assert!(output.program.generic_interface_specializations.is_empty());

    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert!(
        dump.contains(concat!(
            "IterableLanguageItem template interface-template0 ",
            "parameters interface-template0:type0 interface-template0:type1 ",
            "requirements interface-template0:requirement0 interface-template0:requirement1"
        )),
        "{dump}"
    );
}

#[test]
fn canonical_iteration_module_is_dependency_free_and_valid_as_an_entry() {
    let (_workspace, graph) =
        load_module_sources("std::iter", &[("std/iter.ska", CANONICAL_ITER_SOURCE)]);
    assert_eq!(graph.modules().len(), 1);
    assert!(graph.modules()[0].imports().is_empty());

    let output = resolve_module_graph(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(output.program.iterable_language_item.is_some());
}

#[test]
fn canonical_template_uses_the_existing_closed_specialization_coordinator() {
    let source = concat!(
        "from std::iter import Iterable;\n",
        "class Counter implements Iterable<i64, u64> {\n",
        "  fn iter_state() -> u64 { return 0u; }\n",
        "  fn iter_next(mut ref state: u64) -> i64? { return none; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let (_workspace, graph) = load_module_sources(
        "app",
        &[("app.ska", source), ("std/iter.ska", CANONICAL_ITER_SOURCE)],
    );
    let output = resolve_module_graph(&graph);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let specialization = output
        .program
        .generic_interface_specializations
        .iter()
        .next()
        .expect("the closed claim must request one specialization");
    assert_eq!(specialization.key.template, InterfaceTemplateId::new(0));
    assert_eq!(
        specialization.key.arguments,
        [ResolvedTypeKind::I64, ResolvedTypeKind::U64]
    );
    assert!(matches!(
        specialization.state,
        GenericInterfaceSpecializationState::Complete(_)
    ));
    assert_eq!(specialization.requirement_mappings.len(), 2);
}

#[test]
fn unrelated_iterable_spelling_is_not_a_language_item() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            ("app.ska", "import other;\nfn main() -> i64 { return 0; }\n"),
            ("other.ska", CANONICAL_ITER_SOURCE),
        ],
    );
    let output = resolve_module_graph(&graph);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(output.program.iterable_language_item.is_none());
}

#[test]
fn iterable_language_item_identities_ignore_source_creation_order() {
    let sources = [("app.ska", APP), ("std/iter.ska", CANONICAL_ITER_SOURCE)];
    let (_first_workspace, first_graph) = load_module_sources("app", &sources);
    let (_second_workspace, second_graph) = load_module_sources("app", &[sources[1], sources[0]]);
    let first = resolve_module_graph(&first_graph);
    let second = resolve_module_graph(&second_graph);

    assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
    assert!(second.diagnostics.is_empty(), "{:?}", second.diagnostics);
    assert_eq!(
        first.program.iterable_language_item,
        second.program.iterable_language_item
    );
    assert_eq!(
        dump_resolved(&first.program),
        dump_resolved(&second.program)
    );
}

#[test]
fn malformed_canonical_iterable_declarations_are_rejected_structurally() {
    let mutations = [
        ("missing declaration", "public interface Other<Item, State> {}\n"),
        ("wrong declaration kind", "public class Iterable<Item, State> {}\n"),
        (
            "private declaration",
            "interface Iterable<Item, State> {\n  fn iter_state() -> State;\n  fn iter_next(mut ref state: State) -> Item?;\n}\n",
        ),
        (
            "duplicate declaration",
            concat!(
                "public interface Iterable<Item, State> {\n",
                "  fn iter_state() -> State;\n",
                "  fn iter_next(mut ref state: State) -> Item?;\n",
                "}\n",
                "public interface Iterable<Item, State> {\n",
                "  fn iter_state() -> State;\n",
                "  fn iter_next(mut ref state: State) -> Item?;\n",
                "}\n",
            ),
        ),
        (
            "wrong arity",
            "public interface Iterable<Item> {\n  fn iter_state() -> Item;\n  fn iter_next(mut ref state: Item) -> Item?;\n}\n",
        ),
        (
            "wrong parameter names",
            "public interface Iterable<Value, Cursor> {\n  fn iter_state() -> Cursor;\n  fn iter_next(mut ref state: Cursor) -> Value?;\n}\n",
        ),
        (
            "generic bound",
            "interface Marker {}\npublic interface Iterable<Item, State> where Item: Marker {\n  fn iter_state() -> State;\n  fn iter_next(mut ref state: State) -> Item?;\n}\n",
        ),
        (
            "extra requirement",
            "public interface Iterable<Item, State> {\n  fn iter_state() -> State;\n  fn iter_next(mut ref state: State) -> Item?;\n  fn extra() -> unit;\n}\n",
        ),
        (
            "wrong requirement order",
            "public interface Iterable<Item, State> {\n  fn iter_next(mut ref state: State) -> Item?;\n  fn iter_state() -> State;\n}\n",
        ),
        (
            "mutable state receiver",
            "public interface Iterable<Item, State> {\n  mut fn iter_state() -> State;\n  fn iter_next(mut ref state: State) -> Item?;\n}\n",
        ),
        (
            "state parameters",
            "public interface Iterable<Item, State> {\n  fn iter_state(value: State) -> State;\n  fn iter_next(mut ref state: State) -> Item?;\n}\n",
        ),
        (
            "state result",
            "public interface Iterable<Item, State> {\n  fn iter_state() -> Item;\n  fn iter_next(mut ref state: State) -> Item?;\n}\n",
        ),
        (
            "mutable next receiver",
            "public interface Iterable<Item, State> {\n  fn iter_state() -> State;\n  mut fn iter_next(mut ref state: State) -> Item?;\n}\n",
        ),
        (
            "next parameter count",
            "public interface Iterable<Item, State> {\n  fn iter_state() -> State;\n  fn iter_next() -> Item?;\n}\n",
        ),
        (
            "next parameter name",
            "public interface Iterable<Item, State> {\n  fn iter_state() -> State;\n  fn iter_next(mut ref cursor: State) -> Item?;\n}\n",
        ),
        (
            "next parameter mode",
            "public interface Iterable<Item, State> {\n  fn iter_state() -> State;\n  fn iter_next(ref state: State) -> Item?;\n}\n",
        ),
        (
            "next parameter type",
            "public interface Iterable<Item, State> {\n  fn iter_state() -> State;\n  fn iter_next(mut ref state: Item) -> Item?;\n}\n",
        ),
        (
            "next result",
            "public interface Iterable<Item, State> {\n  fn iter_state() -> State;\n  fn iter_next(mut ref state: State) -> Item;\n}\n",
        ),
    ];

    for (name, source) in mutations {
        let output = resolve_iteration_module(source);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == INVALID_ITERABLE_LANGUAGE_ITEM),
            "{name} must produce a focused language-item diagnostic: {:?}",
            output.diagnostics
        );
        assert!(
            output.program.iterable_language_item.is_none(),
            "{name} must not publish canonical identities"
        );
    }
}
