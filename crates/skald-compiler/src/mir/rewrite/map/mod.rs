//! Macro-composed traversal of every callable-local identity in executable MIR.
//!
//! The facade owns leaf behavior and assembles private structural fragments
//! inside both generated traversals. A new identity-bearing field belongs in
//! exactly one fragment and must therefore receive the same exhaustive visit
//! position in mutable mapping and read-only observation.

use self::{
    body::define_body_traversal,
    definition::define_definition_traversal,
    instruction::{define_core_operation_traversal, define_instruction_dispatch},
};
use super::super::*;
use super::{
    identity::LocalIdentityOwnerValidator, MirLocalIdentityMapper, MirLocalIdentityObserver,
    MirLocalIdentityOwnershipError, MirLocalIdentitySite,
};

#[cfg(test)]
use super::identity::PreserveLocalIdentities;

mod body;
mod definition;
mod instruction;

macro_rules! map_identity {
    (storage, $mapper:expr, $site:expr, $identity:expr) => {{
        *$identity = $mapper.map_storage($site, *$identity)?;
        Ok(())
    }};
    (storage_use, $mapper:expr, $site:expr, $role:expr, $identity:expr) => {{
        let _ = $role;
        *$identity = $mapper.map_storage($site, *$identity)?;
        Ok(())
    }};
    (value, $mapper:expr, $site:expr, $identity:expr) => {{
        *$identity = $mapper.map_value($site, *$identity)?;
        Ok(())
    }};
    (value_use, $mapper:expr, $site:expr, $role:expr, $identity:expr) => {{
        let _ = $role;
        *$identity = $mapper.map_value($site, *$identity)?;
        Ok(())
    }};
    (value_definition, $mapper:expr, $site:expr, $identity:expr) => {{
        *$identity = $mapper.map_value_definition($site, *$identity)?;
        Ok(())
    }};
    (block, $mapper:expr, $site:expr, $identity:expr) => {{
        *$identity = $mapper.map_block($site, *$identity)?;
        Ok(())
    }};
    (path_condition, $mapper:expr, $site:expr, $identity:expr) => {{
        *$identity = $mapper.map_path_condition($site, *$identity)?;
        Ok(())
    }};
    (optional_guard, $mapper:expr, $site:expr, $identity:expr) => {{
        *$identity = $mapper.map_optional_guard($site, *$identity)?;
        Ok(())
    }};
}

macro_rules! observe_identity {
    (storage, $observer:expr, $site:expr, $identity:expr) => {
        $observer.observe_storage($site, *$identity)
    };
    (storage_use, $observer:expr, $site:expr, $role:expr, $identity:expr) => {
        $observer.observe_storage_use($site, $role, *$identity)
    };
    (value, $observer:expr, $site:expr, $identity:expr) => {
        $observer.observe_value($site, *$identity)
    };
    (value_use, $observer:expr, $site:expr, $role:expr, $identity:expr) => {
        $observer.observe_value_use($site, $role, *$identity)
    };
    (value_definition, $observer:expr, $site:expr, $identity:expr) => {
        $observer.observe_value_definition($site, *$identity)
    };
    (block, $observer:expr, $site:expr, $identity:expr) => {
        $observer.observe_block($site, *$identity)
    };
    (path_condition, $observer:expr, $site:expr, $identity:expr) => {
        $observer.observe_path_condition($site, *$identity)
    };
    (optional_guard, $observer:expr, $site:expr, $identity:expr) => {
        $observer.observe_optional_guard($site, *$identity)
    };
}

/// Defines one traversal over either mutable or shared MIR.
///
/// Private macro fragments divide the structural inventory by MIR
/// responsibility. Every fragment expands here for both mapping and
/// observation, which keeps identity coverage and order single-sourced.
macro_rules! define_identity_traversal {
    ($module:ident, $behavior:ident, ($($mir_mutability:tt)*), $leaf:ident) => {
mod $module {
use super::super::super::*;
use super::super::{MirCallValueUse, MirLocalIdentitySite, MirScalarValueUse, MirStoragePlaceUse, MirStorageUseRole, MirStorageWriteAuthorization, MirValueUseRole, $behavior as MirLocalIdentityMapper};
use super::{
    define_body_traversal, define_core_operation_traversal, define_definition_traversal,
    define_instruction_dispatch,
};

define_definition_traversal!(($($mir_mutability)*));
define_body_traversal!(($($mir_mutability)*));

define_instruction_dispatch!(($($mir_mutability)*));
define_core_operation_traversal!(($($mir_mutability)*));

fn map_optional_source<M: MirLocalIdentityMapper>(
    source: &$($mir_mutability)* MirOptionalSource,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    match source {
        MirOptionalSource::Absent => Ok(()),
        MirOptionalSource::Present(value) => map_value_use(
            mapper,
            site,
            MirValueUseRole::OwnershipOrLifecycle,
            value,
        ),
        MirOptionalSource::Copy(place) => map_place(
            place,
            mapper,
            site,
            MirPlaceUseContext::OwnershipOrLifecycle,
        ),
    }
}

fn map_optional_initialize<M: MirLocalIdentityMapper>(
    instruction: &$($mir_mutability)* MirOptionalInitialize,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirOptionalInitialize {
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
    map_optional_source(source, mapper, site)
}

fn map_optional_assign<M: MirLocalIdentityMapper>(
    instruction: &$($mir_mutability)* MirOptionalAssign,
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
    map_place(
        destination,
        mapper,
        site,
        MirPlaceUseContext::OwnershipOrLifecycle,
    )?;
    map_optional_source(source, mapper, site)
}

fn map_aggregate_optional_source<M: MirLocalIdentityMapper>(
    source: &$($mir_mutability)* MirAggregateOptionalSource,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    match source {
        MirAggregateOptionalSource::Absent | MirAggregateOptionalSource::Unpublished => Ok(()),
        MirAggregateOptionalSource::Copy(place) => map_place(
            place,
            mapper,
            site,
            MirPlaceUseContext::OwnershipOrLifecycle,
        ),
    }
}

fn map_aggregate_optional_initialize<M: MirLocalIdentityMapper>(
    instruction: &$($mir_mutability)* MirAggregateOptionalInitialize,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirAggregateOptionalInitialize {
        optional: _,
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
    map_aggregate_optional_source(source, mapper, site)
}

fn map_aggregate_optional_assign<M: MirLocalIdentityMapper>(
    instruction: &$($mir_mutability)* MirAggregateOptionalAssign,
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
    map_place(
        destination,
        mapper,
        site,
        MirPlaceUseContext::OwnershipOrLifecycle,
    )?;
    map_aggregate_optional_source(source, mapper, site)
}

fn map_class_optional_source<M: MirLocalIdentityMapper>(
    source: &$($mir_mutability)* MirClassOptionalSource,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    match source {
        MirClassOptionalSource::Absent => Ok(()),
        MirClassOptionalSource::Present(place) | MirClassOptionalSource::Copy(place) => {
            map_place(
                place,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )
        }
    }
}

fn map_class_optional_initialize<M: MirLocalIdentityMapper>(
    instruction: &$($mir_mutability)* MirClassOptionalInitialize,
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
    map_place(
        destination,
        mapper,
        site,
        MirPlaceUseContext::OwnershipOrLifecycle,
    )?;
    map_class_optional_source(source, mapper, site)
}

fn map_class_optional_assign<M: MirLocalIdentityMapper>(
    instruction: &$($mir_mutability)* MirClassOptionalAssign,
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
    map_place(
        destination,
        mapper,
        site,
        MirPlaceUseContext::OwnershipOrLifecycle,
    )?;
    map_class_optional_source(source, mapper, site)
}

fn map_optional_view_begin<M: MirLocalIdentityMapper>(
    begin: &$($mir_mutability)* MirOptionalViewBegin,
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
    map_place(
        source,
        mapper,
        site,
        MirPlaceUseContext::OwnershipOrLifecycle,
    )
}

fn map_optional_view_end<M: MirLocalIdentityMapper>(
    end: &$($mir_mutability)* MirOptionalViewEnd,
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
    map_place(
        source,
        mapper,
        site,
        MirPlaceUseContext::OwnershipOrLifecycle,
    )
}

fn map_optional_box_view_begin<M: MirLocalIdentityMapper>(
    begin: &$($mir_mutability)* MirOptionalBoxViewBegin,
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
    map_storage_use(mapper, site, MirStorageUseRole::Alias, owner)
}

fn map_optional_box_view_end<M: MirLocalIdentityMapper>(
    end: &$($mir_mutability)* MirOptionalBoxViewEnd,
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
    map_storage_use(mapper, site, MirStorageUseRole::Alias, owner)
}

fn map_optional_shared_source<M: MirLocalIdentityMapper>(
    source: &$($mir_mutability)* MirOptionalSharedSource,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    match source {
        MirOptionalSharedSource::Absent => Ok(()),
        MirOptionalSharedSource::Present(owner) | MirOptionalSharedSource::Move(owner) => {
            map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, owner)
        }
        MirOptionalSharedSource::Copy(place) => map_place(
            place,
            mapper,
            site,
            MirPlaceUseContext::OwnershipOrLifecycle,
        ),
    }
}

fn map_optional_shared_initialize<M: MirLocalIdentityMapper>(
    instruction: &$($mir_mutability)* MirOptionalSharedInitialize,
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
    map_place(
        destination,
        mapper,
        site,
        MirPlaceUseContext::OwnershipOrLifecycle,
    )?;
    map_optional_shared_source(source, mapper, site)
}

fn map_optional_shared_assign<M: MirLocalIdentityMapper>(
    instruction: &$($mir_mutability)* MirOptionalSharedAssign,
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
    map_place(
        destination,
        mapper,
        site,
        MirPlaceUseContext::OwnershipOrLifecycle,
    )?;
    map_optional_shared_source(source, mapper, site)
}

fn map_io_instruction<M: MirLocalIdentityMapper>(
    instruction: &$($mir_mutability)* MirIoInstruction,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirIoInstruction {
        result,
        operation,
        span: _,
    } = instruction;
    map_value_definition(mapper, site, result)?;
    match operation {
        MirIoOperation::StandardHandle { stream } => {
            map_value_use(mapper, site, MirValueUseRole::InputOutput, stream)
        }
        MirIoOperation::Open { path, mode } => {
            map_io_buffer(path, mapper, site)?;
            map_value_use(mapper, site, MirValueUseRole::InputOutput, mode)
        }
        MirIoOperation::Read {
            handle,
            destination,
            offset,
        } => {
            map_value_use(mapper, site, MirValueUseRole::InputOutput, handle)?;
            map_io_buffer(destination, mapper, site)?;
            map_storage_use(mapper, site, MirStorageUseRole::InputOutput, offset)
        }
        MirIoOperation::Write {
            handle,
            source,
            offset,
        } => {
            map_value_use(mapper, site, MirValueUseRole::InputOutput, handle)?;
            map_io_buffer(source, mapper, site)?;
            map_storage_use(mapper, site, MirStorageUseRole::InputOutput, offset)
        }
        MirIoOperation::Close { handle } => {
            map_value_use(mapper, site, MirValueUseRole::InputOutput, handle)
        }
    }
}

fn map_io_buffer<M: MirLocalIdentityMapper>(
    buffer: &$($mir_mutability)* MirIoBuffer,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirIoBuffer {
        place,
        anchor,
        array: _,
        access: _,
    } = buffer;
    map_place(place, mapper, site, MirPlaceUseContext::InputOutput)?;
    map_storage_use(mapper, site, MirStorageUseRole::InputOutput, anchor)
}

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

pub(crate) fn map_terminator<M: MirLocalIdentityMapper>(
    terminator: &$($mir_mutability)* MirTerminator,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    match terminator {
        MirTerminator::Return { value, span: _ } => {
            if let Some(value) = value {
                map_value_use(mapper, site, MirValueUseRole::OrdinaryReturn, value)?;
            }
            Ok(())
        }
        MirTerminator::ReturnShared { owner, span: _ }
        | MirTerminator::ReturnOptionalShared { owner, span: _ } => {
            map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, owner)
        }
        MirTerminator::Panic { message, span: _ } => map_place(
            message,
            mapper,
            site,
            MirPlaceUseContext::OtherExecutable,
        ),
        MirTerminator::Goto { target, span: _ } => map_block(mapper, site, target),
        MirTerminator::Branch {
            condition,
            true_target,
            false_target,
            span: _,
        } => {
            map_value_use(mapper, site, MirValueUseRole::OrdinaryBranch, condition)?;
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
            map_storage_use(mapper, site, MirStorageUseRole::CheckedProtocol, left)?;
            map_storage_use(mapper, site, MirStorageUseRole::CheckedProtocol, count)?;
            map_storage_use(mapper, site, MirStorageUseRole::CheckedProtocol, result)?;
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
            map_storage_use(mapper, site, MirStorageUseRole::CheckedProtocol, dividend)?;
            map_storage_use(mapper, site, MirStorageUseRole::CheckedProtocol, divisor)?;
            map_storage_use(mapper, site, MirStorageUseRole::CheckedProtocol, result)?;
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
            map_storage_use(mapper, site, MirStorageUseRole::CheckedProtocol, source)?;
            map_storage_use(mapper, site, MirStorageUseRole::CheckedProtocol, result)?;
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
                destination,
            )?;
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
                destination,
            )?;
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
            map_place(
                source,
                mapper,
                site,
                MirPlaceUseContext::OtherExecutable,
            )?;
            map_block_pair(mapper, site, success_target, failure_target)
        }
        MirTerminator::ArrayPositionCheck {
            position,
            kind: _,
            success_target,
            failure_target,
            span: _,
        } => {
            map_storage_use(mapper, site, MirStorageUseRole::OtherExecutable, position)?;
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
            kind,
            body_target,
            complete_target,
            span: _,
        } => {
            map_storage_use(
                mapper,
                site,
                MirStorageUseRole::OwnershipOrLifecycle,
                backing,
            )?;
            map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, index)?;
            map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, length)?;
            if let crate::mir::MirArrayLoopKind::Indexed { binding } = kind {
                map_storage_use(mapper, site, MirStorageUseRole::OtherExecutable, binding)?;
            }
            map_block_pair(mapper, site, body_target, complete_target)
        }
        MirTerminator::Terminate { reason: _, span: _ } => Ok(()),
    }
}

fn map_block_pair<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    first: &$($mir_mutability)* BlockId,
    second: &$($mir_mutability)* BlockId,
) -> Result<(), M::Error> {
    map_block(mapper, site, first)?;
    map_block(mapper, site, second)
}

fn map_block_triple<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    first: &$($mir_mutability)* BlockId,
    second: &$($mir_mutability)* BlockId,
    third: &$($mir_mutability)* BlockId,
) -> Result<(), M::Error> {
    map_block(mapper, site, first)?;
    map_block(mapper, site, second)?;
    map_block(mapper, site, third)
}

#[derive(Clone, Copy)]
enum MirPlaceUseContext {
    OrdinaryRead,
    OrdinaryWrite(MirStorageWriteAuthorization),
    Alias,
    Call,
    OwnershipOrLifecycle,
    InputOutput,
    OtherExecutable,
}

impl MirPlaceUseContext {
    const fn storage_role(self, place: MirStoragePlaceUse) -> MirStorageUseRole {
        match self {
            Self::OrdinaryRead => MirStorageUseRole::OrdinaryRead(place),
            Self::OrdinaryWrite(authorization) => MirStorageUseRole::OrdinaryWrite {
                place,
                authorization,
            },
            Self::Alias => MirStorageUseRole::Alias,
            Self::Call => MirStorageUseRole::Call,
            Self::OwnershipOrLifecycle => MirStorageUseRole::OwnershipOrLifecycle,
            Self::InputOutput => MirStorageUseRole::InputOutput,
            Self::OtherExecutable => MirStorageUseRole::OtherExecutable,
        }
    }
}

fn map_place<M: MirLocalIdentityMapper>(
    place: &$($mir_mutability)* MirPlace,
    mapper: &mut M,
    site: MirLocalIdentitySite,
    context: MirPlaceUseContext,
) -> Result<(), M::Error> {
    let MirPlace { base, projections } = place;
    let place_use = if !projections.is_empty() {
        MirStoragePlaceUse::Projected
    } else if matches!(base, MirPlaceBase::Storage(_)) {
        MirStoragePlaceUse::ExactBase
    } else {
        MirStoragePlaceUse::Alias
    };
    let role = context.storage_role(place_use);
    match base {
        MirPlaceBase::StaticField(_) | MirPlaceBase::StaticLifecycleDestination(_) => {}
        MirPlaceBase::Storage(storage)
        | MirPlaceBase::AliasParameter(storage)
        | MirPlaceBase::CheckedView(storage)
        | MirPlaceBase::ArrayAlias(storage)
        | MirPlaceBase::SharedPointee(storage)
        | MirPlaceBase::SharedAllocationPayload(storage) => {
            map_storage_use(mapper, site, role, storage)?;
        }
        MirPlaceBase::OptionalBoxPayload { owner, target: _ } => {
            map_storage_use(mapper, site, role, owner)?;
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
            } => map_storage_use(
                mapper,
                site,
                MirStorageUseRole::OtherExecutable,
                normalized_index,
            )?,
        }
    }
    Ok(())
}

fn map_object_view<M: MirLocalIdentityMapper>(
    view: &$($mir_mutability)* MirObjectView,
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
    map_place(source, mapper, site, MirPlaceUseContext::Alias)?;
    map_object_origin(origin, mapper, site)
}

fn map_method_receiver<M: MirLocalIdentityMapper>(
    receiver: &$($mir_mutability)* MirMethodReceiver,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    let MirMethodReceiver {
        place,
        origin,
        access: _,
        provenance: _,
    } = receiver;
    map_place(place, mapper, site, MirPlaceUseContext::Alias)?;
    map_object_origin(origin, mapper, site)
}

fn map_object_origin<M: MirLocalIdentityMapper>(
    origin: &$($mir_mutability)* MirObjectOrigin,
    mapper: &mut M,
    site: MirLocalIdentitySite,
) -> Result<(), M::Error> {
    match origin {
        MirObjectOrigin::Exact {
            complete,
            dynamic_class: _,
        } => map_place(complete, mapper, site, MirPlaceUseContext::Alias),
        MirObjectOrigin::Forwarded {
            carrier,
            static_target: _,
            access: _,
            dispatch_limit: _,
            span: _,
        } => map_storage_use(mapper, site, MirStorageUseRole::Alias, carrier),
        MirObjectOrigin::Shared {
            owner,
            static_target: _,
            access: _,
            exact_dynamic_class: _,
            span: _,
        } => map_storage_use(mapper, site, MirStorageUseRole::Alias, owner),
    }
}

fn map_optional_storage<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    role: MirStorageUseRole,
    identity: &$($mir_mutability)* Option<StorageId>,
) -> Result<(), M::Error> {
    if let Some(identity) = identity {
        map_storage_use(mapper, site, role, identity)?;
    }
    Ok(())
}

fn map_storage_use<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    role: MirStorageUseRole,
    identity: &$($mir_mutability)* StorageId,
) -> Result<(), M::Error> {
    $leaf!(storage_use, mapper, site, role, identity)
}

fn map_value<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    identity: &$($mir_mutability)* ValueId,
) -> Result<(), M::Error> {
    $leaf!(value, mapper, site, identity)
}

fn map_value_use<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    role: MirValueUseRole,
    identity: &$($mir_mutability)* ValueId,
) -> Result<(), M::Error> {
    $leaf!(value_use, mapper, site, role, identity)
}

fn map_value_definition<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    identity: &$($mir_mutability)* ValueId,
) -> Result<(), M::Error> {
    $leaf!(value_definition, mapper, site, identity)
}

fn map_block<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    identity: &$($mir_mutability)* BlockId,
) -> Result<(), M::Error> {
    $leaf!(block, mapper, site, identity)
}

fn map_path_condition<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    identity: &$($mir_mutability)* PathConditionId,
) -> Result<(), M::Error> {
    $leaf!(path_condition, mapper, site, identity)
}

fn map_optional_guard<M: MirLocalIdentityMapper>(
    mapper: &mut M,
    site: MirLocalIdentitySite,
    identity: &$($mir_mutability)* OptionalGuardId,
) -> Result<(), M::Error> {
    $leaf!(optional_guard, mapper, site, identity)
}

}
    };
}

define_identity_traversal!(mapping, MirLocalIdentityMapper, (mut), map_identity);
define_identity_traversal!(observation, MirLocalIdentityObserver, (), observe_identity);

pub(super) use mapping::{
    map_body_local_identities, map_common_local_identities, map_function_attachments,
    map_instruction, map_logical_expression, map_member_attachments, map_path_condition_metadata,
    map_static_publication_attachment, map_terminator,
};
pub(crate) use mapping::{
    map_function_local_identities, map_member_local_identities,
    map_static_initializer_local_identities,
};
pub(super) use observation::{
    map_body_local_identities as observe_body_local_identities,
    map_instruction as observe_instruction, map_logical_expression as observe_logical_expression,
    map_path_condition_metadata as observe_path_condition_metadata,
    map_terminator as observe_terminator,
};
pub(crate) use observation::{
    map_function_local_identities as observe_function_local_identities,
    map_member_local_identities as observe_member_local_identities,
    map_static_initializer_local_identities as observe_static_initializer_local_identities,
};

/// Observes one borrowed executable definition without exposing its concrete
/// function, member, or static-initializer shape to analyses.
pub(crate) fn observe_definition_local_identities<O: MirLocalIdentityObserver>(
    definition: MirDefinitionRef<'_>,
    observer: &mut O,
) -> Result<(), O::Error> {
    match definition {
        MirDefinitionRef::Function(definition) => {
            observe_function_local_identities(definition, observer)
        }
        MirDefinitionRef::Member(definition) => {
            observe_member_local_identities(definition, observer)
        }
        MirDefinitionRef::StaticInitializer(definition) => {
            observe_static_initializer_local_identities(definition, observer)
        }
    }
}

pub(crate) fn validate_function_local_identity_owners(
    definition: &MirFunctionDefinition,
) -> Result<(), MirLocalIdentityOwnershipError> {
    let mut validator = LocalIdentityOwnerValidator::new(definition.callable());
    observe_function_local_identities(definition, &mut validator)
}

pub(crate) fn validate_member_local_identity_owners(
    definition: &MirMemberDefinition,
) -> Result<(), MirLocalIdentityOwnershipError> {
    let mut validator = LocalIdentityOwnerValidator::new(definition.callable);
    observe_member_local_identities(definition, &mut validator)
}

pub(crate) fn validate_static_initializer_local_identity_owners(
    definition: &MirStaticInitializerBody,
) -> Result<(), MirLocalIdentityOwnershipError> {
    let mut validator = LocalIdentityOwnerValidator::new(definition.callable());
    observe_static_initializer_local_identities(definition, &mut validator)
}

#[cfg(test)]
pub(super) fn preserve_function_local_identities(
    definition: &mut MirFunctionDefinition,
) -> Result<(), std::convert::Infallible> {
    map_function_local_identities(definition, &mut PreserveLocalIdentities)
}
