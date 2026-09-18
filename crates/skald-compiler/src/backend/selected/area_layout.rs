//! Explicit target slot layout, checked against independent logical area shapes.
use super::Representation;
use crate::backend::plan::PlanError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) struct AbiSlotLayout {
    pub offset: usize,
    pub bytes: usize,
    pub alignment: usize,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::backend) struct AbiAreaLayout {
    pub slots: Vec<AbiSlotLayout>,
    pub bytes: usize,
    pub alignment: usize,
}
impl AbiAreaLayout {
    pub(super) fn validate(&self, shape: &[Representation]) -> Result<(), PlanError> {
        if !self.alignment.is_power_of_two()
            || self.slots.len() != shape.len()
            || self.bytes % self.alignment != 0
        {
            return Err(PlanError::InvalidSignature);
        }
        let mut end = 0;
        for (slot, representation) in self.slots.iter().zip(shape) {
            if !slot.alignment.is_power_of_two()
                || slot.alignment > self.alignment
                || slot.offset % slot.alignment != 0
                || slot.bytes < usize::from(representation.bits()).div_ceil(8)
                || slot.offset < end
            {
                return Err(PlanError::InvalidSignature);
            }
            end = slot
                .offset
                .checked_add(slot.bytes)
                .ok_or(PlanError::InvalidSignature)?;
            if end > self.bytes {
                return Err(PlanError::InvalidSignature);
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::selected::RepresentationKind;
    #[test]
    fn slot_shape_extent_alignment_overlap_and_overflow_are_independent() {
        let shape = vec![Representation::new(RepresentationKind::Bits, 8).unwrap(); 2];
        let good = AbiAreaLayout {
            slots: vec![
                AbiSlotLayout {
                    offset: 0,
                    bytes: 8,
                    alignment: 8,
                },
                AbiSlotLayout {
                    offset: 8,
                    bytes: 8,
                    alignment: 8,
                },
            ],
            bytes: 16,
            alignment: 16,
        };
        assert!(good.validate(&shape).is_ok());
        for case in 0..7 {
            let mut bad = good.clone();
            match case {
                0 => bad.bytes = 2,
                1 => bad.alignment = 3,
                2 => bad.slots[1].offset = 0,
                3 => bad.slots[1].offset = 9,
                4 => bad.slots[0].bytes = 0,
                5 => bad.slots[1].offset = usize::MAX - 7,
                _ => {
                    bad.slots.pop();
                }
            }
            assert!(bad.validate(&shape).is_err());
        }
    }
}
