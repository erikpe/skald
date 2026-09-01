//! Focused tests for the execution-dependency contract.

use std::collections::BTreeSet;

use crate::{
    identity::{ClassId, ExternalLinkId, FunctionId, StaticFieldId},
    intrinsic::Intrinsic,
    mir::{
        MirArrayLifecycleOperation, MirCallTarget, MirClassLifecycleOperation, MirExecutionNode,
        MirFunctionLinkage, MirInstruction, MirPlaceBase, MirRvalueKind, StaticAccessKind,
        StaticLifecycleRootAuthority,
    },
    test_support::{lower_generic_source_to_final_mir, lower_generic_source_to_preliminary_mir},
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

#[test]
fn executable_definition_view_borrows_preliminary_and_final_definitions_in_native_order() {
    let source = "fn helper() -> i64 { return 1; }
                  class State { static value: i64 = helper(); init() {} }
                  fn main() -> i64 { return State.value; }";
    let preliminary = lower_generic_source_to_preliminary_mir(source);
    let expected = preliminary.executable_definitions().collect::<Vec<_>>();
    let actual = MirExecutableDefinitionView::preliminary(&preliminary)
        .iter()
        .collect::<Vec<_>>();

    assert_eq!(
        actual
            .iter()
            .map(|definition| definition.callable())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|definition| definition.callable())
            .collect::<Vec<_>>()
    );
    assert!(actual
        .iter()
        .zip(&expected)
        .all(|(actual, expected)| std::ptr::eq(actual.body(), expected.body())));

    let final_program = lower_generic_source_to_final_mir(source);
    let expected = final_program.executable_definitions().collect::<Vec<_>>();
    let actual = MirExecutableDefinitionView::final_program(&final_program)
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(
        actual
            .iter()
            .map(|definition| definition.callable())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|definition| definition.callable())
            .collect::<Vec<_>>()
    );
    assert!(actual
        .iter()
        .zip(&expected)
        .all(|(actual, expected)| std::ptr::eq(actual.body(), expected.body())));
}

#[test]
fn extraction_centralizes_dispatch_and_scoped_function_value_targets() {
    let preliminary = lower_generic_source_to_preliminary_mir(
        "interface View { fn read() -> i64; }
         class Base implements View {
           init() {}
           virtual fn read() -> i64 { return 1; }
           fn exact() -> i64 { return 4; }
           static fn answer() -> i64 { return 5; }
         }
         class Child extends Base {
           init() { super(); }
           override fn read() -> i64 { return 2; }
         }
         fn target() -> i64 { return 3; }
         fn invoke(callback: fn() -> i64) -> i64 { return callback(); }
         fn retain() -> unit { var callback: fn() -> i64 = target; }
         fn read_virtual(ref value: Base) -> i64 { return value.read(); }
         fn read_interface(ref value: View) -> i64 { return value.read(); }
         fn read_exact(ref value: Base) -> i64 { return value.exact(); }
         fn read_static() -> i64 { return Base.answer(); }
         fn main() -> i64 { return invoke(target); }",
    );
    let first = extract_preliminary_dependencies(&preliminary).unwrap();
    let second = extract_preliminary_dependencies(&preliminary).unwrap();
    assert_eq!(first, second);

    let kinds = first
        .dependencies()
        .iter()
        .map(|dependency| dependency.edge().kind())
        .collect::<BTreeSet<_>>();
    assert!(kinds.contains(&MirDependencyEdgeKind::DirectCall));
    assert!(kinds.contains(&MirDependencyEdgeKind::StaticCall));
    assert!(kinds.contains(&MirDependencyEdgeKind::DirectMethodCall));
    assert!(kinds.contains(&MirDependencyEdgeKind::VirtualDispatch));
    assert!(kinds.contains(&MirDependencyEdgeKind::InterfaceDispatch));
    assert!(kinds.contains(&MirDependencyEdgeKind::CallableAddressRetention));
    assert_eq!(first.callable_addresses().len(), 2);
    assert_eq!(first.indirect_calls().len(), 1);
    assert!(first
        .callable_addresses()
        .iter()
        .all(|formation| first.nodes().contains(&formation.source())));
    assert_eq!(
        first
            .all_indirect_targets(first.indirect_calls()[0].function_type())
            .collect::<Vec<_>>(),
        vec![first.callable_addresses()[0].target()]
    );
}

#[test]
fn extraction_inventories_direct_static_access_kinds_in_canonical_order() {
    let preliminary = lower_generic_source_to_preliminary_mir(
        "class Item {
           value: i64;
           init(value: i64) { self.value = value; }
           copy(ref other: Item) { self.value = other.value; }
           assign(ref other: Item) { self.value = other.value; }
         }
         class State {
           static scalar: i64 = 1;
           static item: Item = Item(2);
           init() {}
         }
         fn inspect(ref value: i64) -> i64 { return value; }
         fn modify(mut ref value: i64) -> unit { value = value + 1; }
         fn main() -> i64 {
           var observed: i64 = State.scalar;
           State.scalar = 3;
           observed = inspect(State.scalar);
           modify(State.scalar);
           var replacement: Item = Item(4);
           State.item = replacement;
           return observed;
         }",
    );
    let first = extract_preliminary_dependencies(&preliminary).unwrap();
    let second = extract_preliminary_dependencies(&preliminary).unwrap();
    assert_eq!(first, second);
    assert!(first
        .static_accesses()
        .windows(2)
        .all(|pair| mir_static_access_key(&pair[0]) < mir_static_access_key(&pair[1])));

    let entry = MirExecutionNode::callable(preliminary.program().entry_function.into());
    let entry_accesses = first.static_accesses_from(entry);
    let kinds = entry_accesses
        .iter()
        .map(|access| access.kind())
        .collect::<BTreeSet<_>>();
    assert!(kinds.contains(&StaticAccessKind::Read));
    assert!(kinds.contains(&StaticAccessKind::Write));
    assert!(kinds.contains(&StaticAccessKind::Borrow));
    assert!(kinds.contains(&StaticAccessKind::Replace));
    assert!(
        entry_accesses
            .iter()
            .filter(|access| access.kind() == StaticAccessKind::Borrow)
            .count()
            >= 2,
        "immutable and mutable static borrows must both be inventoried"
    );
    assert!(entry_accesses
        .iter()
        .all(|access| access.origin() == MirStaticAccessOrigin::Ordinary));
}

#[test]
fn extraction_distinguishes_initializer_destination_from_ordinary_self_access() {
    let preliminary = lower_generic_source_to_preliminary_mir(
        "class State {
           static value: i64 = State.value;
           init() {}
         }
         fn main() -> i64 {
           if (false) { return State.value; }
           return 0;
         }",
    );
    let field = preliminary.static_fields().next().unwrap().field;
    let initializer = preliminary.static_initializers().next().unwrap().callable();
    let extracted = extract_preliminary_dependencies(&preliminary).unwrap();
    let initializer_accesses =
        extracted.static_accesses_from(MirExecutionNode::callable(initializer));

    assert!(initializer_accesses.iter().any(|access| {
        access.target() == field
            && access.kind() == StaticAccessKind::Initialize
            && access.region() == MirDependencyRegion::StaticInitializerBeforePublication
            && access.origin() == MirStaticAccessOrigin::LifecycleOwnedDestination
    }));
    assert!(initializer_accesses.iter().any(|access| {
        access.target() == field
            && access.kind() == StaticAccessKind::Read
            && access.region() == MirDependencyRegion::StaticInitializerBeforePublication
            && access.origin() == MirStaticAccessOrigin::Ordinary
    }));

    let entry = MirExecutionNode::callable(preliminary.program().entry_function.into());
    assert!(extracted.static_accesses().iter().any(|access| {
        access.source() == entry
            && access.target() == field
            && access.kind() == StaticAccessKind::Read
    }));
}

#[test]
fn static_access_extraction_reports_malformed_field_and_destination_identities() {
    let preliminary = lower_generic_source_to_preliminary_mir(
        "class State { static value: i64 = 1; init() {} }
         fn main() -> i64 { return State.value; }",
    );
    let field = preliminary.static_fields().next().unwrap().field;
    let mut unknown_field = preliminary.clone();
    let unknown_entry = unknown_field.program().entry_function;
    let place = unknown_field
        .program_mut()
        .definitions
        .get_mut_for_test(unknown_entry)
        .unwrap()
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assign) => match &mut assign.rvalue.kind {
                MirRvalueKind::Load(place) => Some(place),
                _ => None,
            },
            _ => None,
        })
        .unwrap();
    let missing = StaticFieldId::new(ClassId::new(999), 0);
    place.base = MirPlaceBase::StaticField(missing);
    assert_eq!(
        extract_preliminary_dependencies(&unknown_field),
        Err(MirDependencyExtractionError::UnknownStaticField(missing))
    );

    let mut foreign_destination = preliminary;
    let entry = foreign_destination.program().entry_function;
    let place = foreign_destination
        .program_mut()
        .definitions
        .get_mut_for_test(entry)
        .unwrap()
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assign) => match &mut assign.rvalue.kind {
                MirRvalueKind::Load(place) => Some(place),
                _ => None,
            },
            _ => None,
        })
        .unwrap();
    place.base = MirPlaceBase::StaticLifecycleDestination(field);
    assert_eq!(
        extract_preliminary_dependencies(&foreign_destination),
        Err(
            MirDependencyExtractionError::InvalidStaticLifecycleDestination {
                source: entry.into(),
                field,
            }
        )
    );
}

#[test]
fn external_and_intrinsic_calls_are_typed_leaves() {
    let external = lower_generic_source_to_preliminary_mir(
        "extern fn foreign() -> i64;
         fn caller() -> i64 { return foreign(); }
         fn main() -> i64 { return caller(); }",
    );
    let extracted = extract_preliminary_dependencies(&external).unwrap();
    assert!(extracted
        .dependencies()
        .iter()
        .any(|dependency| matches!(dependency.edge().target(), MirDependencyTarget::External(_))));

    let mut intrinsic = lower_generic_source_to_preliminary_mir(
        "fn leaf() -> i64 { return 1; }
         fn caller() -> i64 { return leaf(); }
         fn main() -> i64 { return caller(); }",
    );
    intrinsic.program_mut().declarations.entries_mut_for_test()[0].linkage =
        MirFunctionLinkage::Intrinsic {
            intrinsic: Intrinsic::Panic,
        };
    let extracted = extract_preliminary_dependencies(&intrinsic).unwrap();
    assert!(extracted.dependencies().iter().any(|dependency| {
        dependency.edge().target() == MirDependencyTarget::Intrinsic(Intrinsic::Panic)
    }));
}

#[test]
fn extraction_reports_malformed_call_identities() {
    let mut preliminary = lower_generic_source_to_preliminary_mir(
        "fn leaf() -> i64 { return 1; }
         fn caller() -> i64 { return leaf(); }
         fn main() -> i64 { return caller(); }",
    );
    let caller = FunctionId::new(1);
    let definition = preliminary
        .program_mut()
        .definitions
        .get_mut_for_test(caller)
        .unwrap();
    let call = definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    call.target = MirCallTarget::Direct(FunctionId::new(999));

    assert_eq!(
        extract_preliminary_dependencies(&preliminary),
        Err(MirDependencyExtractionError::UnknownFunction(
            FunctionId::new(999)
        ))
    );
}

#[test]
fn extraction_inventories_explicit_and_implicit_lifecycle_families() {
    let preliminary = lower_generic_source_to_preliminary_mir(
        "class Item {
           value: i64;
           init() { self.value = 1; }
           copy(ref other: Item) { self.value = other.value; }
           assign(ref other: Item) { self.value = other.value; }
           destroy { var observed: i64 = self.value; }
         }
         fn lifecycle() -> i64 {
           var first: Item = Item();
           var second: Item = first;
           first = second;
           var optional: Item? = Item();
           var items: Item[] = Item[]{Item()};
           return 0;
         }
         fn main() -> i64 { return lifecycle(); }",
    );
    let extracted = extract_preliminary_dependencies(&preliminary).unwrap();
    let kinds = extracted
        .dependencies()
        .iter()
        .map(|dependency| dependency.edge().kind())
        .collect::<BTreeSet<_>>();
    for kind in [
        MirDependencyEdgeKind::Initializer,
        MirDependencyEdgeKind::CopyConstructor,
        MirDependencyEdgeKind::CopyAssignment,
        MirDependencyEdgeKind::UserCopyBody,
        MirDependencyEdgeKind::UserDestructor,
        MirDependencyEdgeKind::CompleteFinalizer,
        MirDependencyEdgeKind::OptionalLifecycle,
        MirDependencyEdgeKind::ArrayCopy,
        MirDependencyEdgeKind::ArrayAssignment,
        MirDependencyEdgeKind::ArrayDestruction,
    ] {
        assert!(kinds.contains(&kind), "missing {kind:?}");
    }
}
