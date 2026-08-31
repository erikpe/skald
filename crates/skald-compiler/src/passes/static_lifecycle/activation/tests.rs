//! Activation vocabulary, fixture, and current-behavior baselines.

use crate::{
    mir::{lower_preliminary_hir, StaticAccessKind, StaticEffectPhase},
    passes::static_lifecycle::{
        plan_static_lifetimes, STATIC_LIFECYCLE_DEPENDENCY_CYCLE, STATIC_LIFECYCLE_SELF_DEPENDENCY,
    },
    resolve::resolve_module_graph,
    test_support::{load_module_sources_with_standard_library, type_check_source},
    typeck::type_check,
};

use super::test_support::{
    activation_analysis_fixture, activation_identity_fixture, CYCLE_SOURCE, DIRECT_ACCESS_SOURCE,
    DYNAMIC_AND_INDIRECT_SOURCE, IMPLICIT_LIFECYCLE_SOURCE, INACTIVE_ONLY_DEPENDENCY_SOURCE,
    SELF_DEPENDENCY_SOURCE, STORED_FAMILY_SOURCE,
};
use super::{
    static_activation_edge_key, static_activation_node_key, StaticActivationNode,
    StaticActivationTrigger,
};

#[test]
fn node_and_edge_keys_define_stable_semantic_order() {
    let fixture = activation_identity_fixture();
    let analysis = activation_analysis_fixture(true);

    assert!(
        static_activation_node_key(StaticActivationNode::execution(fixture.entry))
            < static_activation_node_key(StaticActivationNode::field(fixture.active))
    );
    assert!(analysis
        .edges()
        .windows(2)
        .all(|pair| static_activation_edge_key(&pair[0]) < static_activation_edge_key(&pair[1])));
    assert_eq!(
        analysis
            .reachable_execution()
            .iter()
            .map(|execution| execution.node())
            .collect::<Vec<_>>(),
        vec![
            fixture.entry,
            fixture.helper,
            fixture.initializer,
            fixture.destruction,
        ]
    );
}

#[test]
fn immutable_queries_keep_fields_execution_witnesses_and_counts_coherent() {
    let fixture = activation_identity_fixture();
    let first = activation_analysis_fixture(false);
    let reordered = activation_analysis_fixture(true);

    assert_eq!(first, reordered);
    assert!(first.is_active(fixture.active));
    assert!(!first.is_active(fixture.inactive));
    assert_eq!(first.inactive_fields(), &[fixture.inactive]);
    assert!(first.is_execution_reachable(fixture.entry));
    assert!(first.is_execution_reachable(fixture.initializer));
    assert!(first.execution(fixture.destruction).is_some());
    assert_eq!(
        first
            .field(fixture.active)
            .unwrap()
            .first_trigger()
            .trigger(),
        StaticActivationTrigger::StaticAccess {
            access: StaticAccessKind::Read,
            phase: StaticEffectPhase::Ordinary,
        }
    );
    assert_eq!(
        first
            .field(fixture.active)
            .unwrap()
            .witness()
            .root()
            .entry(),
        fixture.entry
    );
    assert_eq!(first.active_fields().len(), 1);
    assert_eq!(first.counts().declared_fields, 2);
    assert_eq!(first.counts().active_fields, 1);
    assert_eq!(first.counts().inactive_fields, 1);
    assert_eq!(first.counts().reachable_execution_nodes, 4);
    assert_eq!(first.counts().edges, 4);
    assert_eq!(first.counts().execution_dependencies, 1);
    assert_eq!(first.counts().static_accesses, 1);
    assert_eq!(first.counts().initializer_roots, 1);
    assert_eq!(first.counts().destruction_roots, 1);
}

#[test]
fn focused_source_fixtures_cover_future_activation_inputs() {
    for source in [
        DIRECT_ACCESS_SOURCE,
        DYNAMIC_AND_INDIRECT_SOURCE,
        IMPLICIT_LIFECYCLE_SOURCE,
        INACTIVE_ONLY_DEPENDENCY_SOURCE,
    ] {
        let checked = type_check_source(source);
        assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
        let preliminary = lower_preliminary_hir(&checked.hir.unwrap());
        assert!(preliminary.executable_definitions().next().is_some());
    }

    let (_workspace, graph) =
        load_module_sources_with_standard_library("app", &[("app.ska", STORED_FAMILY_SOURCE)]);
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let preliminary = lower_preliminary_hir(&checked.hir.unwrap());
    assert!(preliminary.static_fields().len() >= 12);
}

#[test]
fn current_eager_planner_still_rejects_unused_self_dependencies_and_cycles() {
    for (source, code) in [
        (SELF_DEPENDENCY_SOURCE, STATIC_LIFECYCLE_SELF_DEPENDENCY),
        (CYCLE_SOURCE, STATIC_LIFECYCLE_DEPENDENCY_CYCLE),
    ] {
        let checked = type_check_source(source);
        assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
        let preliminary = lower_preliminary_hir(&checked.hir.unwrap());
        let failure = plan_static_lifetimes(preliminary).unwrap_err();
        assert!(failure
            .diagnostics()
            .any(|diagnostic| diagnostic.code == code));
    }
}
