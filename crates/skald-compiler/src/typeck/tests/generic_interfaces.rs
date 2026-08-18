use super::*;
use crate::{
    hir::dump_hir,
    identity::{ClassId, InterfaceId, InterfaceRequirementId, MethodId},
    resolve::dump_resolved,
    test_support::load_module_sources,
};

#[test]
fn resolved_dump_records_closed_specialized_claims_and_origins() {
    let resolved = crate::test_support::resolve_source(
        "interface Marker<T> {}\n\
         class Holder<T> implements Marker<T> { init() {} }\n\
         fn use(ref value: Holder<u64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let dump = dump_resolved(&resolved.program);
    for expected in [
        "ClosedInterfaceClaim 0 -> i0",
        "SpecializationOrigin module m0",
        "Implements i0",
    ] {
        assert!(dump.contains(expected), "missing `{expected}` in:\n{dump}");
    }
    assert_eq!(dump, dump_resolved(&resolved.program));
}

#[test]
fn ordinary_class_conformance_uses_substituted_exact_signatures() {
    let hir = check_generic_source(
        "class Item { init() {} }\n\
         interface Transfer<T> { fn transfer(value: T) -> T; }\n\
         class Scalar implements Transfer<u64> {\n\
           init() {}\n\
           fn transfer(value: u64) -> u64 { return value; }\n\
         }\n\
         class Exact implements Transfer<Item> {\n\
           init() {}\n\
           fn transfer(value: Item) -> Item { return value; }\n\
         }\n\
         class Owner implements Transfer<shared Item> {\n\
           init() {}\n\
           fn transfer(value: shared Item) -> shared Item { return value; }\n\
         }\n\
         class Maybe implements Transfer<u64?> {\n\
           init() {}\n\
           fn transfer(value: u64?) -> u64? { return value; }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    for class in 1..=4 {
        let class = hir.class(ClassId::new(class)).unwrap();
        assert_eq!(class.conformances.len(), 1);
        assert_eq!(class.conformances[0].implementations.len(), 1);
    }
    let interfaces = (0..4)
        .map(|class| hir.class(ClassId::new(class + 1)).unwrap().conformances[0].interface)
        .collect::<Vec<_>>();
    assert_eq!(
        interfaces,
        [
            InterfaceId::new(0),
            InterfaceId::new(1),
            InterfaceId::new(2),
            InterfaceId::new(3),
        ]
    );
}

#[test]
fn generic_class_claims_close_per_specialization() {
    let hir = check_generic_source(
        "interface Transfer<T> { fn transfer(value: T) -> T; }\n\
         class Adapter<T> implements Transfer<T> {\n\
           init() {}\n\
           fn transfer(value: T) -> T { return value; }\n\
         }\n\
         fn use_i64(ref value: Adapter<i64>) -> unit {}\n\
         fn use_u64(ref value: Adapter<u64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    for index in 0..2 {
        let class = hir.class(ClassId::new(index)).unwrap();
        assert_eq!(class.conformances.len(), 1);
        assert_eq!(class.conformances[0].interface, InterfaceId::new(index));
        assert_eq!(
            class.conformances[0].implementations[0].requirement,
            InterfaceRequirementId::new(InterfaceId::new(index), 0)
        );
        assert_eq!(
            class.conformances[0].implementations[0].method,
            MethodId::new(ClassId::new(index), 0)
        );
    }

    let dump = dump_hir(&hir);
    assert!(dump.contains("Interface i0"), "{dump}");
    assert!(dump.contains("i0:requirement0 -> c0:method0"), "{dump}");
    assert!(dump.contains("Interface i1"), "{dump}");
    assert!(dump.contains("i1:requirement0 -> c1:method0"), "{dump}");
}

#[test]
fn inherited_conformance_uses_the_effective_override() {
    let hir = check_generic_source(
        "interface Source<T> { fn get() -> T; }\n\
         class Base implements Source<u64> {\n\
           init() {}\n\
           virtual fn get() -> u64 { return 1u; }\n\
         }\n\
         class Derived extends Base {\n\
           init() { super(); }\n\
           override fn get() -> u64 { return 2u; }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    let derived = hir.class(ClassId::new(1)).unwrap();
    assert_eq!(derived.conformances.len(), 1);
    assert_eq!(derived.conformances[0].interface, InterfaceId::new(0));
    assert_eq!(
        derived.conformances[0].implementations[0].method,
        MethodId::new(ClassId::new(1), 0)
    );
}

#[test]
fn generated_generic_base_passes_closed_conformance_to_its_derived_class() {
    let hir = check_generic_source(
        "interface Marker<T> {}\n\
         class Base<T> implements Marker<T> { init() {} }\n\
         class Derived extends Base<u64> { init() { super(); } }\n\
         fn use(ref value: Derived) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let derived = hir.class(ClassId::new(0)).unwrap();
    let base = hir.class(ClassId::new(1)).unwrap();
    assert_eq!(derived.conformances.len(), 1);
    assert_eq!(base.conformances.len(), 1);
    assert_eq!(derived.conformances[0].interface, InterfaceId::new(0));
    assert_eq!(base.conformances[0].interface, InterfaceId::new(0));
}

#[test]
fn distinct_applications_can_share_one_method_or_marker_class() {
    let hir = check_generic_source(
        "interface Marker<T> {}\n\
         interface Named<T> { fn name() -> u64; }\n\
         class BothMarkers implements Marker<i64>, Marker<u64> { init() {} }\n\
         class BothNames implements Named<i64>, Named<u64> {\n\
           init() {}\n\
           fn name() -> u64 { return 7u; }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    let markers = hir.class(ClassId::new(0)).unwrap();
    assert_eq!(markers.conformances.len(), 2);
    assert!(markers
        .conformances
        .iter()
        .all(|conformance| conformance.implementations.is_empty()));

    let names = hir.class(ClassId::new(1)).unwrap();
    assert_eq!(names.conformances.len(), 2);
    assert!(names.conformances.iter().all(|conformance| {
        conformance.implementations[0].method == MethodId::new(ClassId::new(1), 0)
    }));
    assert_ne!(
        names.conformances[0].interface,
        names.conformances[1].interface
    );
}

#[test]
fn exact_duplicate_and_redundant_claim_rules_do_not_merge_distinct_applications() {
    let duplicate = crate::test_support::resolve_source(
        "interface Marker<T> {}\n\
         class Duplicate implements Marker<u64>, Marker<u64> { init() {} }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert_eq!(
        duplicate
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message.contains("duplicate interface"))
            .count(),
        1
    );

    let substituted_duplicate = check_text(
        "interface Marker<T> {}\n\
         class Pair<Left, Right> implements Marker<Left>, Marker<Right> { init() {} }\n\
         fn use(ref value: Pair<u64, u64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(substituted_duplicate.hir.is_none());
    assert!(substituted_duplicate
        .diagnostics
        .iter()
        .any(
            |diagnostic| diagnostic.message.contains("implements interface")
                && diagnostic.message.contains("more than once")
        ));

    let inherited = check_generic_source(
        "interface Marker<T> {}\n\
         class Base implements Marker<i64> { init() {} }\n\
         class Derived extends Base implements Marker<u64> { init() { super(); } }\n\
         fn main() -> i64 { return 0; }\n",
    );
    let derived = inherited.class(ClassId::new(1)).unwrap();
    assert_eq!(derived.conformances.len(), 2);
    assert_ne!(
        derived.conformances[0].interface,
        derived.conformances[1].interface
    );

    let redundant = check_text(
        "interface Marker<T> {}\n\
         class Base implements Marker<u64> { init() {} }\n\
         class Derived extends Base implements Marker<u64> { init() { super(); } }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(redundant.hir.is_none());
    assert!(redundant
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("redundantly implements")));
}

#[test]
fn incompatible_application_reports_claim_requirement_and_method_origins() {
    let output = check_text(
        "interface Value<T> { fn value() -> T; }\n\
         class Bad implements Value<i64>, Value<u64> {\n\
           init() {}\n\
           fn value() -> i64 { return 0; }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.hir.is_none());
    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("does not exactly implement"))
        .expect("one exact application must reject the incompatible result");
    assert!(diagnostic
        .labels
        .iter()
        .any(|label| label.message == "interface conformance declared here"));
    assert!(diagnostic
        .labels
        .iter()
        .any(|label| label.message == "requirement declared here"));
    assert!(diagnostic
        .labels
        .iter()
        .any(|label| label.message == "result type differs from the interface requirement"));
}

#[test]
fn generic_requirements_reuse_all_ordinary_exact_signature_checks() {
    let cases = [
        (
            "interface Required<T> { mut fn use(value: T) -> unit; }\n\
             class Bad implements Required<u64> {\n\
               init() {}\n\
               fn use(value: u64) -> unit {}\n\
             }\n\
             fn main() -> i64 { return 0; }\n",
            "receiver access differs",
        ),
        (
            "interface Required<T> { fn use(ref value: T) -> unit; }\n\
             class Bad implements Required<u64> {\n\
               init() {}\n\
               fn use(value: u64) -> unit {}\n\
             }\n\
             fn main() -> i64 { return 0; }\n",
            "binding mode differs",
        ),
        (
            "interface Required<T> { fn use(value: T) -> unit; }\n\
             class Bad implements Required<u64> {\n\
               init() {}\n\
               private fn use(value: u64) -> unit {}\n\
             }\n\
             fn main() -> i64 { return 0; }\n",
            "private method",
        ),
        (
            "interface Required<T> { fn use(value: T) -> unit; }\n\
             class Bad implements Required<u64> {\n\
               init() {}\n\
               static fn use(value: u64) -> unit {}\n\
             }\n\
             fn main() -> i64 { return 0; }\n",
            "static method",
        ),
    ];

    for (source, expected) in cases {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(expected)
                    || diagnostic
                        .labels
                        .iter()
                        .any(|label| label.message.contains(expected))
            }),
            "missing `{expected}` in {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn public_cross_module_application_conforms_exactly() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import api;\n\
                 class Reader implements api::Readable<u64> {\n\
                   init() {}\n\
                   fn read() -> u64 { return 1u; }\n\
                 }\n\
                 fn main() -> i64 { return 0; }\n",
            ),
            (
                "api.ska",
                "public interface Readable<T> { fn read() -> T; }\n",
            ),
        ],
    );
    let resolved = crate::resolve::resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let reader = checked.hir.unwrap().class(ClassId::new(0)).unwrap().clone();
    assert_eq!(reader.conformances.len(), 1);
    assert_eq!(reader.conformances[0].interface, InterfaceId::new(0));
}
