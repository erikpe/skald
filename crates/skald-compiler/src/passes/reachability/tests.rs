//! Focused tests for the execution-dependency contract.

use crate::{
    identity::{ClassId, ExternalLinkId, FunctionId, StaticFieldId},
    intrinsic::Intrinsic,
    mir::{
        MirArrayLifecycleOperation, MirClassLifecycleOperation, MirExecutionNode,
        StaticArrayLifecycleOperation, StaticClassLifecycleOperation, StaticEffectNode,
        StaticLifecycleRootAuthority,
    },
};

use super::{
    test_support::{fixture_spans, reachability_identity_fixture},
    *,
};

fn all_edge_kinds() -> [MirDependencyEdgeKind; 25] {
    [
        MirDependencyEdgeKind::DirectCall,
        MirDependencyEdgeKind::StaticCall,
        MirDependencyEdgeKind::DirectMethodCall,
        MirDependencyEdgeKind::VirtualDispatch,
        MirDependencyEdgeKind::InterfaceDispatch,
        MirDependencyEdgeKind::CallableAddressRetention,
        MirDependencyEdgeKind::IndirectCall,
        MirDependencyEdgeKind::Initializer,
        MirDependencyEdgeKind::CopyConstructor,
        MirDependencyEdgeKind::CopyAssignment,
        MirDependencyEdgeKind::UserCopyBody,
        MirDependencyEdgeKind::BaseCopy,
        MirDependencyEdgeKind::FieldCopy,
        MirDependencyEdgeKind::CompleteFinalizer,
        MirDependencyEdgeKind::UserDestructor,
        MirDependencyEdgeKind::FieldFinalizer,
        MirDependencyEdgeKind::BaseFinalizer,
        MirDependencyEdgeKind::SharedFinalizer,
        MirDependencyEdgeKind::TemporaryCleanup,
        MirDependencyEdgeKind::OptionalLifecycle,
        MirDependencyEdgeKind::ArrayDefault,
        MirDependencyEdgeKind::ArrayCopy,
        MirDependencyEdgeKind::ArrayAssignment,
        MirDependencyEdgeKind::ArrayDestruction,
        MirDependencyEdgeKind::RuntimeEntityReference,
    ]
}

#[test]
fn execution_node_taxonomy_has_canonical_semantic_order() {
    let fixture = reachability_identity_fixture();
    let mut nodes = fixture.callable_nodes();
    nodes.extend(fixture.lifecycle_nodes());
    nodes.sort_by_key(|node| mir_execution_node_key(*node));

    assert_eq!(nodes.len(), 17);
    assert!(nodes
        .windows(2)
        .all(|pair| mir_execution_node_key(pair[0]) < mir_execution_node_key(pair[1])));
    assert!(matches!(nodes[0], MirExecutionNode::Callable(_)));
    assert!(matches!(
        nodes[nodes.len() - 1],
        MirExecutionNode::ArrayLifecycle {
            operation: MirArrayLifecycleOperation::Destruction,
            ..
        }
    ));
}

#[test]
fn static_lifecycle_identity_names_are_compatible_aliases() {
    let class = ClassId::new(3);
    let neutral = MirExecutionNode::class(class, MirClassLifecycleOperation::CompleteFinalizer);
    let certificate_name =
        StaticEffectNode::class(class, StaticClassLifecycleOperation::CompleteFinalizer);
    let authority = StaticLifecycleRootAuthority::new(certificate_name, Vec::new());

    assert_eq!(neutral, certificate_name);
    assert_eq!(authority.root(), neutral);
    assert_eq!(
        MirExecutionNode::array(
            reachability_identity_fixture().array,
            MirArrayLifecycleOperation::Copy,
        ),
        StaticEffectNode::array(
            reachability_identity_fixture().array,
            StaticArrayLifecycleOperation::Copy,
        )
    );
}

#[test]
fn dependency_edge_kind_taxonomy_has_explicit_canonical_order() {
    let kinds = all_edge_kinds();
    assert_eq!(kinds.len(), 25);
    assert_eq!(
        kinds.map(mir_dependency_edge_kind_key),
        core::array::from_fn(|index| index as u8)
    );
}

#[test]
fn root_reasons_and_spans_have_stable_canonical_order() {
    let first = StaticFieldId::new(ClassId::new(0), 2);
    let second = StaticFieldId::new(ClassId::new(1), 0);
    let mut reasons = [
        MirReachabilityRootReason::StaticShutdown(first),
        MirReachabilityRootReason::StaticActivation(second),
        MirReachabilityRootReason::Entry,
        MirReachabilityRootReason::StaticActivation(first),
    ];
    reasons.sort_by_key(|reason| mir_reachability_root_reason_key(*reason));

    assert_eq!(reasons[0], MirReachabilityRootReason::Entry);
    assert_eq!(
        reasons[1],
        MirReachabilityRootReason::StaticActivation(first)
    );
    assert_eq!(
        reasons[2],
        MirReachabilityRootReason::StaticActivation(second)
    );
    assert_eq!(reasons[3], MirReachabilityRootReason::StaticShutdown(first));

    let mut spans = fixture_spans();
    spans.reverse();
    spans.sort_by_key(|span| mir_span_key(*span));
    assert_eq!(
        spans.map(mir_span_key),
        [(0, 1, 1), (0, 7, 7), (1, 0, 0), (1, 3, 3)]
    );
}

#[test]
fn dependency_types_keep_execution_metadata_and_leaf_targets_distinct() {
    let fixture = reachability_identity_fixture();
    let source = MirExecutionNode::Callable(fixture.ordinary_function);
    let execution = MirDependencyEdge::new(
        source,
        MirDependencyTarget::Execution(MirExecutionNode::Callable(fixture.direct_method)),
        MirDependencyEdgeKind::DirectMethodCall,
        fixture.span,
    );
    let runtime = MirDependencyEdge::new(
        source,
        MirDependencyTarget::RuntimeEntity(MirRuntimeEntity::VirtualFamily(fixture.virtual_family)),
        MirDependencyEdgeKind::RuntimeEntityReference,
        fixture.span,
    );

    assert_eq!(execution.source(), source);
    assert_eq!(execution.kind(), MirDependencyEdgeKind::DirectMethodCall);
    assert_eq!(execution.span(), fixture.span);
    assert!(matches!(
        execution.target(),
        MirDependencyTarget::Execution(_)
    ));
    assert!(matches!(
        runtime.target(),
        MirDependencyTarget::RuntimeEntity(_)
    ));
    assert!(matches!(
        MirDependencyTarget::External(ExternalLinkId::new(0)),
        MirDependencyTarget::External(_)
    ));
    assert!(matches!(
        MirDependencyTarget::Intrinsic(Intrinsic::Panic),
        MirDependencyTarget::Intrinsic(_)
    ));
}

#[test]
fn roots_declarations_and_retained_definitions_are_separate_roles() {
    let fixture = reachability_identity_fixture();
    let node = MirExecutionNode::Callable(fixture.ordinary_function);
    let definition = MirRetainedDefinition::new(fixture.ordinary_function);
    let declaration = MirSemanticDeclaration::Callable(fixture.ordinary_function);
    let root = MirReachabilityRoot::new(
        MirReachabilityRootTarget::Execution(node),
        MirReachabilityRootReason::Entry,
        fixture.span,
    );
    let zero_default_root = MirReachabilityRoot::new(
        MirReachabilityRootTarget::RuntimeEntity(MirRuntimeEntity::StaticStorage(
            fixture.static_field,
        )),
        MirReachabilityRootReason::StaticActivation(fixture.static_field),
        fixture.span,
    );

    assert_eq!(definition.callable(), fixture.ordinary_function);
    assert_eq!(definition.execution_node(), node);
    assert_eq!(
        declaration,
        MirSemanticDeclaration::Callable(definition.callable())
    );
    assert_eq!(root.target(), MirReachabilityRootTarget::Execution(node));
    assert_eq!(root.reason(), MirReachabilityRootReason::Entry);
    assert_eq!(root.span(), fixture.span);
    assert!(matches!(
        zero_default_root.target(),
        MirReachabilityRootTarget::RuntimeEntity(MirRuntimeEntity::StaticStorage(_))
    ));
}

#[test]
fn identity_fixture_is_deterministic_and_covers_future_target_inputs() {
    let first = reachability_identity_fixture();
    let second = reachability_identity_fixture();

    assert_eq!(first, second);
    assert_eq!(first.callable_nodes(), second.callable_nodes());
    assert_eq!(first.lifecycle_nodes(), second.lifecycle_nodes());
    assert_eq!(first.runtime_entities(), second.runtime_entities());
    assert_eq!(first.virtual_method.class(), Some(first.class));
    assert_eq!(first.interface_method.class(), Some(first.class));
    assert_eq!(first.function_value_target, FunctionId::new(1).into());
}
