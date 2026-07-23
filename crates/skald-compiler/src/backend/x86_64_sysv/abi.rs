//! System V AMD64 scalar argument classification used by this backend.

use crate::mir::{MirParameter, MirParameterMode, MirType};

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
const fn parameter_class(parameter: MirParameter) -> Option<ScalarClass> {
    match parameter.mode {
        MirParameterMode::ReadOnlyAlias | MirParameterMode::MutableAlias => {
            Some(ScalarClass::Integer)
        }
        MirParameterMode::Value => match parameter.ty {
            MirType::I64 | MirType::U64 | MirType::U8 | MirType::Bool | MirType::Class(_) => {
                Some(ScalarClass::Integer)
            }
            MirType::F64 => Some(ScalarClass::Sse),
            MirType::Obj | MirType::Unit => None,
        },
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
    return_destination: Option<ArgumentLocation>,
    receiver: Option<ArgumentLocation>,
    locations: Vec<ArgumentLocation>,
    stack_size: u32,
}

impl CallLayout {
    pub(super) fn classify(parameters: &[MirParameter]) -> Option<Self> {
        Self::classify_internal(parameters, false, false)
    }

    pub(super) fn classify_with_receiver(parameters: &[MirParameter]) -> Option<Self> {
        Self::classify_internal(parameters, true, false)
    }

    pub(super) fn classify_internal_call(
        parameters: &[MirParameter],
        has_receiver: bool,
        has_return_destination: bool,
    ) -> Option<Self> {
        Self::classify_internal(parameters, has_receiver, has_return_destination)
    }

    fn classify_internal(
        parameters: &[MirParameter],
        has_receiver: bool,
        has_return_destination: bool,
    ) -> Option<Self> {
        let return_destination = has_return_destination.then_some(
            ArgumentLocation::IntegerRegister(INTEGER_ARGUMENT_REGISTERS[0]),
        );
        let receiver_index = usize::from(has_return_destination);
        let receiver = has_receiver.then_some(ArgumentLocation::IntegerRegister(
            INTEGER_ARGUMENT_REGISTERS[receiver_index],
        ));
        let mut integer_index = receiver_index + usize::from(has_receiver);
        let mut sse_index = 0;
        let mut stack_count = 0usize;
        let mut locations = Vec::with_capacity(parameters.len());

        for &parameter in parameters {
            let location = match parameter_class(parameter)? {
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
            return_destination,
            receiver,
            locations,
            stack_size,
        })
    }

    pub(super) const fn return_destination(&self) -> Option<ArgumentLocation> {
        self.return_destination
    }

    pub(super) const fn receiver(&self) -> Option<ArgumentLocation> {
        self.receiver
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
        let layout = CallLayout::classify(&MirParameter::values([
            MirType::I64,
            MirType::F64,
            MirType::U8,
            MirType::F64,
        ]))
        .unwrap();

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
        let layout = CallLayout::classify(&MirParameter::values([
            MirType::I64,
            MirType::U64,
            MirType::U8,
            MirType::Bool,
            MirType::F64,
        ]))
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
        let layout = CallLayout::classify(&MirParameter::values(types)).unwrap();

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
        assert!(CallLayout::classify(&MirParameter::values([MirType::Unit])).is_none());
        assert_eq!(align_up(8, STACK_ALIGNMENT), Some(16));
        assert_eq!(align_up(16, STACK_ALIGNMENT), Some(16));
    }

    #[test]
    fn hidden_receiver_consumes_only_the_first_integer_location() {
        let layout = CallLayout::classify_with_receiver(&MirParameter::values([
            MirType::I64,
            MirType::F64,
            MirType::I64,
            MirType::F64,
        ]))
        .unwrap();

        assert_eq!(
            layout.receiver(),
            Some(ArgumentLocation::IntegerRegister(Register::Rdi))
        );
        assert_eq!(
            layout.locations(),
            [
                ArgumentLocation::IntegerRegister(Register::Rsi),
                ArgumentLocation::SseRegister(XmmRegister::Xmm0),
                ArgumentLocation::IntegerRegister(Register::Rdx),
                ArgumentLocation::SseRegister(XmmRegister::Xmm1),
            ]
        );
    }

    #[test]
    fn receiver_layout_preserves_independent_exhaustion_and_stack_order() {
        let mut types = vec![MirType::I64; 5];
        types.extend([MirType::F64; 8]);
        types.extend([MirType::I64, MirType::F64]);
        let layout = CallLayout::classify_with_receiver(&MirParameter::values(types)).unwrap();

        assert_eq!(layout.locations()[13], ArgumentLocation::Stack(0));
        assert_eq!(layout.locations()[14], ArgumentLocation::Stack(8));
        assert_eq!(layout.stack_size(), 16);
    }

    #[test]
    fn alias_descriptors_are_integer_class_independent_of_access_mode() {
        let class = crate::identity::ClassId::new(0);
        let layout = CallLayout::classify(&[
            MirParameter::read_only_alias(MirType::Class(class)),
            MirParameter::value(MirType::F64),
            MirParameter::mutable_alias(MirType::Class(class)),
        ])
        .unwrap();

        assert_eq!(
            layout.locations(),
            [
                ArgumentLocation::IntegerRegister(Register::Rdi),
                ArgumentLocation::SseRegister(XmmRegister::Xmm0),
                ArgumentLocation::IntegerRegister(Register::Rsi),
            ]
        );
    }

    #[test]
    fn aliases_and_sse_values_exhaust_independently_in_source_order() {
        let class = crate::identity::ClassId::new(0);
        let mut parameters = vec![MirParameter::read_only_alias(MirType::Class(class)); 6];
        parameters.extend(MirParameter::values([MirType::F64; 8]));
        parameters.extend([
            MirParameter::mutable_alias(MirType::Class(class)),
            MirParameter::value(MirType::F64),
        ]);
        let layout = CallLayout::classify(&parameters).unwrap();

        assert_eq!(layout.locations()[14], ArgumentLocation::Stack(0));
        assert_eq!(layout.locations()[15], ArgumentLocation::Stack(8));
        assert_eq!(layout.stack_size(), 16);
    }

    #[test]
    fn receiver_aliases_and_sse_values_keep_independent_counters() {
        let class = crate::identity::ClassId::new(0);
        let mut parameters = vec![MirParameter::read_only_alias(MirType::Class(class)); 5];
        parameters.extend(MirParameter::values([MirType::F64; 8]));
        parameters.extend([
            MirParameter::mutable_alias(MirType::Class(class)),
            MirParameter::value(MirType::F64),
        ]);
        let layout = CallLayout::classify_with_receiver(&parameters).unwrap();

        assert_eq!(
            layout.receiver(),
            Some(ArgumentLocation::IntegerRegister(Register::Rdi))
        );
        assert_eq!(layout.locations()[13], ArgumentLocation::Stack(0));
        assert_eq!(layout.locations()[14], ArgumentLocation::Stack(8));
        assert_eq!(layout.stack_size(), 16);
    }
}
