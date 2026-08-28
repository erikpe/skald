use crate::{
    resolve::{dump_resolved, resolve_module_graph, UNSATISFIED_GENERIC_REQUIREMENT},
    test_support::{load_module_sources, CANONICAL_RANGE_SOURCE},
};

fn resolve_successor_source(source: &str) -> crate::resolve::ResolveOutput {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            ("app.ska", source),
            ("std/range.ska", CANONICAL_RANGE_SOURCE),
        ],
    );
    resolve_module_graph(&graph)
}

#[test]
fn integer_successor_bounds_close_to_existing_addition() {
    let resolved = resolve_successor_source(
        "from std::range import Successor;\n\
         class Advance<T> where T: Successor<T> {\n\
           init() {}\n\
           fn next(value: T) -> T { return value.successor(); }\n\
         }\n\
         fn use(ref byte: Advance<u8>, ref wide: Advance<u64>, ref signed: Advance<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let resolved_dump = dump_resolved(&resolved.program);
    for operation in ["AddOneU8", "AddOneU64", "AddOneI64"] {
        assert!(
            resolved_dump.contains(operation),
            "missing {operation}:\n{resolved_dump}"
        );
    }

    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = crate::hir::dump_hir(&checked.hir.expect("integer successors must produce HIR"));
    assert_eq!(hir.matches("AddU8").count(), 1, "{hir}");
    assert_eq!(hir.matches("AddU64").count(), 1, "{hir}");
    assert_eq!(hir.matches("AddI64").count(), 1, "{hir}");
    assert!(!hir.contains("InterfaceCall"), "{hir}");
}

#[test]
fn class_successor_bounds_retain_ordinary_witness_dispatch() {
    let resolved = resolve_successor_source(
        "from std::range import Successor;\n\
         class Value implements Successor<Value> {\n\
           value: u64;\n\
           init(value: u64) { self.value = value; }\n\
           fn successor() -> Value { return Value(self.value + 1u); }\n\
         }\n\
         class Advance<T> where T: Successor<T> {\n\
           init() {}\n\
           fn next(value: T) -> T { return value.successor(); }\n\
         }\n\
         fn use(ref value: Advance<Value>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let resolved_dump = dump_resolved(&resolved.program);
    assert!(
        resolved_dump.contains("ClosedBoundSelection 0 class-witness"),
        "{resolved_dump}"
    );

    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = crate::hir::dump_hir(&checked.hir.expect("class successors must produce HIR"));
    assert!(hir.contains("ObjectCall interface"), "{hir}");
}

#[test]
fn unsupported_wrong_and_noncanonical_successor_bounds_are_rejected() {
    let resolved = resolve_successor_source(
        "import std::range;\n\
         interface Successor<Output> { fn successor() -> Output; }\n\
         class UnsupportedFloat<T> where T: std::range::Successor<T> { init() {} }\n\
         class UnsupportedBool<T> where T: std::range::Successor<T> { init() {} }\n\
         class WrongOutput<T> where T: std::range::Successor<u64> { init() {} }\n\
         class Foreign<T> where T: Successor<T> { init() {} }\n\
         class Value implements std::range::Successor<Value> {\n\
           init() {}\n\
           fn successor() -> Value { return Value(); }\n\
         }\n\
         class ExactOnly<T> where T: std::range::Successor<T> { init() {} }\n\
         fn reject_float(ref value: UnsupportedFloat<f64>) -> unit {}\n\
         fn reject_bool(ref value: UnsupportedBool<bool>) -> unit {}\n\
         fn reject_output(ref value: WrongOutput<u8>) -> unit {}\n\
         fn reject_foreign(ref value: Foreign<u64>) -> unit {}\n\
         fn reject_view(ref value: ExactOnly<std::range::Successor<Value>>) -> unit {}\n\
         fn reject_owner(ref value: ExactOnly<shared Value>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    let failures = resolved
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == UNSATISFIED_GENERIC_REQUIREMENT)
        .collect::<Vec<_>>();
    assert_eq!(failures.len(), 6, "{:?}", resolved.diagnostics);
    assert!(
        failures.iter().all(|failure| failure
            .notes
            .iter()
            .any(|note| { note.contains("supported exact canonical successor application") })),
        "{:?}",
        resolved.diagnostics
    );
}

#[test]
fn primitive_successor_evidence_does_not_create_direct_members_or_views() {
    let direct = resolve_successor_source(
        "from std::range import Successor;\n\
         fn invalid(value: u64) -> u64 { return value.successor(); }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(
        direct
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == crate::resolve::INVALID_MEMBER_SELECTION }),
        "{:?}",
        direct.diagnostics
    );

    let view = resolve_successor_source(
        "from std::range import Successor;\n\
         fn invalid(ref value: Successor<u64>) -> unit {}\n\
         fn main() -> i64 { var primitive: u64 = 1u; invalid(primitive); return 0; }\n",
    );
    assert!(view.diagnostics.is_empty(), "{:?}", view.diagnostics);
    let checked = crate::typeck::type_check(&view.program);
    assert!(
        checked.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == crate::typeck::TYPE_MISMATCH
                || diagnostic.code == crate::typeck::INVALID_ALIAS_ARGUMENT
        }),
        "{:?}",
        checked.diagnostics
    );
}
