use super::*;
use crate::{
    identity::{InterfaceTemplateId, InterfaceTemplateRequirementId, TypeParameterId},
    test_support::{
        load_module_sources, load_module_sources_with_standard_library_overrides,
        CANONICAL_OPS_SOURCE,
    },
    typeck::type_check,
};

const APP_IMPORT: &str = "import std::ops;\nfn main() -> i64 { return 0; }\n";
const OP_ADD: &str = concat!(
    "public interface OpAdd<Rhs, Output> {\n",
    "    fn op_add(ref rhs: Rhs) -> Output;\n",
    "}\n",
);

fn resolve_operator_module(source: &str) -> ResolveOutput {
    let (_workspace, graph) =
        load_module_sources("app", &[("app.ska", APP_IMPORT), ("std/ops.ska", source)]);
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
fn canonical_operator_bundle_retains_exact_protocol_identities() {
    let output = resolve_operator_module(CANONICAL_OPS_SOURCE);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let item = output
        .program
        .operator_language_item
        .as_ref()
        .expect("reachable canonical operator protocols must be recognized");

    assert_eq!(item.iter().len(), CanonicalOperatorProtocol::COUNT);
    for (index, kind) in CanonicalOperatorProtocol::ALL.into_iter().enumerate() {
        let protocol = item.get(kind);
        let template = InterfaceTemplateId::new(index);
        assert_eq!(protocol.kind, kind);
        assert_eq!(protocol.template, template);
        assert_eq!(
            protocol.requirement,
            InterfaceTemplateRequirementId::new(template, 0)
        );
        let expected = match kind.shape() {
            CanonicalOperatorProtocolShape::Unary => ResolvedOperatorProtocolParameters::Unary {
                output: TypeParameterId::new(template, 0),
            },
            CanonicalOperatorProtocolShape::Predicate => {
                ResolvedOperatorProtocolParameters::Predicate {
                    rhs: TypeParameterId::new(template, 0),
                }
            }
            CanonicalOperatorProtocolShape::Binary => ResolvedOperatorProtocolParameters::Binary {
                rhs: TypeParameterId::new(template, 0),
                output: TypeParameterId::new(template, 1),
            },
        };
        assert_eq!(protocol.parameters, expected);
    }
    assert_eq!(item.requiring_spans.len(), 1);

    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert!(dump.contains("OperatorLanguageItem\n"), "{dump}");
    assert!(dump.contains(concat!(
        "OpAdd template interface-template7 ",
        "rhs interface-template7:type0 output interface-template7:type1 ",
        "requirement interface-template7:requirement0"
    )));
}

#[test]
fn canonical_operator_module_is_dependency_free_and_valid_as_an_entry() {
    let (_workspace, graph) =
        load_module_sources("std::ops", &[("std/ops.ska", CANONICAL_OPS_SOURCE)]);
    assert_eq!(graph.modules().len(), 1);
    assert!(graph.modules()[0].imports().is_empty());

    let output = resolve_module_graph(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(output.program.operator_language_item.is_some());
}

#[test]
fn operator_language_item_identities_ignore_source_creation_order() {
    let sources = [
        ("app.ska", APP_IMPORT),
        ("std/ops.ska", CANONICAL_OPS_SOURCE),
    ];
    let (_first_workspace, first_graph) = load_module_sources("app", &sources);
    let (_second_workspace, second_graph) = load_module_sources("app", &[sources[1], sources[0]]);
    let first = resolve_module_graph(&first_graph);
    let second = resolve_module_graph(&second_graph);

    assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
    assert!(second.diagnostics.is_empty(), "{:?}", second.diagnostics);
    assert_eq!(
        first.program.operator_language_item,
        second.program.operator_language_item
    );
    assert_eq!(
        dump_resolved(&first.program),
        dump_resolved(&second.program)
    );
}

#[test]
fn unreachable_or_same_named_foreign_operator_protocols_are_irrelevant() {
    let malformed = "public interface OpAdd<Rhs> {}\n";
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            ("app.ska", "fn main() -> i64 { return (20 + 1) * 2; }\n"),
            ("std/ops.ska", malformed),
            ("other.ska", CANONICAL_OPS_SOURCE),
        ],
    );
    assert_eq!(graph.modules().len(), 1);
    let output = resolve_module_graph(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(output.program.operator_language_item.is_none());
    let checked = type_check(&output.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
}

#[test]
fn malformed_operator_bundle_components_are_rejected_structurally() {
    let mutations = [
        (
            "missing declaration",
            replace_once(CANONICAL_OPS_SOURCE, OP_ADD, ""),
            "does not declare the required `OpAdd`",
        ),
        (
            "duplicate declaration",
            format!("{CANONICAL_OPS_SOURCE}\n{OP_ADD}"),
            "must declare `OpAdd` exactly once",
        ),
        (
            "private declaration",
            replace_once(
                CANONICAL_OPS_SOURCE,
                "public interface OpAdd<Rhs, Output>",
                "interface OpAdd<Rhs, Output>",
            ),
            "`std::ops::OpAdd` must be public",
        ),
        (
            "wrong declaration kind",
            replace_once(
                CANONICAL_OPS_SOURCE,
                OP_ADD,
                "public class OpAdd<Rhs, Output> {}\n",
            ),
            "must be a generic interface",
        ),
        (
            "wrong arity",
            replace_once(
                CANONICAL_OPS_SOURCE,
                "public interface OpAdd<Rhs, Output>",
                "public interface OpAdd<Rhs>",
            ),
            "must declare exactly 2 type parameters",
        ),
        (
            "wrong parameter order",
            replace_once(
                CANONICAL_OPS_SOURCE,
                "public interface OpAdd<Rhs, Output>",
                "public interface OpAdd<Output, Rhs>",
            ),
            "first `std::ops::OpAdd` parameter must be named `Rhs`",
        ),
        (
            "generic bound",
            replace_once(
                CANONICAL_OPS_SOURCE,
                "public interface OpAdd<Rhs, Output> {",
                "interface Marker {}\npublic interface OpAdd<Rhs, Output> where Rhs: Marker {",
            ),
            "must not declare generic bounds",
        ),
        (
            "missing requirement",
            replace_once(
                CANONICAL_OPS_SOURCE,
                OP_ADD,
                "public interface OpAdd<Rhs, Output> {}\n",
            ),
            "must declare exactly one requirement",
        ),
        (
            "extra requirement",
            replace_once(
                CANONICAL_OPS_SOURCE,
                "    fn op_add(ref rhs: Rhs) -> Output;\n}",
                "    fn op_add(ref rhs: Rhs) -> Output;\n    fn extra() -> unit;\n}",
            ),
            "must declare exactly one requirement",
        ),
        (
            "wrong requirement name",
            replace_once(CANONICAL_OPS_SOURCE, "fn op_add(ref rhs", "fn add(ref rhs"),
            "requirement must be named `op_add`",
        ),
        (
            "mutable receiver",
            replace_once(
                CANONICAL_OPS_SOURCE,
                "fn op_add(ref rhs",
                "mut fn op_add(ref rhs",
            ),
            "must have a read-only receiver",
        ),
        (
            "wrong parameter count",
            replace_once(CANONICAL_OPS_SOURCE, "op_add(ref rhs: Rhs)", "op_add()"),
            "must declare exactly one parameter",
        ),
        (
            "unexpected unary parameter",
            replace_once(
                CANONICAL_OPS_SOURCE,
                "op_neg() -> Output",
                "op_neg(ref rhs: Output) -> Output",
            ),
            "`std::ops::OpNeg.op_neg` must declare no parameters",
        ),
        (
            "wrong parameter name",
            replace_once(
                CANONICAL_OPS_SOURCE,
                "op_add(ref rhs: Rhs)",
                "op_add(ref value: Rhs)",
            ),
            "parameter must be named `rhs`",
        ),
        (
            "wrong parameter mode",
            replace_once(
                CANONICAL_OPS_SOURCE,
                "op_add(ref rhs: Rhs)",
                "op_add(rhs: Rhs)",
            ),
            "parameter must use `ref`",
        ),
        (
            "wrong parameter type",
            replace_once(
                CANONICAL_OPS_SOURCE,
                "op_add(ref rhs: Rhs)",
                "op_add(ref rhs: Output)",
            ),
            "parameter must have type `Rhs`",
        ),
        (
            "wrong result",
            replace_once(
                CANONICAL_OPS_SOURCE,
                "op_add(ref rhs: Rhs) -> Output",
                "op_add(ref rhs: Rhs) -> Rhs",
            ),
            "must return `Output`",
        ),
        (
            "wrong predicate result",
            replace_once(
                CANONICAL_OPS_SOURCE,
                "op_eq(ref rhs: Rhs) -> bool",
                "op_eq(ref rhs: Rhs) -> Rhs",
            ),
            "must return `bool`",
        ),
    ];

    for (name, source, expected) in mutations {
        let output = resolve_operator_module(&source);
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == INVALID_OPERATOR_LANGUAGE_ITEM
                    && diagnostic.message.contains(expected)
            }),
            "{name} must report a focused operator language-item diagnostic: {:?}",
            output.diagnostics
        );
        assert!(
            output.program.operator_language_item.is_none(),
            "{name} must not publish a partial canonical bundle"
        );
    }
}

#[test]
fn simultaneous_bundle_defects_are_reported_in_canonical_order() {
    let source = CANONICAL_OPS_SOURCE
        .replace("public interface OpAdd", "interface OpAdd")
        .replace("public interface OpNeg", "interface OpNeg")
        .replace("public interface OpEq", "interface OpEq");
    let output = resolve_operator_module(&source);
    let messages = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == INVALID_OPERATOR_LANGUAGE_ITEM)
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(messages.len(), 3, "{:?}", output.diagnostics);
    assert!(messages[0].contains("OpNeg"), "{messages:?}");
    assert!(messages[1].contains("OpEq"), "{messages:?}");
    assert!(messages[2].contains("OpAdd"), "{messages:?}");
}

#[test]
fn canonical_bundle_failures_precede_generic_operator_selection_failures() {
    let malformed = replace_once(
        CANONICAL_OPS_SOURCE,
        "op_add(ref rhs: Rhs) -> Output",
        "op_add(rhs: Rhs) -> Output",
    );
    let app = concat!(
        "from std::ops import OpAdd;\n",
        "class Broken<T> where T: OpAdd<T, T> {\n",
        "  fn add(ref left: T, ref right: T) -> T { return left + right; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let (_workspace, graph) = load_module_sources(
        "app",
        &[("app.ska", app), ("std/ops.ska", malformed.as_str())],
    );
    let output = resolve_module_graph(&graph);
    let codes = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();
    assert_eq!(
        codes.first(),
        Some(&INVALID_OPERATOR_LANGUAGE_ITEM),
        "{codes:?}"
    );
    assert!(
        codes
            .iter()
            .skip(1)
            .all(|code| *code == UNSUPPORTED_GENERIC_OPERATOR_APPLICATION),
        "{codes:?}"
    );
    assert!(output.program.operator_language_item.is_none());
}

#[test]
fn valid_replacement_bundle_and_explicit_protocol_use_follow_ordinary_interfaces() {
    let app = concat!(
        "from std::ops import OpAdd;\n",
        "class Number implements OpAdd<Number, Number> {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn op_add(ref rhs: Number) -> Number { return Number(self.value + rhs.value); }\n",
        "}\n",
        "class Adder<T> where T: OpAdd<T, T> {\n",
        "  init() {}\n",
        "  fn add(ref left: T, ref right: T) -> T { return left.op_add(right); }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var adder: Adder<Number> = Adder<Number>();\n",
        "  var answer: Number = adder.add(Number(17), Number(25));\n",
        "  return answer.value;\n",
        "}\n",
    );
    let replacement = CANONICAL_OPS_SOURCE.replace("\n\n", "\n\n\n");
    let (_workspace, graph) = load_module_sources_with_standard_library_overrides(
        "app",
        &[("app.ska", app)],
        &[("std/ops.ska", &replacement)],
    );
    let output = resolve_module_graph(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(output.program.operator_language_item.is_some());
    let checked = type_check(&output.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    crate::mir::verify_mir(&crate::mir::lower_hir(
        &checked
            .hir
            .expect("valid manual protocol use must type-check"),
    ))
    .expect("manual canonical protocol calls must lower through ordinary interface machinery");
}

#[test]
fn qualified_protocol_use_authorizes_value_producing_class_punctuation() {
    let app = concat!(
        "import std::ops;\n",
        "class Number implements std::ops::OpAdd<Number, Number> {\n",
        "  value: i64; init(value: i64) { self.value = value; }\n",
        "  fn op_add(ref rhs: Number) -> Number { return Number(self.value + rhs.value); }\n",
        "}\n",
        "fn main() -> i64 { var left: Number = Number(1); var right: Number = Number(2); var invalid: Number = left + right; return 0; }\n",
    );
    let (_workspace, graph) = load_module_sources(
        "app",
        &[("app.ska", app), ("std/ops.ska", CANONICAL_OPS_SOURCE)],
    );
    let output = resolve_module_graph(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(output.program.operator_language_item.is_some());
    let checked = type_check(&output.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked
        .hir
        .expect("qualified canonical conformance must authorize punctuation");
    let dump = crate::hir::dump_hir(&hir);
    assert!(dump.contains("ObjectCall interface"), "{dump}");
    assert!(dump.contains("ObjectResult"), "{dump}");
    crate::mir::verify_mir(&crate::mir::lower_hir(&hir))
        .expect("overloaded punctuation must lower through ordinary interface machinery");
}
