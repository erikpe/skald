//! Exhaustive dispatch across executable MIR instruction variants.

mod array;
mod io;
mod operation;
mod optional;

pub(super) use {
    array::define_array_instruction_traversal, io::define_io_instruction_traversal,
    operation::define_core_operation_traversal, optional::define_optional_instruction_traversal,
};

macro_rules! define_instruction_dispatch {
    (($($mir_mutability:tt)*)) => {
        pub(crate) fn map_instruction<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirInstruction,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            match instruction {
                MirInstruction::StorageLive(instruction) => {
                    let MirStorageLive { storage, span: _ } = instruction;
                    map_storage_use(mapper, site, MirStorageUseRole::LifetimeLive, storage)
                }
                MirInstruction::StorageDead(instruction) => {
                    let MirStorageDead { storage, span: _ } = instruction;
                    map_storage_use(mapper, site, MirStorageUseRole::LifetimeDead, storage)
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
                    map_storage_use(mapper, site, MirStorageUseRole::Alias, carrier)
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
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        allocation,
                    )
                }
                MirInstruction::SharedStatic(instruction) => {
                    let MirSharedStatic {
                        destination,
                        data: _,
                        target: _,
                        origin: _,
                        span: _,
                    } = instruction;
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        destination,
                    )
                }
                MirInstruction::SharedAdopt(instruction) => {
                    let MirSharedAdopt {
                        destination,
                        allocation,
                        span: _,
                    } = instruction;
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        destination,
                    )?;
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        allocation,
                    )
                }
                MirInstruction::SharedCopy(instruction) => {
                    let MirSharedCopy {
                        destination,
                        source,
                        span: _,
                    } = instruction;
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        destination,
                    )?;
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        source,
                    )
                }
                MirInstruction::SharedFieldCopy(instruction) => {
                    let MirSharedFieldCopy {
                        destination,
                        source,
                        span: _,
                    } = instruction;
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        destination,
                    )?;
                    map_place(source, mapper, site, MirPlaceUseContext::OwnershipOrLifecycle)
                }
                MirInstruction::SharedCast(instruction) => map_shared_cast(instruction, mapper, site),
                MirInstruction::SharedMove(instruction) => {
                    let MirSharedMove {
                        destination,
                        source,
                        span: _,
                    } = instruction;
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        destination,
                    )?;
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        source,
                    )
                }
                MirInstruction::SharedRelease(instruction) => {
                    let MirSharedRelease { owner, span: _ } = instruction;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, owner)
                }
                MirInstruction::SharedFieldInitialize(instruction) => {
                    let MirSharedFieldInitialize {
                        destination,
                        source,
                        span: _,
                    } = instruction;
                    map_place(
                        destination,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, source)
                }
                MirInstruction::SharedFieldReplace(instruction) => {
                    let MirSharedFieldReplace {
                        destination,
                        source,
                        authorization: _,
                        final_authorization: _,
                        span: _,
                    } = instruction;
                    map_place(
                        destination,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, source)
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
                    map_place(
                        destination,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )
                }
                MirInstruction::AggregateOptionalCleanup(instruction) => {
                    let MirAggregateOptionalCleanup {
                        optional: _,
                        destination,
                        span: _,
                    } = instruction;
                    map_place(
                        destination,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )
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
                    map_place(
                        destination,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )
                }
                MirInstruction::ClassOptionalCleanup(instruction) => {
                    let MirClassOptionalCleanup {
                        optional: _,
                        destination,
                        class: _,
                        span: _,
                    } = instruction;
                    map_place(
                        destination,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )
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
                    map_place(
                        destination,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )
                }
                MirInstruction::Array(instruction) => map_array_instruction(instruction, mapper, site),
                MirInstruction::Io(instruction) => map_io_instruction(instruction, mapper, site),
            }
        }

    };
}

pub(super) use define_instruction_dispatch;
