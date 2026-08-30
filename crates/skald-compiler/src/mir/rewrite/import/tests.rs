use std::convert::Infallible;

use crate::{
    identity::{BindingId, CallableId, FunctionId, LocalId},
    mir::{
        BlockId, MirCallTarget, MirFunctionDefinition, MirInstruction, MirStorageKind,
        MirTerminator, OptionalGuardId, PathConditionId, StorageId, ValueId,
    },
    passes::verify_final_mir,
    test_support::lower_source_to_final_mir,
};

use super::*;
use crate::mir::rewrite::{
    commit::commit, edit::test_support::fixture_parts, map::map_common_local_identities,
    rewrite_program, validate_function_local_identity_owners, BlockPlacement, MirCallableEdit,
    MirLocalIdentity, MirLocalIdentityMapper, MirLocalIdentitySite, MirReferenceFailure,
    MirRewriteError,
};

#[test]
fn complete_region_rehomes_every_local_identity_and_metadata_family() {
    let (source, mut destination) = source_and_destination();
    let request = complete_request(&source);
    let result = destination
        .import_region(&source, request)
        .expect("complete source region imports");

    for source_id in source.storage_ids() {
        let destination_id = result.maps.storage.destination(source_id).unwrap();
        assert_eq!(destination_id.callable(), destination.callable());
        assert_eq!(destination.storage(destination_id).unwrap().source, None);
    }
    for source_id in source.value_ids() {
        assert_eq!(
            result
                .maps
                .values
                .destination(source_id)
                .unwrap()
                .callable(),
            destination.callable()
        );
    }
    for source_id in source.block_ids() {
        assert_eq!(
            result
                .maps
                .blocks
                .destination(source_id)
                .unwrap()
                .callable(),
            destination.callable()
        );
    }
    for source_id in source.path_condition_ids() {
        assert_eq!(
            result
                .maps
                .path_conditions
                .destination(source_id)
                .unwrap()
                .callable(),
            destination.callable()
        );
    }
    let source_guard = source.optional_guard_ids().next().unwrap();
    let destination_guard = result
        .maps
        .optional_guards
        .destination(source_guard)
        .unwrap();
    assert_eq!(
        destination_guard,
        OptionalGuardId::new(destination.callable(), 3)
    );
    assert_eq!(result.logical_records.len(), 2);

    let source_child = source.path_condition_ids().nth(1).unwrap();
    let imported_child = result
        .maps
        .path_conditions
        .destination(source_child)
        .unwrap();
    assert_eq!(
        destination.path_condition(imported_child).unwrap().parent,
        Some(
            result
                .maps
                .path_conditions
                .destination(PathConditionId::new(source.callable(), 0))
                .unwrap()
        )
    );

    let imported_entry = result.maps.blocks.destination(source.entry()).unwrap();
    let source_optional = match &source.block(source.entry()).unwrap().terminator {
        Some(MirTerminator::BeginOptionalView { begin, .. }) => begin.optional,
        other => panic!("expected source optional-view terminator, got {other:?}"),
    };
    match &destination.block(imported_entry).unwrap().terminator {
        Some(MirTerminator::BeginOptionalView { begin, .. }) => {
            assert_eq!(begin.optional, source_optional);
            assert_eq!(begin.guard, destination_guard);
        }
        other => panic!("expected imported optional-view terminator, got {other:?}"),
    }

    let committed = commit(destination).expect("complete imported region commits densely");
    let function = MirFunctionDefinition {
        function: committed
            .callable
            .callable
            .as_function()
            .expect("test destination is a function"),
        return_storage: None,
        parameters: Vec::new(),
        storage: committed.callable.storage,
        values: committed.callable.values,
        body: committed.callable.body,
        span: source.block(source.entry()).unwrap().span,
    };
    validate_function_local_identity_owners(&function.clone())
        .expect("no source-local identity survives commit");
}

#[test]
fn partial_region_requires_and_applies_explicit_boundaries_atomically() {
    let (source, mut destination) = source_and_destination();
    let unchanged = destination.clone();
    let source_block = BlockId::new(source.callable(), 1);
    let source_guard = source.optional_guard_ids().next().unwrap();
    let mut missing = MirImportRequest::new(BlockPlacement::Append);
    missing.import_block(source_block);
    missing.import_optional_guard(source_guard);

    let error = destination.import_region(&source, missing).unwrap_err();
    assert_eq!(destination, unchanged);
    assert!(matches!(
        error,
        MirRewriteError::MissingImportSubstitution {
            identity: MirLocalIdentity::Storage(identity),
            site: MirLocalIdentitySite::Instruction {
                block: 1,
                instruction: 0
            }
        } if identity == StorageId::new(source.callable(), 0)
    ));

    let mut request = MirImportRequest::new(BlockPlacement::Append);
    request.import_block(source_block);
    request.import_optional_guard(source_guard);
    request.substitute_storage(
        StorageId::new(source.callable(), 0),
        StorageId::new(destination.callable(), 0),
    );
    request.substitute_value(
        ValueId::new(source.callable(), 0),
        ValueId::new(destination.callable(), 0),
    );
    let result = destination
        .import_region(&source, request)
        .expect("all partial-region boundaries are explicit");
    let imported = result.maps.blocks.destination(source_block).unwrap();
    assert!(destination.block(imported).is_ok());

    let mut exit_request = MirImportRequest::new(BlockPlacement::Append);
    exit_request.import_block(source.entry());
    exit_request.import_optional_guard(source_guard);
    exit_request.substitute_storage(
        StorageId::new(source.callable(), 0),
        StorageId::new(destination.callable(), 0),
    );
    exit_request.substitute_block(source_block, BlockId::new(destination.callable(), 1));
    destination
        .import_region(&source, exit_request)
        .expect("entry and exit block references use the typed boundary map");
}

#[test]
fn imported_storage_requires_explicit_source_free_destination_provenance() {
    let (mut source, mut destination) = source_and_destination();
    let source_storage = source.storage_ids().next().unwrap();
    source.storage[0].source = Some(BindingId::Local(LocalId::new(source.callable(), 0)));
    let mut invalid = MirImportRequest::new(BlockPlacement::Append);
    invalid.import_storage(source_storage, MirStorageKind::Return);
    assert!(matches!(
        destination.import_region(&source, invalid),
        Err(MirRewriteError::InvalidImportStorageKind { storage, .. })
            if storage == source_storage
    ));

    let mut valid = MirImportRequest::new(BlockPlacement::Append);
    valid.import_storage(source_storage, MirStorageKind::Temporary);
    let result = destination.import_region(&source, valid).unwrap();
    let imported = result.maps.storage.destination(source_storage).unwrap();
    assert_eq!(destination.storage(imported).unwrap().source, None);
    assert_eq!(
        destination.storage(imported).unwrap().kind,
        MirStorageKind::Temporary
    );
}

#[test]
fn foreign_binding_and_ambiguous_selection_fail_deterministically() {
    let (source_owner, mut storage, values, body) = fixture_parts();
    let foreign = CallableId::Function(FunctionId::new(8_000));
    storage[0].source = Some(BindingId::Receiver(foreign));
    assert!(matches!(
        MirImportSource::from_common_parts(source_owner, storage, values, body),
        Err(MirRewriteError::ForeignImportBinding {
            expected,
            binding: BindingId::Receiver(actual),
            ..
        }) if expected == source_owner && actual == foreign
    ));

    let (source, mut destination) = source_and_destination();
    let source_value = source.value_ids().next().unwrap();
    let mut request = MirImportRequest::new(BlockPlacement::Append);
    request.import_value(source_value);
    request.substitute_value(source_value, ValueId::new(destination.callable(), 0));
    assert!(matches!(
        destination.import_region(&source, request),
        Err(MirRewriteError::SelectedImportIdentityHasSubstitution {
            identity: MirLocalIdentity::Value(identity)
        }) if identity == source_value
    ));
}

#[test]
fn foreign_source_reference_is_not_treated_as_a_boundary() {
    let (mut source, mut destination) = source_and_destination();
    let source_block = BlockId::new(source.callable(), 1);
    let foreign = CallableId::Function(FunctionId::new(8_001));
    source.blocks[1].terminator = Some(MirTerminator::Return {
        value: Some(ValueId::new(foreign, 0)),
        span: source.blocks[1].span,
    });
    let mut request = MirImportRequest::new(BlockPlacement::Append);
    request.import_block(source_block);
    request.import_optional_guard(source.optional_guard_ids().next().unwrap());
    request.substitute_storage(
        StorageId::new(source.callable(), 0),
        StorageId::new(destination.callable(), 0),
    );

    assert!(matches!(
        destination.import_region(&source, request),
        Err(MirRewriteError::InvalidReference {
            expected,
            identity: MirLocalIdentity::Value(identity),
            site: MirLocalIdentitySite::Terminator(1),
            failure: MirReferenceFailure::Foreign,
        }) if expected == source.callable() && identity.callable() == foreign
    ));
}

#[test]
fn callable_header_storage_requires_explicit_role_substitutions() {
    let (mut source, mut destination) = source_and_destination();
    let receiver = StorageId::new(source.callable(), 0);
    let parameter = StorageId::new(source.callable(), 1);
    let result_storage = StorageId::new(source.callable(), 2);
    source.receiver = Some(receiver);
    source.parameters = vec![parameter];
    source.return_storage = Some(result_storage);

    let missing = MirImportRequest::new(BlockPlacement::Append);
    assert!(matches!(
        destination.import_region(&source, missing),
        Err(MirRewriteError::MissingImportSubstitution {
            identity: MirLocalIdentity::Storage(identity),
            site: MirLocalIdentitySite::Receiver,
        }) if identity == receiver
    ));

    let mut complete = MirImportRequest::new(BlockPlacement::Append);
    complete.substitute_storage(receiver, StorageId::new(destination.callable(), 0));
    complete.substitute_storage(parameter, StorageId::new(destination.callable(), 1));
    complete.substitute_storage(result_storage, StorageId::new(destination.callable(), 2));
    let result = destination.import_region(&source, complete).unwrap();
    assert_eq!(
        result.maps.storage.destination(receiver).unwrap(),
        StorageId::new(destination.callable(), 0)
    );
}

#[test]
fn repeated_imports_are_collision_free_and_deterministic() {
    let (source, destination) = source_and_destination();
    let mut first = destination.clone();
    let mut second = destination;
    let first_result = first
        .import_region(&source, complete_request(&source))
        .unwrap();
    let second_result = second
        .import_region(&source, complete_request(&source))
        .unwrap();
    assert_eq!(first_result, second_result);
    assert_eq!(first, second);

    let next = first
        .import_region(&source, complete_request(&source))
        .unwrap();
    let source_guard = source.optional_guard_ids().next().unwrap();
    assert!(
        next.maps.optional_guards.destination(source_guard).unwrap()
            > first_result
                .maps
                .optional_guards
                .destination(source_guard)
                .unwrap()
    );
    commit(first).expect("repeated imports retain deterministic non-colliding slots");
}

#[test]
fn complete_parameterless_import_passes_final_verification() {
    let original = lower_source_to_final_mir(
        "fn leaf() -> i64 { return 7; } fn constant() -> i64 { return leaf(); } fn main() -> i64 { return 0; }",
    );
    let source_definition = original
        .executable_definitions()
        .find(|definition| {
            definition.body().blocks.iter().any(|block| {
                block
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, MirInstruction::Call(_)))
            })
        })
        .expect("source definition containing a semantic call target");
    let expected_target = source_definition
        .body()
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call.target),
            _ => None,
        })
        .unwrap();
    assert!(matches!(expected_target, MirCallTarget::Direct(_)));
    let source = MirImportSource::snapshot(source_definition).unwrap();
    assert!(source.receiver().is_none());
    assert!(source.parameters().is_empty());
    assert!(source.return_storage().is_none());
    let entry = original.entry_function;
    let result = rewrite_program(original, |callable, destination| {
        if callable != CallableId::Function(entry) {
            return Ok(());
        }
        let mut request = MirImportRequest::new(BlockPlacement::Append);
        for value in source.value_ids() {
            request.import_value(value);
        }
        for block in source.block_ids() {
            request.import_block(block);
        }
        destination.import_region(&source, request)?;
        Ok(())
    })
    .expect("complete source callable imports into entry callable");

    let imported_target = result
        .program
        .definitions
        .get(entry)
        .unwrap()
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call.target),
            _ => None,
        })
        .unwrap();
    assert_eq!(imported_target, expected_target);
    verify_final_mir(result.program).expect("unreachable imported region remains valid final MIR");
}

fn source_and_destination() -> (MirImportSource, MirCallableEdit) {
    let (source_owner, storage, values, body) = fixture_parts();
    let source = MirImportSource::from_common_parts(
        source_owner,
        storage.clone(),
        values.clone(),
        body.clone(),
    )
    .unwrap();
    let destination_owner = CallableId::Function(FunctionId::new(9_000));
    let (mut destination_storage, mut destination_values, mut destination_body) =
        (storage, values, body);
    let mut mapper = TestReowner {
        destination: destination_owner,
    };
    map_common_local_identities(
        &mut destination_storage,
        &mut destination_values,
        &mut destination_body,
        &mut mapper,
    )
    .unwrap();
    let destination = MirCallableEdit::from_dense_parts(
        destination_owner,
        destination_storage,
        destination_values,
        destination_body,
    )
    .unwrap();
    (source, destination)
}

fn complete_request(source: &MirImportSource) -> MirImportRequest {
    let mut request = MirImportRequest::new(BlockPlacement::Append);
    for storage in source.storage_ids() {
        request.import_storage(storage, MirStorageKind::Temporary);
    }
    for value in source.value_ids() {
        request.import_value(value);
    }
    for block in source.block_ids() {
        request.import_block(block);
    }
    for condition in source.path_condition_ids() {
        request.import_path_condition(condition);
    }
    for record in source.logical_record_indices() {
        request.import_logical_record(record);
    }
    for guard in source.optional_guard_ids() {
        request.import_optional_guard(guard);
    }
    request
}

struct TestReowner {
    destination: CallableId,
}

impl MirLocalIdentityMapper for TestReowner {
    type Error = Infallible;

    fn map_storage(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: StorageId,
    ) -> Result<StorageId, Self::Error> {
        Ok(StorageId::new(self.destination, identity.index()))
    }

    fn map_value(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: ValueId,
    ) -> Result<ValueId, Self::Error> {
        Ok(ValueId::new(self.destination, identity.index()))
    }

    fn map_block(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: BlockId,
    ) -> Result<BlockId, Self::Error> {
        Ok(BlockId::new(self.destination, identity.index()))
    }

    fn map_path_condition(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: PathConditionId,
    ) -> Result<PathConditionId, Self::Error> {
        Ok(PathConditionId::new(self.destination, identity.index()))
    }

    fn map_optional_guard(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: OptionalGuardId,
    ) -> Result<OptionalGuardId, Self::Error> {
        Ok(OptionalGuardId::new(self.destination, identity.index()))
    }
}
