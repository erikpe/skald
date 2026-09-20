//! String descriptor publication from checked semantic layout facts.

use super::{context::Lowerer, LowerError};
use crate::{
    backend::{
        lir::{Constant, MemoryRepresentation, Operation},
        plan::{PlanError, ScalarType, SemanticType},
    },
    mir::{BlockId, MirStringInitialize},
};

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn string_initialize(
        &mut self,
        block: BlockId,
        initialize: &MirStringInitialize,
    ) -> Result<(), LowerError> {
        let destination = self.place_address(block, &initialize.destination)?;
        let backing = self.load_storage(block, initialize.backing)?;
        let string = self
            .plan()
            .semantic()
            .string
            .ok_or(PlanError::UnknownDeclaration)?;
        if string.class != initialize.class
            || string.storage_field != initialize.storage_field
            || string.start_field != initialize.start_field
            || string.length_field != initialize.length_field
            || string.hash_code_field != initialize.hash_code_field
        {
            return Err(PlanError::InvalidDomain.into());
        }
        let storage = self.string_field(
            initialize.storage_field,
            SemanticType::Shared(crate::backend::plan::SharedTarget::Array(
                string.storage_array,
            )),
        )?;
        self.store_at_offset(
            block,
            destination,
            storage.offset,
            backing,
            self.address_representation(),
        )?;

        let start = self.append(block, Operation::Constant(Constant::I64(initialize.start)))?[0];
        let start_field = self.string_field(initialize.start_field, SemanticType::I64)?;
        self.store_at_offset(
            block,
            destination,
            start_field.offset,
            start,
            self.scalar_field_representation(start_field.layout, ScalarType::I64)?,
        )?;

        let length = self.append(block, Operation::Constant(Constant::U64(initialize.length)))?[0];
        let length_field = self.string_field(initialize.length_field, SemanticType::U64)?;
        self.store_at_offset(
            block,
            destination,
            length_field.offset,
            length,
            self.scalar_field_representation(length_field.layout, ScalarType::U64)?,
        )?;

        let hash_field = self
            .plan()
            .semantic()
            .field(initialize.hash_code_field)
            .ok_or(PlanError::UnknownDeclaration)?;
        let SemanticType::Optional(optional) = hash_field.ty else {
            return Err(PlanError::InvalidDomain.into());
        };
        let state_offset = self
            .plan()
            .semantic()
            .optional(optional)
            .and_then(|fact| fact.state_offset)
            .ok_or(PlanError::InvalidLayout)?;
        let absent = self.append(block, Operation::Constant(Constant::U64(0)))?[0];
        self.store_at_offset(
            block,
            destination,
            hash_field
                .offset
                .checked_add(state_offset)
                .ok_or(PlanError::SizeOverflow)?,
            absent,
            self.count_representation(),
        )
    }

    fn string_field(
        &self,
        field: crate::identity::FieldId,
        ty: SemanticType,
    ) -> Result<crate::backend::plan::FieldLayoutFact, LowerError> {
        self.plan()
            .semantic()
            .field(field)
            .filter(|fact| fact.ty == ty)
            .ok_or_else(|| PlanError::InvalidLayout.into())
    }

    fn scalar_field_representation(
        &self,
        layout: crate::backend::plan::LayoutId,
        scalar: ScalarType,
    ) -> Result<MemoryRepresentation, LowerError> {
        let layout = self.plan().layout(self.plan().layout_id(layout.index())?)?;
        Ok(MemoryRepresentation {
            scalar,
            bytes: layout.size,
            alignment: layout.alignment,
        })
    }
}
