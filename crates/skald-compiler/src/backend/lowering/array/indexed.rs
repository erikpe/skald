//! Dynamic indexed-array construction epochs.

use super::{array_element, Lowerer};
use crate::{
    backend::{lir::Operation, plan::ScalarType},
    mir::{BlockId, MirPlace, StorageId, ValueId},
};

use crate::backend::{lir::Conversion, plan::PlanError};

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn begin_indexed_array(
        &mut self,
        block: BlockId,
        prefix: StorageId,
    ) -> Result<(), super::LowerError> {
        self.store_u64(block, prefix, 0)
    }

    pub(super) fn bind_indexed_array(
        &mut self,
        block: BlockId,
        prefix: StorageId,
        binding: StorageId,
    ) -> Result<(), super::LowerError> {
        let prefix = self.load_storage(block, prefix)?;
        let value = self.append(
            block,
            Operation::Convert {
                conversion: Conversion::IntegerBits,
                value: prefix,
                target: ScalarType::I64,
                evidence: None,
            },
        )?[0];
        if self.representation(binding)?.scalar != ScalarType::I64 {
            return Err(PlanError::InvalidDomain.into());
        }
        self.store(block, binding, value)
    }

    pub(super) fn initialize_indexed_element(
        &mut self,
        block: BlockId,
        backing: StorageId,
        prefix: StorageId,
        value: ValueId,
    ) -> Result<(), super::LowerError> {
        let array = self.array_for_backing(backing)?;
        let place = array_element(MirPlace::base(backing), array, prefix);
        let address = self.place_address(block, &place)?;
        let representation = self.place_representation(&place)?;
        self.append(
            block,
            Operation::Store {
                address,
                value: self.values[value.index()],
                representation,
            },
        )?;
        self.advance_index(block, prefix)
    }
}
