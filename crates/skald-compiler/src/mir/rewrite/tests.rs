use std::convert::Infallible;

use crate::{
    identity::{
        ArrayTypeId, BindingId, CallableId, ClassId, LocalId, MethodId, OptionalBoxTypeId,
        OptionalTypeId, StaticFieldId,
    },
    test_support::lower_source_to_mir,
};

use super::*;
use crate::mir::{
    test_fixtures::empty_member_definition, BlockId, MirAliasAccess, MirArrayInstruction,
    MirBasicBlock, MirBody, MirCall, MirCallReceiver, MirCallTarget, MirCheckedViewBinding,
    MirF64ToIntegerRange, MirFunctionDefinition, MirIndirectCallTarget, MirInstruction,
    MirIoBuffer, MirIoInstruction, MirIoOperation, MirLogicalExpression, MirLogicalOperation,
    MirObjectOrigin, MirObjectView, MirOptionalBoxViewEnd, MirOptionalViewBegin, MirPathCondition,
    MirPathConditionValue, MirPlace, MirPlaceProjection, MirPrimitiveCastRangeCheck, MirRvalue,
    MirRvalueKind, MirSharedAllocate, MirSharedAllocationMode, MirSharedAllocationOrigin,
    MirSharedAllocationTarget, MirSharedCopy, MirSharedTarget, MirStaticInitializerBody,
    MirStaticPublication, MirStorage, MirStorageKind, MirStorageLive, MirTerminator, MirType,
    MirValue, MirViewProvenance, MirViewTarget, OptionalGuardId, PathConditionId, StorageId,
    ValueId,
};

#[derive(Default)]
struct Collector {
    visits: Vec<(MirLocalIdentitySite, MirLocalIdentity)>,
}

impl Collector {
    fn record<T>(
        &mut self,
        site: MirLocalIdentitySite,
        identity: MirLocalIdentity,
        value: T,
    ) -> Result<T, Infallible> {
        self.visits.push((site, identity));
        Ok(value)
    }
}

impl MirLocalIdentityMapper for Collector {
    type Error = Infallible;

    fn map_storage(
        &mut self,
        site: MirLocalIdentitySite,
        identity: StorageId,
    ) -> Result<StorageId, Self::Error> {
        self.record(site, MirLocalIdentity::Storage(identity), identity)
    }

    fn map_value(
        &mut self,
        site: MirLocalIdentitySite,
        identity: ValueId,
    ) -> Result<ValueId, Self::Error> {
        self.record(site, MirLocalIdentity::Value(identity), identity)
    }

    fn map_block(
        &mut self,
        site: MirLocalIdentitySite,
        identity: BlockId,
    ) -> Result<BlockId, Self::Error> {
        self.record(site, MirLocalIdentity::Block(identity), identity)
    }

    fn map_path_condition(
        &mut self,
        site: MirLocalIdentitySite,
        identity: PathConditionId,
    ) -> Result<PathConditionId, Self::Error> {
        self.record(site, MirLocalIdentity::PathCondition(identity), identity)
    }

    fn map_optional_guard(
        &mut self,
        site: MirLocalIdentitySite,
        identity: OptionalGuardId,
    ) -> Result<OptionalGuardId, Self::Error> {
        self.record(site, MirLocalIdentity::OptionalGuard(identity), identity)
    }
}

struct ReindexBy(usize);

impl MirLocalIdentityMapper for ReindexBy {
    type Error = Infallible;

    fn map_storage(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: StorageId,
    ) -> Result<StorageId, Self::Error> {
        Ok(StorageId::new(
            identity.callable(),
            identity.index() + self.0,
        ))
    }

    fn map_value(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: ValueId,
    ) -> Result<ValueId, Self::Error> {
        Ok(ValueId::new(identity.callable(), identity.index() + self.0))
    }

    fn map_block(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: BlockId,
    ) -> Result<BlockId, Self::Error> {
        Ok(BlockId::new(identity.callable(), identity.index() + self.0))
    }

    fn map_path_condition(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: PathConditionId,
    ) -> Result<PathConditionId, Self::Error> {
        Ok(PathConditionId::new(
            identity.callable(),
            identity.index() + self.0,
        ))
    }

    fn map_optional_guard(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: OptionalGuardId,
    ) -> Result<OptionalGuardId, Self::Error> {
        Ok(OptionalGuardId::new(
            identity.callable(),
            identity.index() + self.0,
        ))
    }
}

fn representative_function() -> MirFunctionDefinition {
    let program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let mut definition = program
        .definitions
        .get(program.entry_function)
        .expect("entry definition")
        .clone();
    let callable = definition.callable();
    let span = definition.span;
    let class = ClassId::new(0);
    let array = ArrayTypeId::new(0);
    let optional = OptionalTypeId::new(0);
    let box_target = OptionalBoxTypeId::new(0);
    let storage = |index| StorageId::new(callable, index);
    let value = |index| ValueId::new(callable, index);
    let block = |index| BlockId::new(callable, index);
    let condition = PathConditionId::new(callable, 0);
    let guard = OptionalGuardId::new(callable, 0);

    definition.return_storage = Some(storage(0));
    definition.parameters = vec![storage(1)];
    definition.storage = (0..8)
        .map(|index| MirStorage {
            id: storage(index),
            source: (index == 2).then(|| BindingId::Local(LocalId::new(callable, 0))),
            name: format!("storage{index}"),
            kind: MirStorageKind::Temporary,
            ty: MirType::I64,
            span,
        })
        .collect();
    definition.values = (0..5)
        .map(|index| MirValue {
            id: value(index),
            ty: MirType::I64,
            span,
        })
        .collect();

    let mut projected = MirPlace::base(storage(2));
    projected
        .projections
        .push(MirPlaceProjection::ArrayElement {
            array,
            normalized_index: storage(3),
        });
    let exact_view = MirObjectView {
        source: MirPlace::base(storage(2)),
        origin: Box::new(MirObjectOrigin::Exact {
            complete: MirPlace::base(storage(2)),
            dynamic_class: class,
        }),
        target: MirViewTarget::Class(class),
        access: MirAliasAccess::ReadOnly,
        provenance: MirViewProvenance::Ordinary,
        span,
    };
    let instructions = vec![
        MirInstruction::StorageLive(MirStorageLive {
            storage: storage(2),
            span,
        }),
        MirInstruction::Assign(crate::mir::MirAssignment {
            result: value(0),
            rvalue: MirRvalue {
                kind: MirRvalueKind::PathCondition(MirPathConditionValue {
                    condition,
                    activation: storage(4),
                }),
                ty: MirType::Bool,
            },
            span,
        }),
        MirInstruction::Call(MirCall {
            target: MirCallTarget::Indirect(MirIndirectCallTarget {
                callee: value(1),
                function_type: crate::identity::FunctionTypeId::new(0),
            }),
            receiver: Some(MirCallReceiver::Interface(exact_view.clone())),
            arguments: vec![crate::mir::MirArgument::Place(projected.clone())],
            result: Some(value(2)),
            shared_result: Some(storage(5)),
            destination: Some(MirPlace::base(storage(6))),
            span,
        }),
        MirInstruction::BindCheckedView(MirCheckedViewBinding {
            destination: storage(7),
            view: exact_view,
            span,
        }),
        MirInstruction::SharedAllocate(MirSharedAllocate {
            allocation: storage(5),
            target: MirSharedAllocationTarget::Class(class),
            origin: MirSharedAllocationOrigin::New,
            mode: MirSharedAllocationMode::Copy {
                source: MirPlace::base(storage(2)),
            },
            span,
        }),
        MirInstruction::SharedCopy(MirSharedCopy {
            destination: storage(6),
            source: storage(5),
            span,
        }),
        MirInstruction::EndOptionalBoxView(MirOptionalBoxViewEnd {
            box_target,
            layer: 0,
            guard,
            owner: storage(5),
            span,
        }),
        MirInstruction::Array(MirArrayInstruction::Normalize {
            destination: storage(3),
            owner: projected.clone(),
            index: value(3),
            array,
            kind: crate::mir::MirArrayPositionKind::Element,
            span,
        }),
        MirInstruction::Io(MirIoInstruction {
            result: value(4),
            operation: MirIoOperation::Read {
                handle: value(1),
                destination: MirIoBuffer {
                    place: projected,
                    anchor: storage(6),
                    array,
                    access: MirAliasAccess::Mutable,
                },
                offset: storage(3),
            },
            span,
        }),
    ];
    definition.body = MirBody {
        entry: block(0),
        blocks: vec![
            MirBasicBlock {
                id: block(0),
                instructions,
                terminator: Some(MirTerminator::BeginOptionalView {
                    begin: MirOptionalViewBegin {
                        optional,
                        guard,
                        source: MirPlace::base(storage(2)),
                        payload: MirType::I64,
                        span,
                    },
                    success_target: block(1),
                    absent_target: block(2),
                    overflow_target: block(2),
                    span,
                }),
                span,
            },
            MirBasicBlock {
                id: block(1),
                instructions: vec![],
                terminator: Some(MirTerminator::PrimitiveCastRangeCheck {
                    check: MirPrimitiveCastRangeCheck {
                        relation: MirF64ToIntegerRange {
                            target: crate::mir::MirIntegerType::I64,
                        },
                        source: storage(0),
                        result: storage(1),
                    },
                    success_target: block(2),
                    failure_target: block(2),
                    span,
                }),
                span,
            },
            MirBasicBlock {
                id: block(2),
                instructions: vec![],
                terminator: Some(MirTerminator::Return {
                    value: Some(value(0)),
                    span,
                }),
                span,
            },
        ],
        path_conditions: vec![MirPathCondition {
            id: condition,
            parent: None,
            activation: storage(4),
            active_predecessor: block(1),
            inactive_predecessor: block(2),
            merge: block(2),
            span,
        }],
        logical_expressions: vec![MirLogicalExpression {
            operation: MirLogicalOperation::And,
            condition,
            result: storage(4),
            left_result: value(0),
            split: block(0),
            selection: block(0),
            right_entry: block(1),
            right_exit: block(1),
            right_result: value(1),
            short: block(2),
            join: block(2),
            selected_result: value(2),
            span,
        }],
    };
    definition
}

#[test]
fn collector_covers_all_identity_families_and_representative_model_families() {
    let mut definition = representative_function();
    let mut collector = Collector::default();
    map_function_local_identities(&mut definition, &mut collector).unwrap();

    for predicate in [
        |identity| matches!(identity, MirLocalIdentity::Storage(_)),
        |identity| matches!(identity, MirLocalIdentity::Value(_)),
        |identity| matches!(identity, MirLocalIdentity::Block(_)),
        |identity| matches!(identity, MirLocalIdentity::PathCondition(_)),
        |identity| matches!(identity, MirLocalIdentity::OptionalGuard(_)),
    ] {
        assert!(collector
            .visits
            .iter()
            .any(|(_, identity)| predicate(*identity)));
    }
    for site in [
        MirLocalIdentitySite::ReturnStorage,
        MirLocalIdentitySite::Parameter(0),
        MirLocalIdentitySite::StorageDeclaration(0),
        MirLocalIdentitySite::ValueDeclaration(0),
        MirLocalIdentitySite::BodyEntry,
        MirLocalIdentitySite::BlockDeclaration(0),
        MirLocalIdentitySite::Instruction {
            block: 0,
            instruction: 0,
        },
        MirLocalIdentitySite::Terminator(0),
        MirLocalIdentitySite::PathCondition(0),
        MirLocalIdentitySite::LogicalExpression(0),
    ] {
        assert!(collector.visits.iter().any(|(visited, _)| *visited == site));
    }
}

#[test]
fn remapper_updates_nested_references_but_not_semantic_identities_or_bindings() {
    let mut definition = representative_function();
    let function = definition.function;
    let binding = definition.storage[2].source;
    map_function_local_identities(&mut definition, &mut ReindexBy(100)).unwrap();

    assert_eq!(definition.function, function);
    assert_eq!(definition.storage[2].source, binding);
    assert_eq!(definition.return_storage.unwrap().index(), 100);
    assert_eq!(definition.body.entry.index(), 100);
    assert_eq!(definition.body.path_conditions[0].id.index(), 100);
    match &definition.body.blocks[0].instructions[7] {
        MirInstruction::Array(MirArrayInstruction::Normalize {
            destination,
            owner,
            index,
            array: _,
            kind: _,
            span: _,
        }) => {
            assert_eq!(destination.index(), 103);
            assert_eq!(owner.base.expect_local_storage().index(), 102);
            assert_eq!(index.index(), 103);
        }
        instruction => panic!("expected array normalization, got {instruction:?}"),
    }
}

#[test]
fn visit_order_is_deterministic() {
    let mut first = representative_function();
    let mut second = first.clone();
    let mut first_collector = Collector::default();
    let mut second_collector = Collector::default();
    map_function_local_identities(&mut first, &mut first_collector).unwrap();
    map_function_local_identities(&mut second, &mut second_collector).unwrap();
    assert_eq!(first_collector.visits, second_collector.visits);
}

#[test]
fn owner_validation_reports_the_exact_structural_site() {
    let mut definition = representative_function();
    let foreign = CallableId::Function(crate::identity::FunctionId::new(99));
    let MirTerminator::Return { value, span: _ } = definition.body.blocks[2]
        .terminator
        .as_mut()
        .expect("return terminator")
    else {
        panic!("expected return terminator");
    };
    *value = Some(ValueId::new(foreign, 0));

    let error = validate_function_local_identity_owners(&mut definition).unwrap_err();
    assert_eq!(error.expected, definition.callable());
    assert_eq!(
        error.identity,
        MirLocalIdentity::Value(ValueId::new(foreign, 0))
    );
    assert_eq!(error.site, MirLocalIdentitySite::Terminator(2));
    assert!(error.to_string().contains("terminator in block 2"));
}

#[test]
fn member_and_static_initializer_attachments_share_the_same_traversal() {
    let base = representative_function();
    let span = base.span;
    let class = ClassId::new(7);
    let callable = CallableId::Method(MethodId::new(class, 0));
    let mut member = empty_member_definition(callable, class, &[], span);
    let mut member_collector = Collector::default();
    map_member_local_identities(&mut member, &mut member_collector).unwrap();
    assert!(member_collector
        .visits
        .iter()
        .any(|(site, _)| *site == MirLocalIdentitySite::Receiver));
    validate_member_local_identity_owners(&mut member).unwrap();

    let field = StaticFieldId::new(class, 0);
    let static_callable = CallableId::StaticInitializer(field.into());
    let block = BlockId::new(static_callable, 0);
    let mut initializer = MirStaticInitializerBody {
        id: field.into(),
        field,
        destination_type: MirType::I64,
        publication: MirStaticPublication {
            initialization_exit: block,
            cleanup_entry: block,
            span,
        },
        storage: vec![],
        values: vec![],
        body: MirBody {
            entry: block,
            blocks: vec![MirBasicBlock {
                id: block,
                instructions: vec![],
                terminator: Some(MirTerminator::Return { value: None, span }),
                span,
            }],
            path_conditions: vec![],
            logical_expressions: vec![],
        },
        span,
    };
    let mut static_collector = Collector::default();
    map_static_initializer_local_identities(&mut initializer, &mut static_collector).unwrap();
    assert_eq!(
        static_collector.visits[0].0,
        MirLocalIdentitySite::StaticPublicationInitializationExit
    );
    assert_eq!(
        static_collector.visits[1].0,
        MirLocalIdentitySite::StaticPublicationCleanupEntry
    );
    validate_static_initializer_local_identity_owners(&mut initializer).unwrap();

    initializer.publication.cleanup_entry = BlockId::new(
        CallableId::Function(crate::identity::FunctionId::new(42)),
        0,
    );
    let error = validate_static_initializer_local_identity_owners(&mut initializer).unwrap_err();
    assert_eq!(
        error.site,
        MirLocalIdentitySite::StaticPublicationCleanupEntry
    );
}

#[test]
fn identity_mapper_preserves_the_complete_definition() {
    let mut definition = representative_function();
    let expected = definition.clone();
    super::map::preserve_function_local_identities(&mut definition).unwrap();
    assert_eq!(definition, expected);
}
