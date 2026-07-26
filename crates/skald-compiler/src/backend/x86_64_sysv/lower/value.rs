//! Stack-home movement and canonical representation selection.

use crate::{
    backend::BackendError,
    mir::{MirPlace, MirStore, MirType, StorageId, ValueId},
};

use super::{
    super::frame::FramePlace,
    super::machine::{ByteRegister, FloatOperand, Instruction, Operand, Register, XmmRegister},
    FrameLayout, InstructionSelector,
};

impl InstructionSelector<'_, '_> {
    pub(super) fn select_store(&mut self, store: &MirStore) -> Result<(), BackendError> {
        let (destination_layout, destination) = self.frame_place(&store.destination)?;
        let ty = destination_layout.ty();
        let source = frame_value(self.frame, store.value);

        if ty == MirType::F64 {
            load_float(float_operand(source), XmmRegister::Xmm14, self.output);
            store_float(XmmRegister::Xmm14, float_operand(destination), self.output);
        } else {
            load_rax(source, self.output);
            if destination_layout.uses_byte_access() {
                self.output.push(Instruction::MoveByte {
                    source: ByteRegister::Al,
                    destination,
                });
            } else {
                store_canonical_rax(ty, destination, self.output);
            }
        }
        Ok(())
    }

    pub(super) fn frame_place(
        &mut self,
        place: &MirPlace,
    ) -> Result<(FramePlace, Operand), BackendError> {
        if place.projections.iter().any(|projection| {
            matches!(
                projection,
                crate::mir::MirPlaceProjection::ArrayElement { .. }
            )
        }) {
            return self.select_array_element_place(place);
        }
        let layout = self
            .frame
            .place(self.program, self.function, self.data_layout, place)?;
        let base = match layout.base().pointer_home() {
            None => Register::Rbp,
            Some(home) => {
                load_rax(memory(Register::Rbp, home), self.output);
                self.output.push(Instruction::Move {
                    source: Register::Rax.into(),
                    destination: Register::R11.into(),
                });
                Register::R11
            }
        };
        let operand = memory(base, layout.displacement());
        Ok((layout, operand))
    }

    pub(super) fn materialize_place_address(
        &mut self,
        place: &MirPlace,
        destination: Register,
    ) -> Result<(), BackendError> {
        if place.projections.iter().any(|projection| {
            matches!(
                projection,
                crate::mir::MirPlaceProjection::ArrayElement { .. }
            )
        }) {
            let (_, operand) = self.frame_place(place)?;
            self.output.push(Instruction::LoadEffectiveAddress {
                source: operand,
                destination,
            });
            return Ok(());
        }
        let layout = self
            .frame
            .place(self.program, self.function, self.data_layout, place)?;
        match layout.base().pointer_home() {
            None => self.output.push(Instruction::LoadEffectiveAddress {
                source: memory(Register::Rbp, layout.displacement()),
                destination,
            }),
            Some(home) => {
                self.output.push(Instruction::Move {
                    source: memory(Register::Rbp, home),
                    destination: destination.into(),
                });
                if layout.displacement() != 0 {
                    self.output.push(Instruction::LoadEffectiveAddress {
                        source: memory(destination, layout.displacement()),
                        destination,
                    });
                }
            }
        }
        Ok(())
    }
}

pub(super) fn load_rax(source: Operand, output: &mut Vec<Instruction>) {
    output.push(Instruction::Move {
        source,
        destination: Register::Rax.into(),
    });
}

pub(super) fn load_byte_rax(source: Operand, output: &mut Vec<Instruction>) {
    output.push(Instruction::LoadZeroExtendByte {
        source,
        destination: Register::Rax,
    });
}

pub(super) fn load_float(
    source: FloatOperand,
    destination: XmmRegister,
    output: &mut Vec<Instruction>,
) {
    output.push(Instruction::MoveFloat64 {
        source,
        destination: destination.into(),
    });
}

pub(super) fn store_float(
    source: XmmRegister,
    destination: FloatOperand,
    output: &mut Vec<Instruction>,
) {
    output.push(Instruction::MoveFloat64 {
        source: source.into(),
        destination,
    });
}

/// Converts a MIR value in `rax` to its canonical full-register form.
///
/// `u8` values use eight-byte homes in the initial backend, but only their low
/// eight bits belong to the language value. Every producer and ABI ingress
/// reaches this helper before the value is stored or returned.
pub(super) fn canonicalize_rax(ty: MirType, output: &mut Vec<Instruction>) {
    if ty == MirType::U8 {
        output.push(Instruction::ZeroExtendByte {
            source: ByteRegister::Al,
            destination: Register::Rax,
        });
    }
}

pub(super) fn store_canonical_rax(
    ty: MirType,
    destination: Operand,
    output: &mut Vec<Instruction>,
) {
    canonicalize_rax(ty, output);
    store_rax(destination, output);
}

pub(super) fn store_rax(destination: Operand, output: &mut Vec<Instruction>) {
    output.push(Instruction::Move {
        source: Register::Rax.into(),
        destination,
    });
}

pub(super) fn frame_storage(frame: &FrameLayout, storage: StorageId) -> Operand {
    memory(Register::Rbp, frame.storage(storage))
}

pub(super) fn frame_value(frame: &FrameLayout, value: ValueId) -> Operand {
    memory(Register::Rbp, frame.value(value))
}

pub(super) fn memory(base: Register, displacement: i32) -> Operand {
    Operand::Memory { base, displacement }
}

pub(super) fn indexed_memory(
    base: Register,
    index: Register,
    scale: u8,
    displacement: i32,
) -> Operand {
    Operand::IndexedMemory {
        base,
        index,
        scale,
        displacement,
    }
}

pub(super) fn float_memory(base: Register, displacement: i32) -> FloatOperand {
    FloatOperand::Memory { base, displacement }
}

pub(super) fn float_operand(operand: Operand) -> FloatOperand {
    match operand {
        Operand::Memory { base, displacement } => float_memory(base, displacement),
        Operand::IndexedMemory {
            base,
            index,
            scale,
            displacement,
        } => FloatOperand::IndexedMemory {
            base,
            index,
            scale,
            displacement,
        },
        Operand::Register(_) => unreachable!("floating values use XMM registers"),
    }
}
