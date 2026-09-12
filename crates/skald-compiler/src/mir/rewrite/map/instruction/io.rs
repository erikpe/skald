//! Input/output operations and their borrowed array buffers.

macro_rules! define_io_instruction_traversal {
    (($($mir_mutability:tt)*)) => {
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

    };
}

pub(in crate::mir::rewrite::map) use define_io_instruction_traversal;
