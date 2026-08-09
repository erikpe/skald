//! Deterministic aligned fixed-stack-frame layout.

use crate::{
    backend::{BackendError, Target},
    identity::StaticFieldId,
    mir::{
        MirDefinitionRef, MirPlace, MirPlaceBase, MirPlaceProjection, MirProgram, MirStorageKind,
        MirType, StorageId, ValueId,
    },
};

use super::{
    abi,
    layout::{DataLayout, SHARED_HANDLE_ALIGNMENT, SHARED_HANDLE_SIZE, SHARED_HEADER_SIZE},
};

const SCALAR_HOME_SIZE: usize = 8;
const SCALAR_HOME_ALIGNMENT: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FramePlace {
    base: FramePlaceBase,
    displacement: i32,
    ty: MirType,
    byte_access: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FramePlaceBase {
    StaticField(StaticFieldId),
    Direct,
    Return { home: i32 },
    Receiver { home: i32 },
    OwnedParameter { home: i32 },
    Alias { home: i32 },
    SharedPointee { home: i32 },
}

impl FramePlaceBase {
    pub(super) const fn pointer_home(self) -> Option<i32> {
        match self {
            Self::Direct | Self::StaticField(_) => None,
            Self::Return { home }
            | Self::Receiver { home }
            | Self::OwnedParameter { home }
            | Self::Alias { home }
            | Self::SharedPointee { home } => Some(home),
        }
    }
}

impl FramePlace {
    pub(super) const fn array_element(ty: MirType) -> Self {
        Self {
            base: FramePlaceBase::Direct,
            displacement: 0,
            ty,
            byte_access: matches!(ty, MirType::U8 | MirType::Bool),
        }
    }

    pub(super) const fn displacement(self) -> i32 {
        self.displacement
    }

    pub(super) const fn base(self) -> FramePlaceBase {
        self.base
    }

    pub(super) const fn ty(self) -> MirType {
        self.ty
    }

    pub(super) const fn uses_byte_access(self) -> bool {
        self.byte_access
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FrameLayout {
    size: u32,
    storage_offsets: Vec<i32>,
    object_origins: Vec<Option<ObjectOriginHomes>>,
    value_offsets: Vec<i32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ObjectOriginHomes {
    complete: i32,
    metadata: i32,
}

impl ObjectOriginHomes {
    pub(super) const fn complete(self) -> i32 {
        self.complete
    }

    pub(super) const fn metadata(self) -> i32 {
        self.metadata
    }
}

impl FrameLayout {
    pub(super) fn plan(
        function: MirDefinitionRef<'_>,
        data_layout: &DataLayout,
    ) -> Result<Self, BackendError> {
        let mut allocator = FrameAllocator::new(function);
        let mut storage_offsets = Vec::with_capacity(function.storage_entries().len());
        let mut object_origins = Vec::with_capacity(function.storage_entries().len());
        for storage in function.storage_entries() {
            let (size, alignment) = match (storage.kind, storage.ty) {
                (MirStorageKind::SharedAllocation, _)
                | (_, MirType::Shared(_) | MirType::OptionalShared(_)) => {
                    (SHARED_HANDLE_SIZE, SHARED_HANDLE_ALIGNMENT)
                }
                (
                    MirStorageKind::Return
                    | MirStorageKind::Receiver
                    | MirStorageKind::AliasParameter(_)
                    | MirStorageKind::CheckedView(_),
                    _,
                )
                | (
                    MirStorageKind::Parameter,
                    MirType::Class(_)
                    | MirType::OptionalPrimitive(_)
                    | MirType::OptionalClass(_)
                    | MirType::Array(_),
                ) => (SCALAR_HOME_SIZE, SCALAR_HOME_ALIGNMENT),
                (_, MirType::Class(_) | MirType::Unit) => {
                    let ty = data_layout.ty(storage.ty)?;
                    (ty.size(), ty.alignment())
                }
                (_, MirType::OptionalPrimitive(payload)) => {
                    let ty = data_layout.optional(payload)?.ty();
                    (ty.size(), ty.alignment())
                }
                (_, MirType::OptionalClass(class)) => {
                    let ty = data_layout.optional_class(class)?.ty();
                    (ty.size(), ty.alignment())
                }
                (_, _) => (SCALAR_HOME_SIZE, SCALAR_HOME_ALIGNMENT),
            };
            storage_offsets.push(allocator.allocate(size, alignment)?);
            let carries_origin = matches!(
                storage.kind,
                MirStorageKind::Receiver | MirStorageKind::CheckedView(_)
            ) || matches!(storage.kind, MirStorageKind::AliasParameter(_))
                && matches!(
                    storage.ty,
                    MirType::Class(_) | MirType::Interface(_) | MirType::Obj
                )
                || matches!(storage.kind, MirStorageKind::ArrayAlias(_))
                    && matches!(storage.ty, MirType::Class(_));
            object_origins.push(if carries_origin {
                Some(ObjectOriginHomes {
                    complete: allocator.allocate(SCALAR_HOME_SIZE, SCALAR_HOME_ALIGNMENT)?,
                    metadata: allocator.allocate(SCALAR_HOME_SIZE, SCALAR_HOME_ALIGNMENT)?,
                })
            } else {
                None
            });
        }
        let mut value_offsets = Vec::with_capacity(function.values().len());
        for _ in function.values() {
            value_offsets.push(allocator.allocate(SCALAR_HOME_SIZE, SCALAR_HOME_ALIGNMENT)?);
        }
        let size = allocator.finish()?;

        Ok(Self {
            size,
            storage_offsets,
            object_origins,
            value_offsets,
        })
    }

    pub(super) const fn size(&self) -> u32 {
        self.size
    }

    pub(super) fn storage(&self, id: StorageId) -> i32 {
        self.storage_offsets[id.index()]
    }

    pub(super) fn value(&self, id: ValueId) -> i32 {
        self.value_offsets[id.index()]
    }

    pub(super) fn object_origin(&self, id: StorageId) -> Option<ObjectOriginHomes> {
        self.object_origins[id.index()]
    }

    /// Resolves a verified semantic place into one frame-relative address.
    /// All target offset arithmetic stays at this layout boundary.
    pub(super) fn place(
        &self,
        program: &MirProgram,
        function: MirDefinitionRef<'_>,
        data_layout: &DataLayout,
        place: &MirPlace,
    ) -> Result<FramePlace, BackendError> {
        if let MirPlaceBase::StaticField(field_id)
        | MirPlaceBase::StaticLifecycleDestination(field_id) = place.base
        {
            let field = program
                .static_field(field_id)
                .expect("verified static place must name a declaration");
            let (displacement, ty) = projected_place(
                program,
                function,
                data_layout,
                0,
                field.ty,
                &place.projections,
            )?;
            return Ok(FramePlace {
                base: FramePlaceBase::StaticField(field_id),
                displacement,
                ty,
                byte_access: matches!(ty, MirType::U8 | MirType::Bool),
            });
        }
        let storage_id = place.base.expect_local_storage();
        let storage = function
            .storage(storage_id)
            .expect("verified place base must identify storage");
        let (base, displacement) = match place.base {
            MirPlaceBase::Storage(_)
                if storage.kind == MirStorageKind::Return
                    && !matches!(storage.ty, MirType::OptionalShared(_)) =>
            {
                (
                    FramePlaceBase::Return {
                        home: self.storage(storage_id),
                    },
                    0,
                )
            }
            MirPlaceBase::Storage(_) if storage.kind == MirStorageKind::Receiver => (
                FramePlaceBase::Receiver {
                    home: self.storage(storage_id),
                },
                0,
            ),
            MirPlaceBase::Storage(_)
                if storage.kind == MirStorageKind::Parameter
                    && matches!(
                        storage.ty,
                        MirType::Class(_)
                            | MirType::OptionalPrimitive(_)
                            | MirType::OptionalClass(_)
                            | MirType::Array(_)
                    ) =>
            {
                (
                    FramePlaceBase::OwnedParameter {
                        home: self.storage(storage_id),
                    },
                    0,
                )
            }
            MirPlaceBase::AliasParameter(_) => (
                FramePlaceBase::Alias {
                    home: self.storage(storage_id),
                },
                0,
            ),
            MirPlaceBase::CheckedView(_) | MirPlaceBase::ArrayAlias(_) => (
                FramePlaceBase::Alias {
                    home: self.storage(storage_id),
                },
                0,
            ),
            MirPlaceBase::SharedPointee(_) => (
                FramePlaceBase::SharedPointee {
                    home: self.storage(storage_id),
                },
                i32::try_from(SHARED_HEADER_SIZE)
                    .map_err(|_| place_address_error(function.callable()))?,
            ),
            MirPlaceBase::SharedAllocationPayload(_) => (
                FramePlaceBase::SharedPointee {
                    home: self.storage(storage_id),
                },
                i32::try_from(SHARED_HEADER_SIZE)
                    .map_err(|_| place_address_error(function.callable()))?,
            ),
            MirPlaceBase::Storage(_) => (FramePlaceBase::Direct, self.storage(storage_id)),
            MirPlaceBase::StaticField(_) | MirPlaceBase::StaticLifecycleDestination(_) => {
                unreachable!("static roots return before frame storage selection")
            }
        };
        let ty = match (place.base, storage.ty) {
            (MirPlaceBase::SharedPointee(_), MirType::Shared(target)) => target.ty(),
            _ => storage.ty,
        };
        let (displacement, ty) = projected_place(
            program,
            function,
            data_layout,
            displacement,
            ty,
            &place.projections,
        )?;
        Ok(FramePlace {
            base,
            displacement,
            ty,
            byte_access: !place.projections.is_empty() && matches!(ty, MirType::U8 | MirType::Bool),
        })
    }
}

fn projected_place(
    program: &MirProgram,
    function: MirDefinitionRef<'_>,
    data_layout: &DataLayout,
    mut displacement: i32,
    mut ty: MirType,
    projections: &[MirPlaceProjection],
) -> Result<(i32, MirType), BackendError> {
    for projection in projections {
        let (offset, projected_ty) = match *projection {
            MirPlaceProjection::Base(base) => {
                let MirType::Class(class) = ty else {
                    return Err(place_metadata_error(function.callable()));
                };
                let layout = data_layout
                    .class(class)
                    .and_then(|layout| layout.base())
                    .filter(|layout| layout.class == base)
                    .ok_or_else(|| place_metadata_error(function.callable()))?;
                (layout.offset, MirType::Class(base))
            }
            MirPlaceProjection::Field(field_id) => {
                let layout = data_layout
                    .field(field_id)
                    .expect("verified field must have a target layout");
                let ty = program
                    .field(field_id)
                    .expect("verified field must be declared")
                    .ty;
                (layout.offset, ty)
            }
            MirPlaceProjection::OptionalPayload(class) => {
                if ty != MirType::OptionalClass(class) {
                    return Err(place_metadata_error(function.callable()));
                }
                (
                    data_layout.optional_class(class)?.payload_offset(),
                    MirType::Class(class),
                )
            }
            MirPlaceProjection::ArrayElement { .. } => {
                unreachable!("array element addresses are selected by array lowering")
            }
        };
        let offset = i32::try_from(offset).map_err(|_| place_address_error(function.callable()))?;
        displacement = displacement
            .checked_add(offset)
            .ok_or_else(|| place_address_error(function.callable()))?;
        ty = projected_ty;
    }
    Ok((displacement, ty))
}

struct FrameAllocator {
    callable: crate::identity::CallableId,
    used: usize,
}

impl FrameAllocator {
    const fn new(function: MirDefinitionRef<'_>) -> Self {
        Self {
            callable: function.callable(),
            used: 0,
        }
    }

    fn allocate(&mut self, size: usize, alignment: usize) -> Result<i32, BackendError> {
        let start = abi::align_up(self.used, alignment).ok_or_else(|| self.error())?;
        let end = start.checked_add(size).ok_or_else(|| self.error())?;
        let displacement = i32::try_from(end)
            .ok()
            .and_then(i32::checked_neg)
            .ok_or_else(|| self.error())?;
        self.used = end;
        Ok(displacement)
    }

    fn finish(&self) -> Result<u32, BackendError> {
        let size = abi::align_up(self.used, abi::STACK_ALIGNMENT).ok_or_else(|| self.error())?;
        if size > i32::MAX as usize {
            return Err(self.error());
        }
        u32::try_from(size).map_err(|_| self.error())
    }

    fn error(&self) -> BackendError {
        frame_too_large(self.callable)
    }
}

fn frame_too_large(callable: crate::identity::CallableId) -> BackendError {
    BackendError::new(
        Target::X86_64SysV,
        Some(callable),
        "stack frame is too large for x86-64 frame-relative addressing",
    )
}

fn place_address_error(callable: crate::identity::CallableId) -> BackendError {
    BackendError::new(
        Target::X86_64SysV,
        Some(callable),
        "projected place exceeds x86-64 displacement limits",
    )
}

fn place_metadata_error(callable: crate::identity::CallableId) -> BackendError {
    BackendError::new(
        Target::X86_64SysV,
        Some(callable),
        "projected base has no matching x86-64 class layout",
    )
}

#[cfg(test)]
mod tests {
    use crate::identity::{CallableId, FunctionId};

    use super::*;

    #[test]
    fn frame_allocator_rejects_displacements_beyond_signed_32_bits() {
        let callable = CallableId::Function(FunctionId::new(0));
        let mut allocator = FrameAllocator {
            callable,
            used: i32::MAX as usize,
        };

        let error = allocator.allocate(1, 1).unwrap_err();

        assert_eq!(error.target(), Target::X86_64SysV);
        assert_eq!(error.callable(), Some(callable));
        assert_eq!(
            error.message(),
            "stack frame is too large for x86-64 frame-relative addressing"
        );
    }

    #[test]
    fn frame_allocator_rejects_alignment_overflow() {
        let callable = CallableId::Function(FunctionId::new(0));
        let mut allocator = FrameAllocator {
            callable,
            used: usize::MAX,
        };

        let error = allocator.allocate(1, SCALAR_HOME_ALIGNMENT).unwrap_err();

        assert_eq!(error.target(), Target::X86_64SysV);
        assert_eq!(error.callable(), Some(callable));
        assert_eq!(
            error.message(),
            "stack frame is too large for x86-64 frame-relative addressing"
        );
    }
}
