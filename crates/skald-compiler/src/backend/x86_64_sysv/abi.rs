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
pub(super) struct ObjectLocations {
    address: ArgumentLocation,
    complete: ArgumentLocation,
    metadata: ArgumentLocation,
}

impl ObjectLocations {
    pub(super) const fn address(self) -> ArgumentLocation {
        self.address
    }

    pub(super) const fn complete(self) -> ArgumentLocation {
        self.complete
    }

    pub(super) const fn metadata(self) -> ArgumentLocation {
        self.metadata
    }

    pub(super) fn incoming(self) -> Option<Self> {
        Some(Self {
            address: self.address.incoming()?,
            complete: self.complete.incoming()?,
            metadata: self.metadata.incoming()?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ParameterLocations {
    value: ArgumentLocation,
    origin: Option<ObjectOriginLocations>,
}

impl ParameterLocations {
    pub(super) const fn value(self) -> ArgumentLocation {
        self.value
    }

    pub(super) const fn origin(self) -> Option<ObjectOriginLocations> {
        self.origin
    }

    pub(super) fn incoming(self) -> Option<Self> {
        Some(Self {
            value: self.value.incoming()?,
            origin: match self.origin {
                Some(origin) => Some(origin.incoming()?),
                None => None,
            },
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ObjectOriginLocations {
    complete: ArgumentLocation,
    metadata: ArgumentLocation,
}

impl ObjectOriginLocations {
    pub(super) const fn new(complete: ArgumentLocation, metadata: ArgumentLocation) -> Self {
        Self { complete, metadata }
    }

    pub(super) const fn complete(self) -> ArgumentLocation {
        self.complete
    }

    pub(super) const fn metadata(self) -> ArgumentLocation {
        self.metadata
    }

    fn incoming(self) -> Option<Self> {
        Some(Self {
            complete: self.complete.incoming()?,
            metadata: self.metadata.incoming()?,
        })
    }
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
            MirType::I64
            | MirType::U64
            | MirType::U8
            | MirType::Bool
            | MirType::Class(_)
            | MirType::Shared(_)
            | MirType::OptionalShared(_)
            | MirType::OptionalPrimitive(_)
            | MirType::OptionalClass(_)
            | MirType::Array(_) => Some(ScalarClass::Integer),
            MirType::F64 => Some(ScalarClass::Sse),
            MirType::Interface(_) | MirType::Obj | MirType::Unit => None,
        },
    }
}

const fn alias_carries_object_origin(parameter: MirParameter) -> bool {
    matches!(
        parameter.mode,
        MirParameterMode::ReadOnlyAlias | MirParameterMode::MutableAlias
    ) && matches!(
        parameter.ty,
        MirType::Class(_) | MirType::Interface(_) | MirType::Obj
    )
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
    receiver: Option<ObjectLocations>,
    locations: Vec<ParameterLocations>,
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
        let mut classifier = Classifier {
            integer_index: usize::from(has_return_destination),
            sse_index: 0,
            stack_count: 0,
        };
        let receiver = if has_receiver {
            Some(ObjectLocations {
                address: classifier.classify(ScalarClass::Integer)?,
                complete: classifier.classify(ScalarClass::Integer)?,
                metadata: classifier.classify(ScalarClass::Integer)?,
            })
        } else {
            None
        };
        let mut locations = Vec::with_capacity(parameters.len());

        for &parameter in parameters {
            let value = classifier.classify(parameter_class(parameter)?)?;
            let origin = if alias_carries_object_origin(parameter) {
                Some(ObjectOriginLocations {
                    complete: classifier.classify(ScalarClass::Integer)?,
                    metadata: classifier.classify(ScalarClass::Integer)?,
                })
            } else {
                None
            };
            locations.push(ParameterLocations { value, origin });
        }

        let bytes = classifier.stack_count.checked_mul(STACK_SLOT_SIZE)?;
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

    #[cfg(test)]
    pub(super) fn receiver(&self) -> Option<ArgumentLocation> {
        self.receiver.map(ObjectLocations::address)
    }

    pub(super) const fn receiver_locations(&self) -> Option<ObjectLocations> {
        self.receiver
    }

    #[cfg(test)]
    pub(super) fn locations(&self) -> Vec<ArgumentLocation> {
        self.locations
            .iter()
            .map(|locations| locations.value())
            .collect()
    }

    pub(super) fn parameter_locations(&self) -> &[ParameterLocations] {
        &self.locations
    }

    pub(super) const fn stack_size(&self) -> u32 {
        self.stack_size
    }
}

struct Classifier {
    integer_index: usize,
    sse_index: usize,
    stack_count: usize,
}

impl Classifier {
    fn classify(&mut self, class: ScalarClass) -> Option<ArgumentLocation> {
        let location = match class {
            ScalarClass::Sse if self.sse_index < SSE_ARGUMENT_REGISTERS.len() => {
                let register = SSE_ARGUMENT_REGISTERS[self.sse_index];
                self.sse_index += 1;
                ArgumentLocation::SseRegister(register)
            }
            ScalarClass::Sse => stack_location(self.stack_count)?,
            ScalarClass::Integer if self.integer_index < INTEGER_ARGUMENT_REGISTERS.len() => {
                let register = INTEGER_ARGUMENT_REGISTERS[self.integer_index];
                self.integer_index += 1;
                ArgumentLocation::IntegerRegister(register)
            }
            ScalarClass::Integer => stack_location(self.stack_count)?,
        };
        if matches!(location, ArgumentLocation::Stack(_)) {
            self.stack_count += 1;
        }
        Some(location)
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
    fn classifies_shared_handles_as_one_integer_component() {
        let layout = CallLayout::classify(&MirParameter::values([MirType::Shared(
            crate::mir::MirSharedTarget::Obj,
        )]))
        .unwrap();

        assert_eq!(
            layout.locations(),
            [ArgumentLocation::IntegerRegister(Register::Rdi)]
        );
        assert_eq!(layout.stack_size(), 0);
    }

    #[test]
    fn classifies_optional_aggregate_addresses_as_integer_arguments() {
        let layout = CallLayout::classify_internal_call(
            &MirParameter::values([
                MirType::OptionalPrimitive(crate::mir::MirPrimitiveType::I64),
                MirType::OptionalPrimitive(crate::mir::MirPrimitiveType::F64),
            ]),
            false,
            true,
        )
        .unwrap();

        assert_eq!(
            layout.return_destination(),
            Some(ArgumentLocation::IntegerRegister(Register::Rdi))
        );
        assert_eq!(
            layout.locations(),
            [
                ArgumentLocation::IntegerRegister(Register::Rsi),
                ArgumentLocation::IntegerRegister(Register::Rdx),
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
    fn hidden_receiver_carries_address_complete_object_and_metadata() {
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
                ArgumentLocation::IntegerRegister(Register::Rcx),
                ArgumentLocation::SseRegister(XmmRegister::Xmm0),
                ArgumentLocation::IntegerRegister(Register::R8),
                ArgumentLocation::SseRegister(XmmRegister::Xmm1),
            ]
        );
        assert_eq!(
            layout.receiver_locations(),
            Some(ObjectLocations {
                address: ArgumentLocation::IntegerRegister(Register::Rdi),
                complete: ArgumentLocation::IntegerRegister(Register::Rsi),
                metadata: ArgumentLocation::IntegerRegister(Register::Rdx),
            })
        );
    }

    #[test]
    fn receiver_layout_preserves_independent_exhaustion_and_stack_order() {
        let mut types = vec![MirType::I64; 5];
        types.extend([MirType::F64; 8]);
        types.extend([MirType::I64, MirType::F64]);
        let layout = CallLayout::classify_with_receiver(&MirParameter::values(types)).unwrap();

        assert_eq!(layout.locations()[13], ArgumentLocation::Stack(16));
        assert_eq!(layout.locations()[14], ArgumentLocation::Stack(24));
        assert_eq!(layout.stack_size(), 32);
    }

    #[test]
    fn alias_addresses_and_origins_are_integer_class() {
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
                ArgumentLocation::IntegerRegister(Register::Rcx),
            ]
        );
        assert_eq!(
            layout.parameter_locations()[0].origin(),
            Some(ObjectOriginLocations {
                complete: ArgumentLocation::IntegerRegister(Register::Rsi),
                metadata: ArgumentLocation::IntegerRegister(Register::Rdx),
            })
        );
    }

    #[test]
    fn optional_container_aliases_carry_only_the_container_address() {
        let layout = CallLayout::classify(&[
            MirParameter::read_only_alias(MirType::OptionalPrimitive(
                crate::mir::MirPrimitiveType::I64,
            )),
            MirParameter::mutable_alias(MirType::OptionalClass(crate::identity::ClassId::new(0))),
        ])
        .unwrap();

        assert_eq!(
            layout.locations(),
            [
                ArgumentLocation::IntegerRegister(Register::Rdi),
                ArgumentLocation::IntegerRegister(Register::Rsi),
            ]
        );
        assert!(layout
            .parameter_locations()
            .iter()
            .all(|location| location.origin().is_none()));
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

        assert_eq!(layout.locations()[14], ArgumentLocation::Stack(96));
        assert_eq!(layout.locations()[15], ArgumentLocation::Stack(120));
        assert_eq!(layout.stack_size(), 128);
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
        assert_eq!(layout.locations()[13], ArgumentLocation::Stack(96));
        assert_eq!(layout.locations()[14], ArgumentLocation::Stack(120));
        assert_eq!(layout.stack_size(), 128);
    }
}
