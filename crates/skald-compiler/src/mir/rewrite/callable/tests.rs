use crate::{
    identity::{CallableId, ClassId, FunctionId, MethodId, StaticFieldId, StaticInitializerId},
    mir::{
        BlockId, MirBasicBlock, MirBody, MirFunctionDefinition, MirMemberDefinition,
        MirStaticInitializerBody, MirStaticPublication, MirStorage, MirStorageKind, MirTerminator,
        MirType, PathConditionId, StorageId,
    },
    test_support::lower_source_to_mir,
};

use super::*;
use crate::mir::rewrite::{
    edit::BlockPlacement,
    error::{MirReferenceFailure, MirRewriteError},
    MirLocalIdentity, MirLocalIdentitySite,
};

#[test]
fn every_header_attachment_reports_its_exact_deleted_reference_site() {
    let span = fixture_span();
    let function = FunctionId::new(0);
    let function_owner = CallableId::Function(function);
    let mut return_package = MirCallablePackage::from_function(MirFunctionDefinition {
        function,
        return_storage: Some(StorageId::new(function_owner, 2)),
        parameters: vec![],
        storage: storage(function_owner, span),
        values: vec![],
        body: body(function_owner, span),
        span,
    })
    .unwrap();
    return_package
        .edit_mut()
        .remove_storage(StorageId::new(function_owner, 2))
        .unwrap();
    assert_deleted_attachment(
        return_package,
        MirLocalIdentity::Storage(StorageId::new(function_owner, 2)),
        MirLocalIdentitySite::ReturnStorage,
    );

    let mut parameter_package = MirCallablePackage::from_function(MirFunctionDefinition {
        function,
        return_storage: None,
        parameters: vec![StorageId::new(function_owner, 1)],
        storage: storage(function_owner, span),
        values: vec![],
        body: body(function_owner, span),
        span,
    })
    .unwrap();
    parameter_package
        .edit_mut()
        .remove_storage(StorageId::new(function_owner, 1))
        .unwrap();
    assert_deleted_attachment(
        parameter_package,
        MirLocalIdentity::Storage(StorageId::new(function_owner, 1)),
        MirLocalIdentitySite::Parameter(0),
    );

    let class = ClassId::new(0);
    let member_owner = CallableId::Method(MethodId::new(class, 0));
    let mut receiver_package = MirCallablePackage::from_member(MirMemberDefinition {
        callable: member_owner,
        class_owner: class,
        return_storage: None,
        receiver: Some(StorageId::new(member_owner, 2)),
        parameters: vec![],
        storage: storage(member_owner, span),
        values: vec![],
        body: body(member_owner, span),
        span,
    })
    .unwrap();
    receiver_package
        .edit_mut()
        .remove_storage(StorageId::new(member_owner, 2))
        .unwrap();
    assert_deleted_attachment(
        receiver_package,
        MirLocalIdentity::Storage(StorageId::new(member_owner, 2)),
        MirLocalIdentitySite::Receiver,
    );

    let initializer = StaticInitializerId::from(StaticFieldId::new(class, 0));
    let static_owner = CallableId::StaticInitializer(initializer);
    for (removed, site) in [
        (
            BlockId::new(static_owner, 1),
            MirLocalIdentitySite::StaticPublicationInitializationExit,
        ),
        (
            BlockId::new(static_owner, 2),
            MirLocalIdentitySite::StaticPublicationCleanupEntry,
        ),
    ] {
        let mut package =
            MirCallablePackage::from_static_initializer(static_initializer(initializer, span))
                .unwrap();
        package.edit_mut().remove_block(removed).unwrap();
        assert_deleted_attachment(package, MirLocalIdentity::Block(removed), site);
    }
}

#[test]
fn static_publication_follows_explicit_block_order_during_atomic_commit() {
    let span = fixture_span();
    let class = ClassId::new(0);
    let initializer = StaticInitializerId::from(StaticFieldId::new(class, 0));
    let owner = CallableId::StaticInitializer(initializer);
    let mut package =
        MirCallablePackage::from_static_initializer(static_initializer(initializer, span)).unwrap();
    let inserted = package
        .edit_mut()
        .allocate_block(BlockPlacement::Before(BlockId::new(owner, 1)), |identity| {
            empty_block(identity, span)
        })
        .unwrap();
    assert_eq!(inserted, BlockId::new(owner, 3));

    let committed = package.commit().unwrap();
    let MirCommittedDefinition::StaticInitializer(definition) = committed.definition else {
        panic!("static initializer package must retain its definition kind")
    };
    assert_eq!(
        definition.publication.initialization_exit,
        BlockId::new(owner, 2)
    );
    assert_eq!(definition.publication.cleanup_entry, BlockId::new(owner, 3));
    assert_eq!(
        definition
            .body
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        (0..4)
            .map(|index| BlockId::new(owner, index))
            .collect::<Vec<_>>()
    );
}

fn assert_deleted_attachment(
    package: MirCallablePackage,
    identity: MirLocalIdentity,
    site: MirLocalIdentitySite,
) {
    assert_eq!(
        package.commit(),
        Err(MirRewriteError::InvalidReference {
            expected: identity.callable(),
            identity,
            site,
            failure: MirReferenceFailure::Deleted,
        })
    );
}

fn static_initializer(
    id: StaticInitializerId,
    span: crate::source::Span,
) -> MirStaticInitializerBody {
    let owner = CallableId::StaticInitializer(id);
    MirStaticInitializerBody {
        id,
        field: id.field(),
        destination_type: MirType::I64,
        publication: MirStaticPublication {
            initialization_exit: BlockId::new(owner, 1),
            cleanup_entry: BlockId::new(owner, 2),
            span,
        },
        storage: storage(owner, span),
        values: vec![],
        body: body(owner, span),
        span,
    }
}

fn storage(owner: CallableId, span: crate::source::Span) -> Vec<MirStorage> {
    (0..3)
        .map(|index| MirStorage {
            id: StorageId::new(owner, index),
            source: None,
            name: format!("storage{index}"),
            kind: MirStorageKind::Temporary,
            ty: MirType::I64,
            span,
        })
        .collect()
}

fn body(owner: CallableId, span: crate::source::Span) -> MirBody {
    MirBody {
        entry: BlockId::new(owner, 0),
        blocks: (0..3)
            .map(|index| empty_block(BlockId::new(owner, index), span))
            .collect(),
        path_conditions: Vec::new(),
        logical_expressions: Vec::new(),
    }
}

fn empty_block(identity: BlockId, span: crate::source::Span) -> MirBasicBlock {
    MirBasicBlock {
        id: identity,
        instructions: Vec::new(),
        terminator: Some(MirTerminator::Return { value: None, span }),
        span,
    }
}

fn fixture_span() -> crate::source::Span {
    lower_source_to_mir("fn main() -> i64 { return 0; }").span
}
