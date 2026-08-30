use super::super::*;
use super::{
    identity::LocalIdentityOwnerValidator, MirLocalIdentityMapper, MirLocalIdentityOwnershipError,
    MirLocalIdentitySite,
};

#[cfg(test)]
use super::identity::PreserveLocalIdentities;

pub(crate) fn map_function_local_identities<M: MirLocalIdentityMapper>(
    definition: &mut MirFunctionDefinition,
    mapper: &mut M,
) -> Result<(), M::Error> {
    let MirFunctionDefinition {
        function: _,
        return_storage,
        parameters,
        storage,
        values,
        body,
        span: _,
    } = definition;
    map_optional_storage(mapper, MirLocalIdentitySite::ReturnStorage, return_storage)?;
    map_parameters(parameters, mapper)?;
    map_common_local_identities(storage, values, body, mapper)
}

pub(crate) fn map_member_local_identities<M: MirLocalIdentityMapper>(
    definition: &mut MirMemberDefinition,
    mapper: &mut M,
) -> Result<(), M::Error> {
    let MirMemberDefinition {
        callable: _,
        class_owner: _,
        return_storage,
        receiver,
        parameters,
        storage,
        values,
        body,
        span: _,
    } = definition;
    map_optional_storage(mapper, MirLocalIdentitySite::ReturnStorage, return_storage)?;
    map_optional_storage(mapper, MirLocalIdentitySite::Receiver, receiver)?;
    map_parameters(parameters, mapper)?;
    map_common_local_identities(storage, values, body, mapper)
}

pub(crate) fn map_static_initializer_local_identities<M: MirLocalIdentityMapper>(
    definition: &mut MirStaticInitializerBody,
    mapper: &mut M,
) -> Result<(), M::Error> {
    let MirStaticInitializerBody {
        id: _,
        field: _,
        destination_type: _,
        publication,
        storage,
        values,
        body,
        span: _,
    } = definition;
    let MirStaticPublication {
        initialization_exit,
        cleanup_entry,
        span: _,
    } = publication;
    map_block(
        mapper,
        MirLocalIdentitySite::StaticPublicationInitializationExit,
        initialization_exit,
    )?;
    map_block(
        mapper,
        MirLocalIdentitySite::StaticPublicationCleanupEntry,
        cleanup_entry,
    )?;
    map_common_local_identities(storage, values, body, mapper)
}

pub(crate) fn validate_function_local_identity_owners(
    definition: &mut MirFunctionDefinition,
) -> Result<(), MirLocalIdentityOwnershipError> {
    let mut validator = LocalIdentityOwnerValidator::new(definition.callable());
    map_function_local_identities(definition, &mut validator)
}

pub(crate) fn validate_member_local_identity_owners(
    definition: &mut MirMemberDefinition,
) -> Result<(), MirLocalIdentityOwnershipError> {
    let mut validator = LocalIdentityOwnerValidator::new(definition.callable);
    map_member_local_identities(definition, &mut validator)
}

pub(crate) fn validate_static_initializer_local_identity_owners(
    definition: &mut MirStaticInitializerBody,
) -> Result<(), MirLocalIdentityOwnershipError> {
    let mut validator = LocalIdentityOwnerValidator::new(definition.callable());
    map_static_initializer_local_identities(definition, &mut validator)
}

fn map_parameters<M: MirLocalIdentityMapper>(
    parameters: &mut [StorageId],
    mapper: &mut M,
) -> Result<(), M::Error> {
    for (index, parameter) in parameters.iter_mut().enumerate() {
        map_storage(mapper, MirLocalIdentitySite::Parameter(index), parameter)?;
    }
    Ok(())
}

pub(super) fn map_common_local_identities<M: MirLocalIdentityMapper>(
    storage: &mut [MirStorage],
    values: &mut [MirValue],
    body: &mut MirBody,
    mapper: &mut M,
) -> Result<(), M::Error> {
    for (index, declaration) in storage.iter_mut().enumerate() {
        let MirStorage {
            id,
            source: _,
            name: _,
            kind: _,
            ty: _,
            span: _,
        } = declaration;
        map_storage(mapper, MirLocalIdentitySite::StorageDeclaration(index), id)?;
    }
    for (index, declaration) in values.iter_mut().enumerate() {
        let MirValue { id, ty: _, span: _ } = declaration;
        map_value(mapper, MirLocalIdentitySite::ValueDeclaration(index), id)?;
    }
    map_body_local_identities(body, mapper)
}

pub(super) fn map_body_local_identities<M: MirLocalIdentityMapper>(
    body: &mut MirBody,
    mapper: &mut M,
) -> Result<(), M::Error> {
    let MirBody {
        entry,
        blocks,
        path_conditions,
        logical_expressions,
    } = body;
    map_block(mapper, MirLocalIdentitySite::BodyEntry, entry)?;
    for (block_index, block) in blocks.iter_mut().enumerate() {
        let MirBasicBlock {
            id,
            instructions,
            terminator,
            span: _,
        } = block;
        map_block(
            mapper,
            MirLocalIdentitySite::BlockDeclaration(block_index),
            id,
        )?;
        for (instruction_index, instruction) in instructions.iter_mut().enumerate() {
            map_instruction(
                instruction,
                mapper,
                MirLocalIdentitySite::Instruction {
                    block: block_index,
                    instruction: instruction_index,
                },
            )?;
        }
        if let Some(terminator) = terminator {
            map_terminator(
                terminator,
                mapper,
                MirLocalIdentitySite::Terminator(block_index),
            )?;
        }
    }
    for (index, condition) in path_conditions.iter_mut().enumerate() {
        map_path_condition_metadata(
            condition,
            mapper,
            MirLocalIdentitySite::PathCondition(index),
        )?;
    }
    for (index, expression) in logical_expressions.iter_mut().enumerate() {
        map_logical_expression(
            expression,
            mapper,
            MirLocalIdentitySite::LogicalExpression(index),
        )?;
    }
    Ok(())
}

fn map_path_condition_metadata<M: MirLocalIdentityMapper>(
    condition: &mut MirPathCondition,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirPathCondition {
        id,
        parent,
        activation,
        active_predecessor,
        inactive_predecessor,
        merge,
        span: _,
    } = condition;
    map_path_condition(mapper, site, id)?;
    if let Some(parent) = parent {
        map_path_condition(mapper, site, parent)?;
    }
    map_storage(mapper, site, activation)?;
    map_block(mapper, site, active_predecessor)?;
    map_block(mapper, site, inactive_predecessor)?;
    map_block(mapper, site, merge)
}

fn map_logical_expression<M: MirLocalIdentityMapper>(
    expression: &mut MirLogicalExpression,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirLogicalExpression {
        operation: _,
        condition,
        result,
        left_result,
        split,
        selection,
        right_entry,
        right_exit,
        right_result,
        short,
        join,
        selected_result,
        span: _,
    } = expression;
    map_path_condition(mapper, site, condition)?;
    map_storage(mapper, site, result)?;
    map_value(mapper, site, left_result)?;
    map_block(mapper, site, split)?;
    map_block(mapper, site, selection)?;
    map_block(mapper, site, right_entry)?;
    map_block(mapper, site, right_exit)?;
    map_value(mapper, site, right_result)?;
    map_block(mapper, site, short)?;
    map_block(mapper, site, join)?;
    map_value(mapper, site, selected_result)
}

fn map_instruction<M: MirLocalIdentityMapper>(
    instruction: &mut MirInstruction,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    match instruction {
        MirInstruction::StorageLive(instruction) => {
            let MirStorageLive { storage, span: _ } = instruction;
            map_storage(mapper, site, storage)
        }
        MirInstruction::StorageDead(instruction) => {
            let MirStorageDead { storage, span: _ } = instruction;
            map_storage(mapper, site, storage)
        }
        MirInstruction::Assign(instruction) => map_assignment(instruction, mapper, site),
        MirInstruction::Call(instruction) => map_call(instruction, mapper, site),
        MirInstruction::Cleanup(instruction) => map_cleanup(instruction, mapper, site),
        MirInstruction::Initialize(instruction) => map_initialize(instruction, mapper, site),
        MirInstruction::Store(instruction) => map_store(instruction, mapper, site),
        MirInstruction::CopyConstruct(instruction) => {
            map_copy_construction(instruction, mapper, site)
        }
        MirInstruction::CopyAssign(instruction) => map_copy_assignment(instruction, mapper, site),
        MirInstruction::EndFullExpression(instruction) => {
            let MirEndFullExpression {
                temporaries,
                span: _,
            } = instruction;
            for cleanup in temporaries {
                map_cleanup(cleanup, mapper, site)?;
            }
            Ok(())
        }
        MirInstruction::BindCheckedView(instruction) => {
            map_checked_view_binding(instruction, mapper, site)
        }
        MirInstruction::EndCheckedView(instruction) => {
            let MirCheckedViewEnd { carrier, span: _ } = instruction;
            map_storage(mapper, site, carrier)
        }
        MirInstruction::SharedAllocate(instruction) => {
            map_shared_allocate(instruction, mapper, site)
        }
        MirInstruction::SharedInitialize(instruction) => {
            map_shared_initialize(instruction, mapper, site)
        }
        MirInstruction::SharedPublish(instruction) => {
            let MirSharedPublish {
                allocation,
                span: _,
            } = instruction;
            map_storage(mapper, site, allocation)
        }
        MirInstruction::SharedStatic(instruction) => {
            let MirSharedStatic {
                destination,
                data: _,
                target: _,
                origin: _,
                span: _,
            } = instruction;
            map_storage(mapper, site, destination)
        }
        MirInstruction::SharedAdopt(instruction) => {
            let MirSharedAdopt {
                destination,
                allocation,
                span: _,
            } = instruction;
            map_storage(mapper, site, destination)?;
            map_storage(mapper, site, allocation)
        }
        MirInstruction::SharedCopy(instruction) => {
            let MirSharedCopy {
                destination,
                source,
                span: _,
            } = instruction;
            map_storage(mapper, site, destination)?;
            map_storage(mapper, site, source)
        }
        MirInstruction::SharedFieldCopy(instruction) => {
            let MirSharedFieldCopy {
                destination,
                source,
                span: _,
            } = instruction;
            map_storage(mapper, site, destination)?;
            map_place(source, mapper, site)
        }
        MirInstruction::SharedCast(instruction) => map_shared_cast(instruction, mapper, site),
        MirInstruction::SharedMove(instruction) => {
            let MirSharedMove {
                destination,
                source,
                span: _,
            } = instruction;
            map_storage(mapper, site, destination)?;
            map_storage(mapper, site, source)
        }
        MirInstruction::SharedRelease(instruction) => {
            let MirSharedRelease { owner, span: _ } = instruction;
            map_storage(mapper, site, owner)
        }
        MirInstruction::SharedFieldInitialize(instruction) => {
            let MirSharedFieldInitialize {
                destination,
                source,
                span: _,
            } = instruction;
            map_place(destination, mapper, site)?;
            map_storage(mapper, site, source)
        }
        MirInstruction::SharedFieldReplace(instruction) => {
            let MirSharedFieldReplace {
                destination,
                source,
                authorization: _,
                final_authorization: _,
                span: _,
            } = instruction;
            map_place(destination, mapper, site)?;
            map_storage(mapper, site, source)
        }
        MirInstruction::StringInitialize(instruction) => {
            map_string_initialize(instruction, mapper, site)
        }
        MirInstruction::OptionalInitialize(instruction) => {
            map_optional_initialize(instruction, mapper, site)
        }
        MirInstruction::OptionalAssign(instruction) => {
            map_optional_assign(instruction, mapper, site)
        }
        MirInstruction::AggregateOptionalInitialize(instruction) => {
            map_aggregate_optional_initialize(instruction, mapper, site)
        }
        MirInstruction::AggregateOptionalAssign(instruction) => {
            map_aggregate_optional_assign(instruction, mapper, site)
        }
        MirInstruction::AggregateOptionalPublish(instruction) => {
            let MirAggregateOptionalPublish {
                optional: _,
                destination,
                span: _,
            } = instruction;
            map_place(destination, mapper, site)
        }
        MirInstruction::AggregateOptionalCleanup(instruction) => {
            let MirAggregateOptionalCleanup {
                optional: _,
                destination,
                span: _,
            } = instruction;
            map_place(destination, mapper, site)
        }
        MirInstruction::ClassOptionalInitialize(instruction) => {
            map_class_optional_initialize(instruction, mapper, site)
        }
        MirInstruction::ClassOptionalAssign(instruction) => {
            map_class_optional_assign(instruction, mapper, site)
        }
        MirInstruction::ClassOptionalPublish(instruction) => {
            let MirClassOptionalPublish {
                optional: _,
                destination,
                class: _,
                span: _,
            } = instruction;
            map_place(destination, mapper, site)
        }
        MirInstruction::ClassOptionalCleanup(instruction) => {
            let MirClassOptionalCleanup {
                optional: _,
                destination,
                class: _,
                span: _,
            } = instruction;
            map_place(destination, mapper, site)
        }
        MirInstruction::EndOptionalView(instruction) => {
            map_optional_view_end(instruction, mapper, site)
        }
        MirInstruction::EndOptionalBoxView(instruction) => {
            map_optional_box_view_end(instruction, mapper, site)
        }
        MirInstruction::OptionalSharedInitialize(instruction) => {
            map_optional_shared_initialize(instruction, mapper, site)
        }
        MirInstruction::OptionalSharedAssign(instruction) => {
            map_optional_shared_assign(instruction, mapper, site)
        }
        MirInstruction::OptionalSharedCleanup(instruction) => {
            let MirOptionalSharedCleanup {
                optional: _,
                destination,
                target: _,
                span: _,
            } = instruction;
            map_place(destination, mapper, site)
        }
        MirInstruction::Array(instruction) => map_array_instruction(instruction, mapper, site),
        MirInstruction::Io(instruction) => map_io_instruction(instruction, mapper, site),
    }
}

fn map_assignment<M: MirLocalIdentityMapper>(
    instruction: &mut MirAssignment,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirAssignment {
        result,
        rvalue,
        span: _,
    } = instruction;
    map_value(mapper, site, result)?;
    map_rvalue(rvalue, mapper, site)
}

fn map_rvalue<M: MirLocalIdentityMapper>(
    rvalue: &mut MirRvalue,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirRvalue { kind, ty: _ } = rvalue;
    match kind {
        MirRvalueKind::ConstantI64(_)
        | MirRvalueKind::ConstantU64(_)
        | MirRvalueKind::ConstantU8(_)
        | MirRvalueKind::ConstantF64Bits(_)
        | MirRvalueKind::ConstantBool(_)
        | MirRvalueKind::CallableAddress(_) => Ok(()),
        MirRvalueKind::PathCondition(condition) => {
            let MirPathConditionValue {
                condition,
                activation,
            } = condition;
            map_path_condition(mapper, site, condition)?;
            map_storage(mapper, site, activation)
        }
        MirRvalueKind::Load(place) => map_place(place, mapper, site),
        MirRvalueKind::Unary {
            operation: _,
            operand,
        }
        | MirRvalueKind::PrimitiveCast {
            operation: _,
            operand,
        }
        | MirRvalueKind::CheckedF64ToInteger {
            relation: _,
            operand,
        } => map_value(mapper, site, operand),
        MirRvalueKind::Binary {
            operation: _,
            left,
            right,
        }
        | MirRvalueKind::IntegerDivision {
            operation: _,
            dividend: left,
            divisor: right,
        }
        | MirRvalueKind::Shift {
            operation: _,
            left,
            count: right,
        }
        | MirRvalueKind::PrimitiveComparison {
            operation: _,
            left,
            right,
        } => {
            map_value(mapper, site, left)?;
            map_value(mapper, site, right)
        }
        MirRvalueKind::TypeTest { source, target: _ } => map_object_view(source, mapper, site),
        MirRvalueKind::OptionalPresence { source, kind: _ } => map_place(source, mapper, site),
        MirRvalueKind::OptionalBoxPresence {
            owner,
            target: _,
            layer: _,
            kind: _,
        } => map_storage(mapper, site, owner),
        MirRvalueKind::ArrayLength { source, array: _ } => map_place(source, mapper, site),
    }
}

fn map_call<M: MirLocalIdentityMapper>(
    call: &mut MirCall,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirCall {
        target,
        receiver,
        arguments,
        result,
        shared_result,
        destination,
        span: _,
    } = call;
    match target {
        MirCallTarget::Direct(_)
        | MirCallTarget::Static(_)
        | MirCallTarget::Method(_)
        | MirCallTarget::Interface(_) => {}
        MirCallTarget::Indirect(target) => {
            let MirIndirectCallTarget {
                callee,
                function_type: _,
            } = target;
            map_value(mapper, site, callee)?;
        }
    }
    if let Some(receiver) = receiver {
        match receiver {
            MirCallReceiver::Method(receiver) => map_method_receiver(receiver, mapper, site)?,
            MirCallReceiver::Interface(view) => map_object_view(view, mapper, site)?,
        }
    }
    for argument in arguments {
        map_argument(argument, mapper, site)?;
    }
    if let Some(result) = result {
        map_value(mapper, site, result)?;
    }
    map_optional_storage(mapper, site, shared_result)?;
    if let Some(destination) = destination {
        map_place(destination, mapper, site)?;
    }
    Ok(())
}

fn map_argument<M: MirLocalIdentityMapper>(
    argument: &mut MirArgument,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    match argument {
        MirArgument::Value(value) => map_value(mapper, site, value),
        MirArgument::Place(place) | MirArgument::OwnedPlace(place) => {
            map_place(place, mapper, site)
        }
        MirArgument::View(view) => map_object_view(view, mapper, site),
        MirArgument::SharedOwner(owner) => map_storage(mapper, site, owner),
    }
}

fn map_cleanup<M: MirLocalIdentityMapper>(
    cleanup: &mut MirCleanup,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirCleanup {
        destination,
        target: _,
        span: _,
    } = cleanup;
    map_place(destination, mapper, site)
}

fn map_initialize<M: MirLocalIdentityMapper>(
    instruction: &mut MirInitialize,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirInitialize {
        destination,
        target: _,
        arguments,
        span: _,
    } = instruction;
    map_place(destination, mapper, site)?;
    for argument in arguments {
        map_argument(argument, mapper, site)?;
    }
    Ok(())
}

fn map_store<M: MirLocalIdentityMapper>(
    instruction: &mut MirStore,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirStore {
        destination,
        value,
        authorization: _,
        final_authorization: _,
        span: _,
    } = instruction;
    map_place(destination, mapper, site)?;
    map_value(mapper, site, value)
}

fn map_copy_construction<M: MirLocalIdentityMapper>(
    instruction: &mut MirCopyConstruction,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirCopyConstruction {
        destination,
        source,
        class: _,
        operation: _,
        span: _,
    } = instruction;
    map_place(destination, mapper, site)?;
    map_place(source, mapper, site)
}

fn map_copy_assignment<M: MirLocalIdentityMapper>(
    instruction: &mut MirCopyAssignment,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirCopyAssignment {
        destination,
        source,
        class: _,
        operation: _,
        authorization: _,
        final_authorization: _,
        span: _,
    } = instruction;
    map_place(destination, mapper, site)?;
    map_place(source, mapper, site)
}

fn map_checked_view_binding<M: MirLocalIdentityMapper>(
    binding: &mut MirCheckedViewBinding,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirCheckedViewBinding {
        destination,
        view,
        span: _,
    } = binding;
    map_storage(mapper, site, destination)?;
    map_object_view(view, mapper, site)
}

fn map_shared_allocate<M: MirLocalIdentityMapper>(
    instruction: &mut MirSharedAllocate,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirSharedAllocate {
        allocation,
        target: _,
        origin: _,
        mode,
        span: _,
    } = instruction;
    map_storage(mapper, site, allocation)?;
    match mode {
        MirSharedAllocationMode::Initialize => Ok(()),
        MirSharedAllocationMode::Copy { source } => map_place(source, mapper, site),
        MirSharedAllocationMode::OptionalBox { completion: _ } => Ok(()),
    }
}

fn map_shared_initialize<M: MirLocalIdentityMapper>(
    instruction: &mut MirSharedInitialize,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirSharedInitialize {
        allocation,
        target: _,
        arguments,
        span: _,
    } = instruction;
    map_storage(mapper, site, allocation)?;
    for argument in arguments {
        map_argument(argument, mapper, site)?;
    }
    Ok(())
}

fn map_shared_cast<M: MirLocalIdentityMapper>(
    cast: &mut MirSharedCast,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirSharedCast {
        destination,
        source,
        target: _,
        transfer: _,
        exact_dynamic_class: _,
        span: _,
    } = cast;
    map_storage(mapper, site, destination)?;
    match source {
        MirSharedCastSource::Owner { storage, target: _ } => map_storage(mapper, site, storage),
        MirSharedCastSource::Field { place, target: _ } => map_place(place, mapper, site),
    }
}

fn map_string_initialize<M: MirLocalIdentityMapper>(
    instruction: &mut MirStringInitialize,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirStringInitialize {
        destination,
        data: _,
        backing,
        class: _,
        storage_field: _,
        start_field: _,
        length_field: _,
        hash_code_field: _,
        start: _,
        length: _,
        span: _,
    } = instruction;
    map_place(destination, mapper, site)?;
    map_storage(mapper, site, backing)
}

fn map_optional_source<M: MirLocalIdentityMapper>(
    source: &mut MirOptionalSource,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    match source {
        MirOptionalSource::Absent => Ok(()),
        MirOptionalSource::Present(value) => map_value(mapper, site, value),
        MirOptionalSource::Copy(place) => map_place(place, mapper, site),
    }
}

fn map_optional_initialize<M: MirLocalIdentityMapper>(
    instruction: &mut MirOptionalInitialize,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirOptionalInitialize {
        destination,
        source,
        span: _,
    } = instruction;
    map_place(destination, mapper, site)?;
    map_optional_source(source, mapper, site)
}

fn map_optional_assign<M: MirLocalIdentityMapper>(
    instruction: &mut MirOptionalAssign,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirOptionalAssign {
        destination,
        source,
        authorization: _,
        final_authorization: _,
        span: _,
    } = instruction;
    map_place(destination, mapper, site)?;
    map_optional_source(source, mapper, site)
}

fn map_aggregate_optional_source<M: MirLocalIdentityMapper>(
    source: &mut MirAggregateOptionalSource,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    match source {
        MirAggregateOptionalSource::Absent | MirAggregateOptionalSource::Unpublished => Ok(()),
        MirAggregateOptionalSource::Copy(place) => map_place(place, mapper, site),
    }
}

fn map_aggregate_optional_initialize<M: MirLocalIdentityMapper>(
    instruction: &mut MirAggregateOptionalInitialize,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirAggregateOptionalInitialize {
        optional: _,
        destination,
        source,
        span: _,
    } = instruction;
    map_place(destination, mapper, site)?;
    map_aggregate_optional_source(source, mapper, site)
}

fn map_aggregate_optional_assign<M: MirLocalIdentityMapper>(
    instruction: &mut MirAggregateOptionalAssign,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirAggregateOptionalAssign {
        optional: _,
        destination,
        source,
        authorization: _,
        final_authorization: _,
        span: _,
    } = instruction;
    map_place(destination, mapper, site)?;
    map_aggregate_optional_source(source, mapper, site)
}

fn map_class_optional_source<M: MirLocalIdentityMapper>(
    source: &mut MirClassOptionalSource,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    match source {
        MirClassOptionalSource::Absent => Ok(()),
        MirClassOptionalSource::Present(place) | MirClassOptionalSource::Copy(place) => {
            map_place(place, mapper, site)
        }
    }
}

fn map_class_optional_initialize<M: MirLocalIdentityMapper>(
    instruction: &mut MirClassOptionalInitialize,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirClassOptionalInitialize {
        optional: _,
        destination,
        source,
        class: _,
        copy_constructor: _,
        span: _,
    } = instruction;
    map_place(destination, mapper, site)?;
    map_class_optional_source(source, mapper, site)
}

fn map_class_optional_assign<M: MirLocalIdentityMapper>(
    instruction: &mut MirClassOptionalAssign,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirClassOptionalAssign {
        optional: _,
        destination,
        source,
        class: _,
        copy_constructor: _,
        copy_assignment: _,
        authorization: _,
        final_authorization: _,
        span: _,
    } = instruction;
    map_place(destination, mapper, site)?;
    map_class_optional_source(source, mapper, site)
}

fn map_optional_view_begin<M: MirLocalIdentityMapper>(
    begin: &mut MirOptionalViewBegin,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirOptionalViewBegin {
        optional: _,
        guard,
        source,
        payload: _,
        span: _,
    } = begin;
    map_optional_guard(mapper, site, guard)?;
    map_place(source, mapper, site)
}

fn map_optional_view_end<M: MirLocalIdentityMapper>(
    end: &mut MirOptionalViewEnd,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirOptionalViewEnd {
        optional: _,
        guard,
        source,
        payload: _,
        span: _,
    } = end;
    map_optional_guard(mapper, site, guard)?;
    map_place(source, mapper, site)
}

fn map_optional_box_view_begin<M: MirLocalIdentityMapper>(
    begin: &mut MirOptionalBoxViewBegin,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirOptionalBoxViewBegin {
        box_target: _,
        layer: _,
        guard,
        owner,
        span: _,
    } = begin;
    map_optional_guard(mapper, site, guard)?;
    map_storage(mapper, site, owner)
}

fn map_optional_box_view_end<M: MirLocalIdentityMapper>(
    end: &mut MirOptionalBoxViewEnd,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirOptionalBoxViewEnd {
        box_target: _,
        layer: _,
        guard,
        owner,
        span: _,
    } = end;
    map_optional_guard(mapper, site, guard)?;
    map_storage(mapper, site, owner)
}

fn map_optional_shared_source<M: MirLocalIdentityMapper>(
    source: &mut MirOptionalSharedSource,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    match source {
        MirOptionalSharedSource::Absent => Ok(()),
        MirOptionalSharedSource::Present(owner) | MirOptionalSharedSource::Move(owner) => {
            map_storage(mapper, site, owner)
        }
        MirOptionalSharedSource::Copy(place) => map_place(place, mapper, site),
    }
}

fn map_optional_shared_initialize<M: MirLocalIdentityMapper>(
    instruction: &mut MirOptionalSharedInitialize,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirOptionalSharedInitialize {
        optional: _,
        destination,
        source,
        target: _,
        span: _,
    } = instruction;
    map_place(destination, mapper, site)?;
    map_optional_shared_source(source, mapper, site)
}

fn map_optional_shared_assign<M: MirLocalIdentityMapper>(
    instruction: &mut MirOptionalSharedAssign,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirOptionalSharedAssign {
        optional: _,
        destination,
        source,
        target: _,
        authorization: _,
        final_authorization: _,
        span: _,
    } = instruction;
    map_place(destination, mapper, site)?;
    map_optional_shared_source(source, mapper, site)
}

fn map_io_instruction<M: MirLocalIdentityMapper>(
    instruction: &mut MirIoInstruction,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirIoInstruction {
        result,
        operation,
        span: _,
    } = instruction;
    map_value(mapper, site, result)?;
    match operation {
        MirIoOperation::StandardHandle { stream } => map_value(mapper, site, stream),
        MirIoOperation::Open { path, mode } => {
            map_io_buffer(path, mapper, site)?;
            map_value(mapper, site, mode)
        }
        MirIoOperation::Read {
            handle,
            destination,
            offset,
        } => {
            map_value(mapper, site, handle)?;
            map_io_buffer(destination, mapper, site)?;
            map_storage(mapper, site, offset)
        }
        MirIoOperation::Write {
            handle,
            source,
            offset,
        } => {
            map_value(mapper, site, handle)?;
            map_io_buffer(source, mapper, site)?;
            map_storage(mapper, site, offset)
        }
        MirIoOperation::Close { handle } => map_value(mapper, site, handle),
    }
}

fn map_io_buffer<M: MirLocalIdentityMapper>(
    buffer: &mut MirIoBuffer,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirIoBuffer {
        place,
        anchor,
        array: _,
        access: _,
    } = buffer;
    map_place(place, mapper, site)?;
    map_storage(mapper, site, anchor)
}

fn map_array_instruction<M: MirLocalIdentityMapper>(
    instruction: &mut MirArrayInstruction,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    match instruction {
        MirArrayInstruction::Allocate {
            backing,
            array: _,
            length,
            ownership: _,
            failure: _,
            span: _,
        } => {
            map_storage(mapper, site, backing)?;
            map_value(mapper, site, length)
        }
        MirArrayInstruction::AllocateElements {
            backing,
            prefix,
            array: _,
            length: _,
            ownership: _,
            failure: _,
            span: _,
        }
        | MirArrayInstruction::CompleteElement {
            backing,
            prefix,
            position: _,
            span: _,
        } => {
            map_storage(mapper, site, backing)?;
            map_storage(mapper, site, prefix)
        }
        MirArrayInstruction::InitializeElement {
            backing,
            prefix,
            position: _,
            value,
            span: _,
        } => {
            map_storage(mapper, site, backing)?;
            map_storage(mapper, site, prefix)?;
            map_value(mapper, site, value)
        }
        MirArrayInstruction::InitializeNext {
            backing,
            index,
            operation: _,
            span: _,
        } => {
            map_storage(mapper, site, backing)?;
            map_storage(mapper, site, index)
        }
        MirArrayInstruction::CopyNext {
            backing,
            source,
            index,
            operation: _,
            span: _,
        } => {
            map_storage(mapper, site, backing)?;
            map_place(source, mapper, site)?;
            map_storage(mapper, site, index)
        }
        MirArrayInstruction::Publish {
            backing,
            destination,
            span: _,
        }
        | MirArrayInstruction::PublishShared {
            backing,
            destination,
            array: _,
            span: _,
        } => {
            map_storage(mapper, site, backing)?;
            map_storage(mapper, site, destination)
        }
        MirArrayInstruction::Adopt {
            destination,
            source,
            array: _,
            span: _,
        } => {
            map_place(destination, mapper, site)?;
            map_storage(mapper, site, source)
        }
        MirArrayInstruction::Replace {
            destination,
            source,
            array: _,
            authorization: _,
            final_authorization: _,
            span: _,
        } => {
            map_place(destination, mapper, site)?;
            map_storage(mapper, site, source)
        }
        MirArrayInstruction::ElementAssign {
            destination,
            source,
            operation: _,
            span: _,
        } => {
            map_place(destination, mapper, site)?;
            map_place(source, mapper, site)
        }
        MirArrayInstruction::DestroyNext {
            owner,
            index,
            operation: _,
            span: _,
        } => {
            map_place(owner, mapper, site)?;
            map_storage(mapper, site, index)
        }
        MirArrayInstruction::Release {
            owner,
            array: _,
            span: _,
        } => map_place(owner, mapper, site),
        MirArrayInstruction::AnchorBegin {
            anchor,
            owner,
            array: _,
            kind: _,
            span: _,
        } => {
            map_storage(mapper, site, anchor)?;
            map_place(owner, mapper, site)
        }
        MirArrayInstruction::AnchorEnd { anchor, span: _ } => map_storage(mapper, site, anchor),
        MirArrayInstruction::AliasBind {
            alias,
            source,
            anchor,
            span: _,
        } => {
            map_storage(mapper, site, alias)?;
            map_place(source, mapper, site)?;
            map_storage(mapper, site, anchor)
        }
        MirArrayInstruction::Normalize {
            destination,
            owner,
            index,
            array: _,
            kind: _,
            span: _,
        } => {
            map_storage(mapper, site, destination)?;
            map_place(owner, mapper, site)?;
            map_value(mapper, site, index)
        }
        MirArrayInstruction::Offset {
            destination,
            owner,
            offset,
            array: _,
            span: _,
        } => {
            map_storage(mapper, site, destination)?;
            map_place(owner, mapper, site)?;
            map_value(mapper, site, offset)
        }
        MirArrayInstruction::Boundary {
            destination,
            owner,
            array: _,
            boundary: _,
            span: _,
        } => {
            map_storage(mapper, site, destination)?;
            map_place(owner, mapper, site)
        }
        MirArrayInstruction::SliceCopy {
            destination,
            source,
            start,
            end,
            array: _,
            operation: _,
            span: _,
        } => {
            map_storage(mapper, site, destination)?;
            map_place(source, mapper, site)?;
            map_storage(mapper, site, start)?;
            map_storage(mapper, site, end)
        }
        MirArrayInstruction::SliceLengthCheck {
            destination_start,
            destination_end,
            source,
            array: _,
            span: _,
        } => {
            map_storage(mapper, site, destination_start)?;
            map_storage(mapper, site, destination_end)?;
            map_place(source, mapper, site)
        }
        MirArrayInstruction::SliceBoundsCheck {
            start,
            end,
            array: _,
            span: _,
        } => {
            map_storage(mapper, site, start)?;
            map_storage(mapper, site, end)
        }
        MirArrayInstruction::SliceAssignNext {
            destination,
            source,
            destination_index,
            source_index,
            operation: _,
            span: _,
        } => {
            map_place(destination, mapper, site)?;
            map_place(source, mapper, site)?;
            map_storage(mapper, site, destination_index)?;
            map_storage(mapper, site, source_index)
        }
    }
}

fn map_terminator<M: MirLocalIdentityMapper>(
    terminator: &mut MirTerminator,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    match terminator {
        MirTerminator::Return { value, span: _ } => {
            if let Some(value) = value {
                map_value(mapper, site, value)?;
            }
            Ok(())
        }
        MirTerminator::ReturnShared { owner, span: _ }
        | MirTerminator::ReturnOptionalShared { owner, span: _ } => {
            map_storage(mapper, site, owner)
        }
        MirTerminator::Panic { message, span: _ } => map_place(message, mapper, site),
        MirTerminator::Goto { target, span: _ } => map_block(mapper, site, target),
        MirTerminator::Branch {
            condition,
            true_target,
            false_target,
            span: _,
        } => {
            map_value(mapper, site, condition)?;
            map_block(mapper, site, true_target)?;
            map_block(mapper, site, false_target)
        }
        MirTerminator::ShiftCountCheck {
            check,
            success_target,
            failure_target,
            span: _,
        } => {
            let MirShiftCountCheck {
                operation: _,
                left,
                count,
                result,
            } = check;
            map_storage(mapper, site, left)?;
            map_storage(mapper, site, count)?;
            map_storage(mapper, site, result)?;
            map_block_pair(mapper, site, success_target, failure_target)
        }
        MirTerminator::IntegerDivisorCheck {
            check,
            success_target,
            failure_target,
            span: _,
        } => {
            let MirIntegerDivisorCheck {
                operation: _,
                dividend,
                divisor,
                result,
            } = check;
            map_storage(mapper, site, dividend)?;
            map_storage(mapper, site, divisor)?;
            map_storage(mapper, site, result)?;
            map_block_pair(mapper, site, success_target, failure_target)
        }
        MirTerminator::PrimitiveCastRangeCheck {
            check,
            success_target,
            failure_target,
            span: _,
        } => {
            let MirPrimitiveCastRangeCheck {
                relation: _,
                source,
                result,
            } = check;
            map_storage(mapper, site, source)?;
            map_storage(mapper, site, result)?;
            map_block_pair(mapper, site, success_target, failure_target)
        }
        MirTerminator::CheckedCast {
            binding,
            success_target,
            failure_target,
            span: _,
        } => {
            map_checked_view_binding(binding, mapper, site)?;
            map_block_pair(mapper, site, success_target, failure_target)
        }
        MirTerminator::SharedCast {
            cast,
            success_target,
            failure_target,
            span: _,
        } => {
            map_shared_cast(cast, mapper, site)?;
            map_block_pair(mapper, site, success_target, failure_target)
        }
        MirTerminator::OptionalUnwrap {
            source,
            destination,
            success_target,
            failure_target,
            span: _,
        } => {
            map_place(source, mapper, site)?;
            map_storage(mapper, site, destination)?;
            map_block_pair(mapper, site, success_target, failure_target)
        }
        MirTerminator::OptionalSharedUnwrap {
            unwrap,
            success_target,
            failure_target,
            span: _,
        } => {
            let MirOptionalSharedUnwrap {
                optional: _,
                source,
                destination,
                target: _,
                span: _,
            } = unwrap;
            map_place(source, mapper, site)?;
            map_storage(mapper, site, destination)?;
            map_block_pair(mapper, site, success_target, failure_target)
        }
        MirTerminator::BeginOptionalView {
            begin,
            success_target,
            absent_target,
            overflow_target,
            span: _,
        } => {
            map_optional_view_begin(begin, mapper, site)?;
            map_block_triple(mapper, site, success_target, absent_target, overflow_target)
        }
        MirTerminator::BeginOptionalBoxView {
            begin,
            success_target,
            absent_target,
            overflow_target,
            span: _,
        } => {
            map_optional_box_view_begin(begin, mapper, site)?;
            map_block_triple(mapper, site, success_target, absent_target, overflow_target)
        }
        MirTerminator::CheckOptionalMutation {
            source,
            success_target,
            failure_target,
            span: _,
        } => {
            map_place(source, mapper, site)?;
            map_block_pair(mapper, site, success_target, failure_target)
        }
        MirTerminator::ArrayPositionCheck {
            position,
            kind: _,
            success_target,
            failure_target,
            span: _,
        } => {
            map_storage(mapper, site, position)?;
            map_block_pair(mapper, site, success_target, failure_target)
        }
        MirTerminator::ArrayOperationCheck {
            failure: _,
            success_target,
            failure_target,
            span: _,
        } => map_block_pair(mapper, site, success_target, failure_target),
        MirTerminator::ArrayLoop {
            backing,
            index,
            length,
            body_target,
            complete_target,
            span: _,
        } => {
            map_storage(mapper, site, backing)?;
            map_storage(mapper, site, index)?;
            map_storage(mapper, site, length)?;
            map_block_pair(mapper, site, body_target, complete_target)
        }
        MirTerminator::Terminate { reason: _, span: _ } => Ok(()),
    }
}

fn map_block_pair<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    first: &mut BlockId,
    second: &mut BlockId,
) -> Result<(), M::Error> {
    map_block(mapper, site, first)?;
    map_block(mapper, site, second)
}

fn map_block_triple<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    first: &mut BlockId,
    second: &mut BlockId,
    third: &mut BlockId,
) -> Result<(), M::Error> {
    map_block(mapper, site, first)?;
    map_block(mapper, site, second)?;
    map_block(mapper, site, third)
}

fn map_place<M: MirLocalIdentityMapper>(
    place: &mut MirPlace,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirPlace { base, projections } = place;
    match base {
        MirPlaceBase::StaticField(_) | MirPlaceBase::StaticLifecycleDestination(_) => {}
        MirPlaceBase::Storage(storage)
        | MirPlaceBase::AliasParameter(storage)
        | MirPlaceBase::CheckedView(storage)
        | MirPlaceBase::ArrayAlias(storage)
        | MirPlaceBase::SharedPointee(storage)
        | MirPlaceBase::SharedAllocationPayload(storage) => {
            map_storage(mapper, site, storage)?;
        }
        MirPlaceBase::OptionalBoxPayload { owner, target: _ } => {
            map_storage(mapper, site, owner)?;
        }
    }
    for projection in projections {
        match projection {
            MirPlaceProjection::Base(_)
            | MirPlaceProjection::Field(_)
            | MirPlaceProjection::OptionalPayload(_)
            | MirPlaceProjection::AggregateOptionalPayload(_)
            | MirPlaceProjection::CheckedOptionalPayload(_) => {}
            MirPlaceProjection::ArrayElement {
                array: _,
                normalized_index,
            } => map_storage(mapper, site, normalized_index)?,
        }
    }
    Ok(())
}

fn map_object_view<M: MirLocalIdentityMapper>(
    view: &mut MirObjectView,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirObjectView {
        source,
        origin,
        target: _,
        access: _,
        provenance: _,
        span: _,
    } = view;
    map_place(source, mapper, site)?;
    map_object_origin(origin, mapper, site)
}

fn map_method_receiver<M: MirLocalIdentityMapper>(
    receiver: &mut MirMethodReceiver,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirMethodReceiver {
        place,
        origin,
        access: _,
        provenance: _,
    } = receiver;
    map_place(place, mapper, site)?;
    map_object_origin(origin, mapper, site)
}

fn map_object_origin<M: MirLocalIdentityMapper>(
    origin: &mut MirObjectOrigin,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    match origin {
        MirObjectOrigin::Exact {
            complete,
            dynamic_class: _,
        } => map_place(complete, mapper, site),
        MirObjectOrigin::Forwarded {
            carrier,
            static_target: _,
            access: _,
            dispatch_limit: _,
            span: _,
        } => map_storage(mapper, site, carrier),
        MirObjectOrigin::Shared {
            owner,
            static_target: _,
            access: _,
            exact_dynamic_class: _,
            span: _,
        } => map_storage(mapper, site, owner),
    }
}

fn map_optional_storage<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    identity: &mut Option<StorageId>,
) -> Result<(), M::Error> {
    if let Some(identity) = identity {
        map_storage(mapper, site, identity)?;
    }
    Ok(())
}

fn map_storage<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    identity: &mut StorageId,
) -> Result<(), M::Error> {
    *identity = mapper.map_storage(site, *identity)?;
    Ok(())
}

fn map_value<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    identity: &mut ValueId,
) -> Result<(), M::Error> {
    *identity = mapper.map_value(site, *identity)?;
    Ok(())
}

fn map_block<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    identity: &mut BlockId,
) -> Result<(), M::Error> {
    *identity = mapper.map_block(site, *identity)?;
    Ok(())
}

fn map_path_condition<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    identity: &mut PathConditionId,
) -> Result<(), M::Error> {
    *identity = mapper.map_path_condition(site, *identity)?;
    Ok(())
}

fn map_optional_guard<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    identity: &mut OptionalGuardId,
) -> Result<(), M::Error> {
    *identity = mapper.map_optional_guard(site, *identity)?;
    Ok(())
}

#[cfg(test)]
pub(super) fn preserve_function_local_identities(
    definition: &mut MirFunctionDefinition,
) -> Result<(), std::convert::Infallible> {
    map_function_local_identities(definition, &mut PreserveLocalIdentities)
}
