//! Checked observable schemas and conservative provenance/effect derivation.

use super::{
    BlockHandle, BuildError, Call, Constant, Conversion, DraftBuilder, DraftChecks, ObjectHandle,
    Operation, ValueHandle,
};
use crate::backend::effects::{Effect, Effects, MemoryRegion};
use crate::backend::graph::{LoweredObjectId, LoweredValueId};
use crate::backend::plan::{ArtifactCategory, ArtifactId, DataKey, ScalarType};
use crate::identity::StaticFieldId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum AddressProvenance {
    Object {
        object: LoweredObjectId,
        offset: usize,
    },
    Static {
        field: StaticFieldId,
        offset: usize,
    },
    Unknown,
}
#[cfg_attr(not(test), allow(dead_code))]
impl AddressProvenance {
    pub(super) fn region(self) -> MemoryRegion<LoweredObjectId> {
        match self {
            Self::Object { object, .. } => MemoryRegion::Object(object),
            Self::Static { field, .. } => MemoryRegion::Static(field),
            Self::Unknown => MemoryRegion::Unknown,
        }
    }
    pub(super) fn offset(self, bytes: i128) -> Self {
        let (offset, base) = match self {
            Self::Object { object, offset } => (offset, Some(object)),
            Self::Static { offset, .. } => (offset, None),
            Self::Unknown => return Self::Unknown,
        };
        let Some(offset) = (offset as i128)
            .checked_add(bytes)
            .and_then(|offset| usize::try_from(offset).ok())
        else {
            return Self::Unknown;
        };
        match (base, self) {
            (Some(object), _) => Self::Object { object, offset },
            (_, Self::Static { field, .. }) => Self::Static { field, offset },
            _ => Self::Unknown,
        }
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p> DraftChecks<'_, 'p> {
    pub(super) fn operation_effects(
        &self,
        operation: &Operation,
    ) -> Result<Effects<LoweredObjectId>, BuildError> {
        self.operation_effects_for(operation, |value| {
            Ok(self.draft.values.get_id(value)?.provenance)
        })
    }
    pub(super) fn operation_effects_for(
        &self,
        operation: &Operation,
        provenance: impl Fn(LoweredValueId) -> Result<AddressProvenance, BuildError>,
    ) -> Result<Effects<LoweredObjectId>, BuildError> {
        Ok(match operation {
            Operation::Load { address, .. } => {
                Effects::new([Effect::Read(provenance(*address)?.region())])
            }
            Operation::Store { address, .. } => {
                Effects::new([Effect::Write(provenance(*address)?.region())])
            }
            Operation::Call(call) => self.call_effects(call)?,
            Operation::Trace(_) => Effects::new([
                Effect::TraceState,
                Effect::Read(MemoryRegion::Unknown),
                Effect::Write(MemoryRegion::Unknown),
            ]),
            _ => Effects::default(),
        })
    }
    pub(super) fn result_provenance(
        &self,
        operation: &Operation,
    ) -> Result<AddressProvenance, BuildError> {
        Ok(match operation {
            Operation::ObjectAddress(object) => AddressProvenance::Object {
                object: *object,
                offset: 0,
            },
            Operation::SymbolAddress {
                symbol: ArtifactId::Data(DataKey::Static(field)),
                ..
            } => AddressProvenance::Static {
                field: *field,
                offset: 0,
            },
            Operation::Convert {
                conversion: Conversion::Identity,
                value,
                ..
            } => self.draft.values.get_id(*value)?.provenance,
            Operation::ByteOffset { base, offset } => {
                self.constant_integer(*offset)
                    .map_or(AddressProvenance::Unknown, |offset| {
                        self.draft
                            .values
                            .get_id(*base)
                            .map_or(AddressProvenance::Unknown, |base| {
                                base.provenance.offset(offset)
                            })
                    })
            }
            Operation::ScaledIndex {
                base,
                index,
                stride,
            } => self
                .constant_integer(*index)
                .and_then(|index| index.checked_mul(stride.bytes() as i128))
                .map_or(AddressProvenance::Unknown, |offset| {
                    self.draft
                        .values
                        .get_id(*base)
                        .map_or(AddressProvenance::Unknown, |base| {
                            base.provenance.offset(offset)
                        })
                }),
            _ => AddressProvenance::Unknown,
        })
    }
    pub(super) fn check_failure_message(
        &self,
        call: &Call,
        reason: crate::backend::failure::FailureMessage,
    ) -> Result<(), BuildError> {
        let message = call.arguments.first().ok_or(BuildError::InvalidCall)?.value;
        let length = call.arguments.get(1).ok_or(BuildError::InvalidCall)?.value;
        if self.constant_integer(length) != Some(reason.bytes().len() as i128) {
            return Err(BuildError::InvalidCall);
        }
        let Some(super::Definition::InstructionResult {
            instruction,
            ordinal: 0,
        }) = self.draft.values.get_id(message)?.definition
        else {
            return Err(BuildError::InvalidCall);
        };
        let block = self.draft.blocks.get_id(instruction.block)?;
        if !matches!(block.instructions.get(instruction.ordinal).map(|instruction| &instruction.operation), Some(Operation::SymbolAddress { symbol: ArtifactId::Data(DataKey::FailureMessage(actual)), ty: ScalarType::DataAddress }) if *actual == reason)
        {
            return Err(BuildError::InvalidCall);
        }
        Ok(())
    }
    pub(super) fn constant_integer(&self, value: LoweredValueId) -> Option<i128> {
        let definition = self.draft.values.get_id(value).ok()?.definition?;
        let super::Definition::InstructionResult {
            instruction,
            ordinal: 0,
        } = definition
        else {
            return None;
        };
        let block = self.draft.blocks.get_id(instruction.block).ok()?;
        match block.instructions.get(instruction.ordinal)?.operation {
            Operation::Constant(Constant::I64(value)) => Some(value as i128),
            Operation::Constant(Constant::U64(value)) => Some(value as i128),
            Operation::Constant(Constant::U8(value)) => Some(value as i128),
            _ => None,
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'p> DraftBuilder<'p> {
    pub(in crate::backend) fn append_with_effects(
        &mut self,
        block: BlockHandle<'p>,
        operation: Operation<ValueHandle<'p>, ObjectHandle<'p>, BlockHandle<'p>>,
        effects: Effects<LoweredObjectId>,
    ) -> Result<Vec<ValueHandle<'p>>, BuildError> {
        self.writable(block)?;
        let (normalized, _) = self.normalize(operation.clone())?;
        let required = self.operation_effects(&normalized)?;
        if !effects.covers(&required) {
            return Err(BuildError::NarrowedEffects);
        }
        for effect in effects.iter() {
            match effect {
                Effect::Read(MemoryRegion::Object(object))
                | Effect::Write(MemoryRegion::Object(object)) => {
                    self.draft.objects.get_id(*object)?;
                }
                Effect::Read(MemoryRegion::Static(field))
                | Effect::Write(MemoryRegion::Static(field)) => {
                    let view = self.draft.owner.context();
                    view.artifact(
                        view.artifact_id(ArtifactId::Data(DataKey::Static(*field)))?,
                        ArtifactCategory::Data,
                    )?;
                }
                _ => {}
            }
        }
        let ordinal = self.draft.blocks.get(block)?.instructions.len();
        let results = self.append(block, operation)?;
        self.draft.blocks.get_mut(block)?.instructions[ordinal].effects = effects;
        Ok(results)
    }
}
