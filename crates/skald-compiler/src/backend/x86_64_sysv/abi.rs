//! System V AMD64 scalar argument classification used by this backend.

use crate::mir::MirType;

use super::machine::{Register, XmmRegister};

pub(super) const INTEGER_ARGUMENT_REGISTERS: [Register; 6] = [
    Register::Rdi,
    Register::Rsi,
    Register::Rdx,
    Register::Rcx,
    Register::R8,
    Register::R9,
];

pub(super) const SSE_ARGUMENT_REGISTERS: [XmmRegister; 8] = [
    XmmRegister::Xmm0,
    XmmRegister::Xmm1,
    XmmRegister::Xmm2,
    XmmRegister::Xmm3,
    XmmRegister::Xmm4,
    XmmRegister::Xmm5,
    XmmRegister::Xmm6,
    XmmRegister::Xmm7,
];

pub(super) const STACK_ALIGNMENT: usize = 16;
const STACK_SLOT_SIZE: usize = 8;
const INCOMING_STACK_BASE: i32 = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ArgumentLocation {
    IntegerRegister(Register),
    SseRegister(XmmRegister),
    /// Byte offset in the caller's outgoing argument area.
    Stack(i32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarClass {
    Integer,
    Sse,
}

/// Classifies every MIR type supported by this scalar ABI profile.
///
/// Keep this match exhaustive: adding a MIR type must make its target ABI
/// treatment an explicit backend decision before the compiler builds again.
const fn scalar_class(ty: MirType) -> Option<ScalarClass> {
    match ty {
        MirType::I64 | MirType::U64 | MirType::U8 | MirType::Bool => Some(ScalarClass::Integer),
        MirType::F64 => Some(ScalarClass::Sse),
        MirType::Unit => None,
    }
}

impl ArgumentLocation {
    pub(super) fn incoming(self) -> Option<Self> {
        match self {
            Self::Stack(offset) => offset.checked_add(INCOMING_STACK_BASE).map(Self::Stack),
            register => Some(register),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CallLayout {
    locations: Vec<ArgumentLocation>,
    stack_size: u32,
}

impl CallLayout {
    pub(super) fn classify(types: &[MirType]) -> Option<Self> {
        let mut integer_index = 0;
        let mut sse_index = 0;
        let mut stack_count = 0usize;
        let mut locations = Vec::with_capacity(types.len());

        for &ty in types {
            let location = match scalar_class(ty)? {
                ScalarClass::Sse if sse_index < SSE_ARGUMENT_REGISTERS.len() => {
                    let register = SSE_ARGUMENT_REGISTERS[sse_index];
                    sse_index += 1;
                    ArgumentLocation::SseRegister(register)
                }
                ScalarClass::Sse => stack_location(stack_count)?,
                ScalarClass::Integer if integer_index < INTEGER_ARGUMENT_REGISTERS.len() => {
                    let register = INTEGER_ARGUMENT_REGISTERS[integer_index];
                    integer_index += 1;
                    ArgumentLocation::IntegerRegister(register)
                }
                ScalarClass::Integer => stack_location(stack_count)?,
            };
            if matches!(location, ArgumentLocation::Stack(_)) {
                stack_count += 1;
            }
            locations.push(location);
        }

        let bytes = stack_count.checked_mul(STACK_SLOT_SIZE)?;
        let aligned = align_up(bytes, STACK_ALIGNMENT)?;
        let stack_size = u32::try_from(aligned).ok()?;
        (aligned <= i32::MAX as usize).then_some(Self {
            locations,
            stack_size,
        })
    }

    pub(super) fn locations(&self) -> &[ArgumentLocation] {
        &self.locations
    }

    pub(super) const fn stack_size(&self) -> u32 {
        self.stack_size
    }
}

fn stack_location(index: usize) -> Option<ArgumentLocation> {
    let offset = index.checked_mul(STACK_SLOT_SIZE)?;
    i32::try_from(offset).ok().map(ArgumentLocation::Stack)
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
    fn integer_and_sse_register_counters_are_independent() {
        let layout =
            CallLayout::classify(&[MirType::I64, MirType::F64, MirType::U8, MirType::F64]).unwrap();

        assert_eq!(
            layout.locations(),
            [
                ArgumentLocation::IntegerRegister(Register::Rdi),
                ArgumentLocation::SseRegister(XmmRegister::Xmm0),
                ArgumentLocation::IntegerRegister(Register::Rsi),
                ArgumentLocation::SseRegister(XmmRegister::Xmm1),
            ]
        );
        assert_eq!(layout.stack_size(), 0);
    }

    #[test]
    fn classifies_every_payload_primitive_through_one_exhaustive_boundary() {
        let layout = CallLayout::classify(&[
            MirType::I64,
            MirType::U64,
            MirType::U8,
            MirType::Bool,
            MirType::F64,
        ])
        .unwrap();

        assert_eq!(
            layout.locations(),
            [
                ArgumentLocation::IntegerRegister(Register::Rdi),
                ArgumentLocation::IntegerRegister(Register::Rsi),
                ArgumentLocation::IntegerRegister(Register::Rdx),
                ArgumentLocation::IntegerRegister(Register::Rcx),
                ArgumentLocation::SseRegister(XmmRegister::Xmm0),
            ]
        );
    }

    #[test]
    fn independently_exhausted_classes_share_source_ordered_stack_slots() {
        let mut types = vec![MirType::I64; 6];
        types.extend([MirType::F64; 8]);
        types.extend([MirType::F64, MirType::I64, MirType::F64]);
        let layout = CallLayout::classify(&types).unwrap();

        assert_eq!(
            &layout.locations()[14..],
            [
                ArgumentLocation::Stack(0),
                ArgumentLocation::Stack(8),
                ArgumentLocation::Stack(16),
            ]
        );
        assert_eq!(layout.stack_size(), 32);
        assert_eq!(
            layout.locations()[14].incoming(),
            Some(ArgumentLocation::Stack(16))
        );
    }

    #[test]
    fn rejects_payload_free_parameters_and_unrepresentable_layouts() {
        assert!(CallLayout::classify(&[MirType::Unit]).is_none());
        assert_eq!(align_up(8, STACK_ALIGNMENT), Some(16));
        assert_eq!(align_up(16, STACK_ALIGNMENT), Some(16));
    }
}
