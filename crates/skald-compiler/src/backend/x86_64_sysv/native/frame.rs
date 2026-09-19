//! SysV frame policy: fixed outgoing area, aligned frame pointer, no red zone.
use super::selected::Instruction;
use crate::backend::{
    frame::{plan_frame, FrameError, FramePlan, FramePolicy, ReturnAddress},
    placement::CheckedPlacement,
};
pub(in crate::backend) fn plan_native_frame<'f, 's, 'p>(
    placement: &'f CheckedPlacement<'s, 'p, Instruction>,
) -> Result<FramePlan<'f, 's, 'p, Instruction>, FrameError> {
    plan_frame(
        placement,
        FramePolicy {
            alignment: 16,
            entry_remainder: 8,
            header_bytes: 8,
            incoming_base: 16,
            return_address: ReturnAddress::Stack {
                offset: 8,
                bytes: 8,
            },
            max_frame: i32::MAX as usize,
            direct_min: i32::MIN as i64,
            direct_max: i32::MAX as i64,
            materialization: None,
            address_scratch_group: 0,
        },
    )
}
