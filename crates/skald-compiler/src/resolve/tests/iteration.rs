use super::*;
use crate::{
    diagnostics::LabelStyle,
    identity::{
        InterfaceTemplateId, InterfaceTemplateRequirementId, LocalId, LoopId, TypeParameterId,
    },
    test_support::{load_module_sources, CANONICAL_ITER_SOURCE},
};

fn source_slice(source: &str, span: crate::source::Span) -> &str {
    &source[span.range().start()..span.range().end()]
}

const APP: &str = "import std::iter;\nfn main() -> i64 { return 0; }\n";

fn resolve_iteration_module(source: &str) -> ResolveOutput {
    let (_workspace, graph) =
        load_module_sources("app", &[("app.ska", APP), ("std/iter.ska", source)]);
    resolve_module_graph(&graph)
}

fn resolve_iteration_app(source: &str) -> ResolveOutput {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[("app.ska", source), ("std/iter.ska", CANONICAL_ITER_SOURCE)],
    );
    resolve_module_graph(&graph)
}

const COUNTER: &str = concat!(
    "from std::iter import Iterable;\n",
    "class Counter implements Iterable<i64, u64> {\n",
    "  init() {}\n",
    "  fn iter_state() -> u64 { return 0u; }\n",
    "  fn iter_next(mut ref state: u64) -> i64? { return none; }\n",
    "}\n",
);

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

#[test]
fn canonical_declaration_failure_labels_the_bad_component_and_requirement_site() {
    let source = concat!(
        "public interface Iterable<Item, State> {\n",
        "  fn iter_state() -> State;\n",
        "  fn iter_next(mut ref state: State) -> Item;\n",
        "}\n",
    );
    let output = resolve_iteration_module(source);
    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.code == INVALID_ITERABLE_LANGUAGE_ITEM
                && diagnostic.message.contains("must return `Item?`")
        })
        .expect("malformed result must have one focused language-item diagnostic");
    assert_eq!(diagnostic.labels.len(), 2);
    assert_eq!(diagnostic.labels[0].style, LabelStyle::Primary);
    assert_eq!(source_slice(source, diagnostic.labels[0].span), "Item");
    assert_eq!(diagnostic.labels[0].message, "result has the wrong type");
    assert_eq!(diagnostic.labels[1].style, LabelStyle::Secondary);
    assert_eq!(
        diagnostic.labels[1].message,
        "iteration language item required here"
    );
}

#[test]
fn direct_claim_selects_exact_protocol_evidence_item_scope_and_loop_identity() {
    let source = format!(
        "{COUNTER}fn main(values: Counter) -> i64 {{\n  for (item in values) {{ var observed: i64 = item; continue; }}\n  return 0;\n}}\n"
    );
    let output = resolve_iteration_app(&source);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let main = output
        .program
        .definitions
        .get(output.program.entry_function.unwrap())
        .unwrap();
    let ResolvedStatement::ForIn(loop_) = &main.body.statements[0] else {
        panic!("expected a resolved iteration statement")
    };
    assert_eq!(loop_.loop_id, LoopId::new(main.function, 0));
    assert_eq!(loop_.binding, LocalId::new(main.function, 0));
    assert_eq!(loop_.selection.item, ResolvedTypeKind::I64);
    assert_eq!(loop_.selection.state, ResolvedTypeKind::U64);
    assert_eq!(
        loop_.selection.iter_state.interface(),
        loop_.selection.interface
    );
    assert_eq!(
        loop_.selection.iter_next.interface(),
        loop_.selection.interface
    );
    assert_eq!(main.locals[0].name, "item");
    assert_eq!(main.locals[0].type_syntax.kind, ResolvedTypeKind::I64);
    assert_eq!(main.locals[1].name, "observed");
    let ResolvedStatement::Continue(exit) = &loop_.body.statements[1] else {
        panic!("expected continue in the iteration body")
    };
    assert_eq!(exit.target, loop_.loop_id);

    let dump = dump_resolved(&output.program);
    assert!(dump.contains("Selection interface"), "{dump}");
    assert!(dump.contains("item i64 state u64 iter_state"), "{dump}");
}

#[test]
fn selection_supports_inherited_claims_and_exact_interface_views() {
    let source = format!(
        "{COUNTER}class Derived extends Counter {{ init() {{ super(); }} }}\nfn inherited(values: Derived) -> unit {{ for (item in values) {{}} }}\nfn viewed(values: Iterable<i64, u64>) -> unit {{ for (item in values) {{}} }}\nfn main() -> i64 {{ return 0; }}\n"
    );
    let output = resolve_iteration_app(&source);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let loops = output
        .program
        .definitions
        .iter()
        .filter_map(|definition| match &definition.body.statements[..] {
            [ResolvedStatement::ForIn(loop_)] => Some(loop_),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(loops.len(), 2);
    assert_eq!(loops[0].selection.interface, loops[1].selection.interface);
    assert_eq!(loops[0].selection.item, ResolvedTypeKind::I64);
}

#[test]
fn exact_annotation_filters_ambiguity_without_conversion_rules() {
    let source = concat!(
        "from std::iter import Iterable;\n",
        "class Both implements Iterable<i64, u64>, Iterable<u8, i64> { init() {} }\n",
        "fn choose(values: Both) -> unit { for (item: u8 in values) {} }\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let output = resolve_iteration_app(source);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let choose = output.program.definitions.iter().next().unwrap();
    let ResolvedStatement::ForIn(loop_) = &choose.body.statements[0] else {
        panic!("expected selected iteration")
    };
    assert_eq!(loop_.selection.item, ResolvedTypeKind::U8);
    assert_eq!(loop_.selection.state, ResolvedTypeKind::I64);

    let ambiguous = resolve_iteration_app(&source.replace("item: u8", "item"));
    assert!(ambiguous
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == AMBIGUOUS_ITERABLE_APPLICATION));

    let mismatch = resolve_iteration_app(&source.replace("item: u8", "item: bool"));
    assert!(mismatch
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == ITERATION_ITEM_TYPE_MISMATCH));
}

#[test]
fn selection_failures_label_the_actionable_header_and_conflicting_claims() {
    let source = concat!(
        "from std::iter import Iterable;\n",
        "class Both implements Iterable<i64, u64>, Iterable<u8, bool> { init() {} }\n",
        "fn missing(number: u64) -> unit { for (item in number) {} }\n",
        "fn ambiguous(values: Both) -> unit { for (item in values) {} }\n",
        "fn mismatch(values: Both) -> unit { for (item: bool in values) {} }\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let output = resolve_iteration_app(source);

    let missing = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == MISSING_ITERABLE_APPLICATION)
        .expect("primitive source must report missing nominal conformance");
    assert_eq!(missing.labels.len(), 1);
    assert_eq!(missing.labels[0].style, LabelStyle::Primary);
    assert_eq!(source_slice(source, missing.labels[0].span), "number");

    let ambiguous = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == AMBIGUOUS_ITERABLE_APPLICATION)
        .expect("unannotated source must report ambiguous conformance");
    assert_eq!(ambiguous.labels[0].style, LabelStyle::Primary);
    assert_eq!(source_slice(source, ambiguous.labels[0].span), "values");
    assert_eq!(ambiguous.labels.len(), 3);
    assert!(ambiguous.labels[1..]
        .iter()
        .all(|label| label.style == LabelStyle::Secondary));
    assert_eq!(
        ambiguous.labels[1..]
            .iter()
            .map(|label| source_slice(source, label.span))
            .collect::<Vec<_>>(),
        ["Iterable<i64, u64>", "Iterable<u8, bool>"]
    );

    let mismatch = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == ITERATION_ITEM_TYPE_MISMATCH)
        .expect("nonmatching annotation must report exact item mismatch");
    assert_eq!(mismatch.labels[0].style, LabelStyle::Primary);
    assert_eq!(source_slice(source, mismatch.labels[0].span), "bool");
    assert_eq!(mismatch.labels.len(), 3);
    assert!(mismatch.labels[1..]
        .iter()
        .all(|label| label.style == LabelStyle::Secondary));
}

#[test]
fn an_item_annotation_does_not_hide_distinct_state_ambiguity() {
    let source = concat!(
        "from std::iter import Iterable;\n",
        "class Both implements Iterable<i64, u64>, Iterable<i64, bool> { init() {} }\n",
        "fn scan(values: Both) -> unit { for (item: i64 in values) {} }\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let output = resolve_iteration_app(source);
    let diagnostics = output.diagnostics.iter().collect::<Vec<_>>();
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == AMBIGUOUS_ITERABLE_APPLICATION)
            .count(),
        1,
        "{:?}",
        output.diagnostics
    );
    assert_eq!(diagnostics[0].labels.len(), 3);
}

#[test]
fn specialized_generic_claim_is_selected_as_an_ordinary_exact_application() {
    let source = concat!(
        "from std::iter import Iterable;\n",
        "class Generic<T> implements Iterable<T, u64> {\n",
        "  init() {}\n",
        "  fn iter_state() -> u64 { return 0u; }\n",
        "  fn iter_next(mut ref state: u64) -> T? { return none; }\n",
        "}\n",
        "fn scan(values: Generic<i64>) -> unit { for (item in values) {} }\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let output = resolve_iteration_app(source);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let scan = output.program.definitions.iter().next().unwrap();
    let ResolvedStatement::ForIn(loop_) = &scan.body.statements[0] else {
        panic!("expected iteration over a specialized generic class")
    };
    assert_eq!(loop_.selection.item, ResolvedTypeKind::I64);
    assert_eq!(loop_.selection.state, ResolvedTypeKind::U64);
}

#[test]
fn nondependent_iteration_in_a_generic_body_retains_generic_type_uses() {
    let source = concat!(
        "from std::iter import Iterable;\n",
        "class Counter implements Iterable<i64, u64> {\n",
        "  init() {}\n",
        "  fn iter_state() -> u64 { return 0u; }\n",
        "  fn iter_next(mut ref state: u64) -> i64? { return none; }\n",
        "}\n",
        "class Scanner<T> {\n",
        "  init() {}\n",
        "  fn scan(values: Counter) -> unit {\n",
        "    for (item in values) {\n",
        "      var observed: i64 = item;\n",
        "      var retained: T? = none;\n",
        "    }\n",
        "  }\n",
        "}\n",
        "fn use(ref scanner: Scanner<u8>) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let output = resolve_iteration_app(source);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let loop_ = output
        .program
        .class_definitions
        .iter()
        .find_map(|definition| match definition.methods.as_slice() {
            [method]
                if matches!(
                    method.body.statements.first(),
                    Some(ResolvedStatement::ForIn(_))
                ) =>
            {
                let ResolvedStatement::ForIn(loop_) = &method.body.statements[0] else {
                    unreachable!()
                };
                Some(loop_)
            }
            _ => None,
        })
        .expect("the Scanner specialization has an iteration body");
    assert_eq!(loop_.body.statements.len(), 2);
}

#[test]
fn nested_specialization_preserves_definition_site_bound_selection() {
    let source = concat!(
        "from std::iter import Iterable;\n",
        "class Generic<T> implements Iterable<T, u64> { init() {} }\n",
        "class Scanner<Source> where Source: Iterable<i64, u64> {\n",
        "  fn scan(ref values: Source) -> unit { for (item in values) {} }\n",
        "}\n",
        "fn use(ref scanner: Scanner<Generic<i64>>) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let output = resolve_iteration_app(source);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let scanner = output
        .program
        .class_definitions
        .iter()
        .find(|definition| !definition.methods.is_empty())
        .unwrap();
    let ResolvedStatement::ForIn(loop_) = &scanner.methods[0].body.statements[0] else {
        panic!("expected nested-specialization iteration")
    };
    assert_eq!(loop_.selection.item, ResolvedTypeKind::I64);
    assert_eq!(loop_.selection.state, ResolvedTypeKind::U64);
}

#[test]
fn claim_declaration_order_does_not_change_exact_selection() {
    let source = |claims: &str| {
        format!(
            "from std::iter import Iterable;\nclass Both implements {claims} {{ init() {{}} }}\nfn scan(values: Both) -> unit {{ for (item: u8 in values) {{}} }}\nfn main() -> i64 {{ return 0; }}\n"
        )
    };
    let first = resolve_iteration_app(&source("Iterable<i64, u64>, Iterable<u8, bool>"));
    let second = resolve_iteration_app(&source("Iterable<u8, bool>, Iterable<i64, u64>"));
    assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
    assert!(second.diagnostics.is_empty(), "{:?}", second.diagnostics);
    let selection = |output: &ResolveOutput| {
        let definition = output.program.definitions.iter().next().unwrap();
        let ResolvedStatement::ForIn(loop_) = &definition.body.statements[0] else {
            panic!("expected selected iteration")
        };
        (
            loop_.selection.iter_state.index(),
            loop_.selection.iter_next.index(),
            loop_.selection.item,
            loop_.selection.state,
        )
    };
    assert_eq!(selection(&first), selection(&second));
}

#[test]
fn module_creation_order_does_not_change_resolved_iteration_products() {
    let source = format!(
        "{COUNTER}fn scan(values: Counter) -> unit {{ for (item in values) {{}} }}\nfn main() -> i64 {{ return 0; }}\n"
    );
    let sources = [
        ("app.ska", source.as_str()),
        ("std/iter.ska", CANONICAL_ITER_SOURCE),
    ];
    let (_first_workspace, first_graph) = load_module_sources("app", &sources);
    let (_second_workspace, second_graph) = load_module_sources("app", &[sources[1], sources[0]]);
    let first = resolve_module_graph(&first_graph);
    let second = resolve_module_graph(&second_graph);
    assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
    assert!(second.diagnostics.is_empty(), "{:?}", second.diagnostics);
    assert_eq!(
        dump_resolved(&first.program),
        dump_resolved(&second.program)
    );
}

#[test]
fn ambiguous_claim_declaration_order_has_identical_diagnostic_ordering() {
    let source = |claims: &str| {
        format!(
            "from std::iter import Iterable;\nclass Both implements {claims} {{ init() {{}} }}\nfn scan(values: Both) -> unit {{ for (item in values) {{}} }}\nfn main() -> i64 {{ return 0; }}\n"
        )
    };
    let first = resolve_iteration_app(&source("Iterable<i64, u64>, Iterable<f64, i64>"));
    let second = resolve_iteration_app(&source("Iterable<f64, i64>, Iterable<i64, u64>"));
    assert_eq!(
        first.diagnostics.iter().collect::<Vec<_>>(),
        second.diagnostics.iter().collect::<Vec<_>>()
    );
}

#[test]
fn structural_methods_and_noncanonical_interfaces_do_not_make_a_type_iterable() {
    let source = concat!(
        "from std::iter import Iterable;\n",
        "interface Lookalike<Item, State> {\n",
        "  fn iter_state() -> State;\n",
        "  fn iter_next(mut ref state: State) -> Item?;\n",
        "}\n",
        "class Structural implements Lookalike<i64, u64> {\n",
        "  init() {}\n",
        "  fn iter_state() -> u64 { return 0u; }\n",
        "  fn iter_next(mut ref state: u64) -> i64? { return none; }\n",
        "}\n",
        "fn scan(values: Structural) -> unit { for (item in values) {} }\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let output = resolve_iteration_app(source);
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == MISSING_ITERABLE_APPLICATION));
}

#[test]
fn generic_bound_selection_is_frozen_before_specialization() {
    let source = concat!(
        "from std::iter import Iterable;\n",
        "class Concrete implements Iterable<i64, u64>, Iterable<u8, bool> { init() {} }\n",
        "class Scanner<T> where T: Iterable<i64, u64> {\n",
        "  fn scan(ref values: T) -> i64 {\n",
        "    for (item in values) { var observed: i64 = item; break; }\n",
        "    return 0;\n",
        "  }\n",
        "}\n",
        "fn use(ref scanner: Scanner<Concrete>) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let output = resolve_iteration_app(source);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);
    assert!(dump.contains("Selection iteration"), "{dump}");
    assert!(dump.contains("ClosedIterationSelection"), "{dump}");

    let scanner = output
        .program
        .class_definitions
        .iter()
        .find(|definition| !definition.methods.is_empty())
        .expect("the Scanner specialization has a method body");
    let ResolvedStatement::ForIn(loop_) = &scanner.methods[0].body.statements[0] else {
        panic!("expected specialized bound-selected iteration")
    };
    assert_eq!(loop_.selection.item, ResolvedTypeKind::I64);
    assert_eq!(loop_.selection.state, ResolvedTypeKind::U64);
}

#[test]
fn generic_annotation_selects_one_bound_and_ignores_additional_concrete_claims() {
    let source = concat!(
        "from std::iter import Iterable;\n",
        "class Concrete implements Iterable<i64, u64>, Iterable<u8, bool>, Iterable<bool, i64> { init() {} }\n",
        "class Scanner<T> where T: Iterable<i64, u64>, T: Iterable<u8, bool> {\n",
        "  fn scan(ref values: T) -> unit { for (item: u8 in values) {} }\n",
        "}\n",
        "fn use(ref scanner: Scanner<Concrete>) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let output = resolve_iteration_app(source);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let scanner = output
        .program
        .class_definitions
        .iter()
        .find(|definition| !definition.methods.is_empty())
        .unwrap();
    let ResolvedStatement::ForIn(loop_) = &scanner.methods[0].body.statements[0] else {
        panic!("expected selected generic-bound iteration")
    };
    assert_eq!(loop_.selection.item, ResolvedTypeKind::U8);
    assert_eq!(loop_.selection.state, ResolvedTypeKind::Bool);
}

#[test]
fn iteration_binding_rejects_duplicate_outer_body_declarations_but_allows_nested_shadowing() {
    let duplicate = format!(
        "{COUNTER}fn scan(values: Counter) -> unit {{ for (item in values) {{ var item: i64 = 0; }} }}\nfn main() -> i64 {{ return 0; }}\n"
    );
    let duplicate = resolve_iteration_app(&duplicate);
    assert!(duplicate
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DUPLICATE_BINDING));

    let nested = format!(
        "{COUNTER}fn scan(values: Counter) -> unit {{ for (item in values) {{ {{ var item: i64 = 0; }} }} }}\nfn main() -> i64 {{ return 0; }}\n"
    );
    let nested = resolve_iteration_app(&nested);
    assert!(nested.diagnostics.is_empty(), "{:?}", nested.diagnostics);
}

#[test]
fn type_checking_builds_structured_hir_after_successful_selection() {
    let source = format!(
        "{COUNTER}fn scan(values: Counter) -> unit {{ for (item in values) {{}} }}\nfn main() -> i64 {{ return 0; }}\n"
    );
    let resolved = resolve_iteration_app(&source);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.expect("selected core iteration must reach HIR");
    let scan = hir.definitions.iter().next().unwrap();
    assert!(matches!(
        scan.body.statements[0],
        crate::hir::HirStatement::ForIn(_)
    ));
}

#[test]
fn iteration_binding_is_body_local_and_reuses_mixed_loop_exit_stack() {
    let source = format!(
        "{COUNTER}fn scan(values: Counter) -> i64 {{\n  while (true) {{\n    for (item in values) {{ while (true) {{ continue; }} break; }}\n    break;\n  }}\n  return item;\n}}\nfn main() -> i64 {{ return 0; }}\n"
    );
    let output = resolve_iteration_app(&source);
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == UNKNOWN_NAME));
    let scan = output.program.definitions.iter().next().unwrap();
    let ResolvedStatement::While(outer) = &scan.body.statements[0] else {
        panic!("expected outer while")
    };
    let ResolvedStatement::ForIn(loop_) = &outer.body.statements[0] else {
        panic!("expected inner iteration")
    };
    let ResolvedStatement::While(inner) = &loop_.body.statements[0] else {
        panic!("expected nested while")
    };
    let ResolvedStatement::Continue(continue_) = &inner.body.statements[0] else {
        panic!("expected nested continue")
    };
    let ResolvedStatement::Break(break_) = &loop_.body.statements[1] else {
        panic!("expected iteration break")
    };
    assert_eq!(continue_.target, inner.loop_id);
    assert_eq!(break_.target, loop_.loop_id);
    assert_eq!(outer.loop_id, LoopId::new(scan.function, 0));
    assert_eq!(loop_.loop_id, LoopId::new(scan.function, 1));
    assert_eq!(inner.loop_id, LoopId::new(scan.function, 2));
}
