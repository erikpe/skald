//! Deterministic aligned fixed-stack-frame layout.

use crate::{
    backend::{BackendError, Target},
    mir::{
        MirFunctionDefinition, MirPlace, MirPlaceProjection, MirProgram, MirType, StorageId,
        ValueId,
    },
};

use super::{abi, layout::DataLayout};

const SCALAR_HOME_SIZE: usize = 8;
const SCALAR_HOME_ALIGNMENT: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FramePlace {
    displacement: i32,
    ty: MirType,
    byte_access: bool,
}

impl FramePlace {
    pub(super) const fn displacement(self) -> i32 {
        self.displacement
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
    value_offsets: Vec<i32>,
}

impl FrameLayout {
    pub(super) fn plan(
        function: &MirFunctionDefinition,
        data_layout: &DataLayout,
    ) -> Result<Self, BackendError> {
        let mut allocator = FrameAllocator::new(function);
        let mut storage_offsets = Vec::with_capacity(function.storage.len());
        for storage in &function.storage {
            let (size, alignment) = match storage.ty {
                MirType::Class(_) | MirType::Unit => {
                    let ty = data_layout.ty(storage.ty)?;
                    (ty.size(), ty.alignment())
                }
                _ => (SCALAR_HOME_SIZE, SCALAR_HOME_ALIGNMENT),
            };
            storage_offsets.push(allocator.allocate(size, alignment)?);
        }
        let mut value_offsets = Vec::with_capacity(function.values.len());
        for _ in &function.values {
            value_offsets.push(allocator.allocate(SCALAR_HOME_SIZE, SCALAR_HOME_ALIGNMENT)?);
        }
        let size = allocator.finish()?;

        Ok(Self {
            size,
            storage_offsets,
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

    /// Resolves a verified semantic place into one frame-relative address.
    /// All target offset arithmetic stays at this layout boundary.
    pub(super) fn place(
        &self,
        program: &MirProgram,
        function: &MirFunctionDefinition,
        data_layout: &DataLayout,
        place: &MirPlace,
    ) -> FramePlace {
        let storage = function
            .storage(place.base)
            .expect("verified place base must identify storage");
        let mut displacement = self.storage(place.base);
        let mut ty = storage.ty;
        for projection in &place.projections {
            match *projection {
                MirPlaceProjection::Field(field_id) => {
                    let field_layout = data_layout
                        .field(field_id)
                        .expect("verified field must have a target layout");
                    let offset = i32::try_from(field_layout.offset)
                        .expect("planned field offset must fit frame addressing");
                    displacement = displacement
                        .checked_add(offset)
                        .expect("planned place must fit frame addressing");
                    ty = program
                        .field(field_id)
                        .expect("verified field must be declared")
                        .ty;
                }
            }
        }
        FramePlace {
            displacement,
            ty,
            byte_access: !place.projections.is_empty() && matches!(ty, MirType::U8 | MirType::Bool),
        }
    }
}

struct FrameAllocator<'function> {
    function: &'function MirFunctionDefinition,
    used: usize,
}

impl<'function> FrameAllocator<'function> {
    const fn new(function: &'function MirFunctionDefinition) -> Self {
        Self { function, used: 0 }
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
        frame_too_large(self.function)
    }
}

fn frame_too_large(function: &MirFunctionDefinition) -> BackendError {
    BackendError::new(
        Target::X86_64SysV,
        Some(function.function),
        "stack frame is too large for x86-64 frame-relative addressing",
    )
}
