//! System V AMD64 integer calling-convention rules used by this backend.

use super::machine::Register;

pub(super) const ARGUMENT_REGISTERS: [Register; 6] = [
    Register::Rdi,
    Register::Rsi,
    Register::Rdx,
    Register::Rcx,
    Register::R8,
    Register::R9,
];

pub(super) const STACK_ALIGNMENT: usize = 16;
const WORD_SIZE: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum IncomingArgument {
    Register(Register),
    /// Offset from `%rbp` after the standard frame-pointer prologue.
    Stack(i32),
}

pub(super) fn incoming_argument(index: usize) -> Option<IncomingArgument> {
    if let Some(register) = ARGUMENT_REGISTERS.get(index) {
        return Some(IncomingArgument::Register(*register));
    }

    let stack_index = index.checked_sub(ARGUMENT_REGISTERS.len())?;
    let byte_offset = stack_index.checked_mul(WORD_SIZE)?.checked_add(16)?;
    i32::try_from(byte_offset).ok().map(IncomingArgument::Stack)
}

/// Space reserved below `%rsp` by a caller for stack arguments. Rounding this
/// to 16 bytes preserves call-site alignment after an aligned fixed frame.
pub(super) fn outgoing_stack_size(argument_count: usize) -> Option<u32> {
    let stack_arguments = argument_count.saturating_sub(ARGUMENT_REGISTERS.len());
    let bytes = stack_arguments.checked_mul(WORD_SIZE)?;
    let aligned = align_up(bytes, STACK_ALIGNMENT)?;
    (aligned <= i32::MAX as usize)
        .then(|| u32::try_from(aligned).ok())
        .flatten()
}

pub(super) fn outgoing_argument_offset(index: usize) -> Option<i32> {
    let stack_index = index.checked_sub(ARGUMENT_REGISTERS.len())?;
    let byte_offset = stack_index.checked_mul(WORD_SIZE)?;
    i32::try_from(byte_offset).ok()
}

pub(super) fn align_up(value: usize, alignment: usize) -> Option<usize> {
    debug_assert!(alignment.is_power_of_two());
    value
        .checked_add(alignment - 1)
        .map(|rounded| rounded & !(alignment - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_register_and_stack_arguments() {
        assert_eq!(
            incoming_argument(0),
            Some(IncomingArgument::Register(Register::Rdi))
        );
        assert_eq!(
            incoming_argument(5),
            Some(IncomingArgument::Register(Register::R9))
        );
        assert_eq!(incoming_argument(6), Some(IncomingArgument::Stack(16)));
        assert_eq!(incoming_argument(7), Some(IncomingArgument::Stack(24)));
    }

    #[test]
    fn aligns_outgoing_stack_arguments() {
        assert_eq!(outgoing_stack_size(0), Some(0));
        assert_eq!(outgoing_stack_size(6), Some(0));
        assert_eq!(outgoing_stack_size(7), Some(16));
        assert_eq!(outgoing_stack_size(8), Some(16));
        assert_eq!(outgoing_stack_size(9), Some(32));
    }
}
