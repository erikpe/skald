use crate::{
    identity::{CallableId, FunctionId, FunctionTypeId},
    mir::MirExecutionNode,
    source::{SourceDatabase, Span},
};

use super::*;

fn node(index: usize) -> MirExecutionNode {
    MirExecutionNode::callable(FunctionId::new(index).into())
}

fn target(index: usize) -> CallableId {
    FunctionId::new(index).into()
}

fn spans() -> [Span; 3] {
    let mut sources = SourceDatabase::new();
    let source = sources.add("function_values.ska", "0123456789");
    [
        Span::empty(source, 1),
        Span::empty(source, 4),
        Span::empty(source, 7),
    ]
}

fn formation(
    source: MirExecutionNode,
    function_type: FunctionTypeId,
    target: CallableId,
    span: Span,
) -> MirCallableAddressFormation {
    MirCallableAddressFormation::new(source, function_type, target, span)
}

fn site(
    source: MirExecutionNode,
    function_type: FunctionTypeId,
    span: Span,
) -> MirIndirectCallSite {
    MirIndirectCallSite::new(source, function_type, MirDependencyRegion::Ordinary, span)
}

fn edge_target(edge: &MirDependencyEdge) -> CallableId {
    let MirDependencyTarget::Execution(MirExecutionNode::Callable(target)) = edge.target() else {
        panic!("function-value coupling must return callable execution edges");
    };
    target
}

#[test]
fn couples_a_site_reached_before_its_candidate_once() {
    let function_type = FunctionTypeId::new(0);
    let [formation_span, site_span, _] = spans();
    let formation_source = node(0);
    let site_source = node(1);
    let mut coupling = MirFunctionValueCoupling::from_parts(
        [formation(
            formation_source,
            function_type,
            target(3),
            formation_span,
        )],
        [site(site_source, function_type, site_span)],
    );

    assert!(coupling.reach(site_source).is_empty());
    let edges = coupling.reach(formation_source);
    assert_eq!(edges.len(), 1);
    assert_eq!(edge_target(&edges[0]), target(3));
    assert!(coupling.reach(formation_source).is_empty());
    assert!(coupling.reach(site_source).is_empty());
}

#[test]
fn couples_a_candidate_reached_before_its_site() {
    let function_type = FunctionTypeId::new(0);
    let [formation_span, site_span, _] = spans();
    let formation_source = node(0);
    let site_source = node(1);
    let mut coupling = MirFunctionValueCoupling::from_parts(
        [formation(
            formation_source,
            function_type,
            target(3),
            formation_span,
        )],
        [site(site_source, function_type, site_span)],
    );

    assert!(coupling.reach(formation_source).is_empty());
    let edges = coupling.reach(site_source);
    assert_eq!(edges.len(), 1);
    assert_eq!(edge_target(&edges[0]), target(3));
}

#[test]
fn isolates_candidates_by_exact_function_type() {
    let [formation_span, site_span, _] = spans();
    let formation_source = node(0);
    let site_source = node(1);
    let mut coupling = MirFunctionValueCoupling::from_parts(
        [formation(
            formation_source,
            FunctionTypeId::new(0),
            target(3),
            formation_span,
        )],
        [site(site_source, FunctionTypeId::new(1), site_span)],
    );

    assert!(coupling.reach(formation_source).is_empty());
    assert!(coupling.reach(site_source).is_empty());
}

#[test]
fn keeps_canonical_formation_evidence_without_emitting_a_second_edge() {
    let function_type = FunctionTypeId::new(0);
    let [best_span, site_span, later_span] = spans();
    let best_source = node(0);
    let later_source = node(2);
    let site_source = node(1);
    let callable = target(3);
    let mut coupling = MirFunctionValueCoupling::from_parts(
        [
            formation(later_source, function_type, callable, later_span),
            formation(best_source, function_type, callable, best_span),
        ],
        [site(site_source, function_type, site_span)],
    );

    assert!(coupling.reach(site_source).is_empty());
    assert_eq!(coupling.reach(later_source).len(), 1);
    assert!(coupling.reach(best_source).is_empty());
    let candidates = coupling.into_candidates().collect::<Vec<_>>();
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].1,
        vec![formation(best_source, function_type, callable, best_span)]
    );
}

#[test]
fn returns_new_edges_in_canonical_target_order() {
    let function_type = FunctionTypeId::new(0);
    let [first_span, site_span, second_span] = spans();
    let formation_source = node(0);
    let site_source = node(1);
    let mut coupling = MirFunctionValueCoupling::from_parts(
        [
            formation(formation_source, function_type, target(4), second_span),
            formation(formation_source, function_type, target(2), first_span),
        ],
        [site(site_source, function_type, site_span)],
    );

    assert!(coupling.reach(formation_source).is_empty());
    let edges = coupling.reach(site_source);
    assert_eq!(
        edges.iter().map(edge_target).collect::<Vec<_>>(),
        vec![target(2), target(4)]
    );
}
