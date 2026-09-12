//! Array construction, ownership, aliasing, indexing, and slicing.

macro_rules! define_array_instruction_traversal {
    (($($mir_mutability:tt)*)) => {
        fn map_array_instruction<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirArrayInstruction,
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
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        backing,
                    )?;
                    map_value_use(
                        mapper,
                        site,
                        MirValueUseRole::OwnershipOrLifecycle,
                        length,
                    )
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
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        backing,
                    )?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, prefix)
                }
                MirArrayInstruction::BeginIndexed {
                    backing,
                    prefix,
                    length,
                    span: _,
                }
                | MirArrayInstruction::EndIndexedElement {
                    backing,
                    prefix,
                    length,
                    span: _,
                }
                | MirArrayInstruction::CompleteIndexed {
                    backing,
                    prefix,
                    length,
                    span: _,
                } => {
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, backing)?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, prefix)?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, length)
                }
                MirArrayInstruction::BindIndexed {
                    backing,
                    prefix,
                    length,
                    binding,
                    span: _,
                } => {
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, backing)?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, prefix)?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, length)?;
                    map_storage_use(mapper, site, MirStorageUseRole::OtherExecutable, binding)
                }
                MirArrayInstruction::InitializeIndexedElement {
                    backing,
                    prefix,
                    value,
                    span: _,
                } => {
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, backing)?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, prefix)?;
                    map_value_use(mapper, site, MirValueUseRole::OwnershipOrLifecycle, value)
                }
                MirArrayInstruction::AdvanceIndexedElement {
                    backing,
                    prefix,
                    span: _,
                } => {
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, backing)?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, prefix)
                }
                MirArrayInstruction::InitializeElement {
                    backing,
                    prefix,
                    position: _,
                    value,
                    span: _,
                } => {
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        backing,
                    )?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, prefix)?;
                    map_value_use(
                        mapper,
                        site,
                        MirValueUseRole::OwnershipOrLifecycle,
                        value,
                    )
                }
                MirArrayInstruction::InitializeNext {
                    backing,
                    index,
                    operation: _,
                    span: _,
                } => {
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        backing,
                    )?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, index)
                }
                MirArrayInstruction::CopyNext {
                    backing,
                    source,
                    index,
                    operation: _,
                    span: _,
                } => {
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        backing,
                    )?;
                    map_place(
                        source,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, index)
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
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        backing,
                    )?;
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        destination,
                    )
                }
                MirArrayInstruction::Adopt {
                    destination,
                    source,
                    array: _,
                    span: _,
                } => {
                    map_place(
                        destination,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, source)
                }
                MirArrayInstruction::Replace {
                    destination,
                    source,
                    array: _,
                    authorization: _,
                    final_authorization: _,
                    span: _,
                } => {
                    map_place(
                        destination,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, source)
                }
                MirArrayInstruction::ElementAssign {
                    destination,
                    source,
                    operation: _,
                    span: _,
                } => {
                    map_place(
                        destination,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )?;
                    map_place(
                        source,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )
                }
                MirArrayInstruction::DestroyNext {
                    owner,
                    index,
                    operation: _,
                    span: _,
                } => {
                    map_place(
                        owner,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, index)
                }
                MirArrayInstruction::Release {
                    owner,
                    array: _,
                    span: _,
                } => map_place(
                    owner,
                    mapper,
                    site,
                    MirPlaceUseContext::OwnershipOrLifecycle,
                ),
                MirArrayInstruction::AnchorBegin {
                    anchor,
                    owner,
                    array: _,
                    kind: _,
                    span: _,
                } => {
                    map_storage_use(mapper, site, MirStorageUseRole::Alias, anchor)?;
                    map_place(owner, mapper, site, MirPlaceUseContext::Alias)
                }
                MirArrayInstruction::AnchorEnd { anchor, span: _ } => {
                    map_storage_use(mapper, site, MirStorageUseRole::Alias, anchor)
                }
                MirArrayInstruction::AliasBind {
                    alias,
                    source,
                    anchor,
                    span: _,
                } => {
                    map_storage_use(mapper, site, MirStorageUseRole::Alias, alias)?;
                    map_place(source, mapper, site, MirPlaceUseContext::Alias)?;
                    map_storage_use(mapper, site, MirStorageUseRole::Alias, anchor)
                }
                MirArrayInstruction::Normalize {
                    destination,
                    owner,
                    index,
                    array: _,
                    kind: _,
                    span: _,
                } => {
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        destination,
                    )?;
                    map_place(
                        owner,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )?;
                    map_value_use(
                        mapper,
                        site,
                        MirValueUseRole::OwnershipOrLifecycle,
                        index,
                    )
                }
                MirArrayInstruction::Offset {
                    destination,
                    owner,
                    offset,
                    array: _,
                    span: _,
                } => {
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        destination,
                    )?;
                    map_place(
                        owner,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )?;
                    map_value_use(
                        mapper,
                        site,
                        MirValueUseRole::OwnershipOrLifecycle,
                        offset,
                    )
                }
                MirArrayInstruction::Boundary {
                    destination,
                    owner,
                    array: _,
                    boundary: _,
                    span: _,
                } => {
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        destination,
                    )?;
                    map_place(
                        owner,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )
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
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        destination,
                    )?;
                    map_place(
                        source,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, start)?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, end)
                }
                MirArrayInstruction::SliceLengthCheck {
                    destination_start,
                    destination_end,
                    source,
                    array: _,
                    span: _,
                } => {
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        destination_start,
                    )?;
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        destination_end,
                    )?;
                    map_place(
                        source,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )
                }
                MirArrayInstruction::SliceBoundsCheck {
                    start,
                    end,
                    array: _,
                    span: _,
                } => {
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, start)?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, end)
                }
                MirArrayInstruction::SliceAssignNext {
                    destination,
                    source,
                    destination_index,
                    source_index,
                    operation: _,
                    span: _,
                } => {
                    map_place(
                        destination,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )?;
                    map_place(
                        source,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )?;
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        destination_index,
                    )?;
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        source_index,
                    )
                }
            }
        }

    };
}

pub(in crate::mir::rewrite::map) use define_array_instruction_traversal;
