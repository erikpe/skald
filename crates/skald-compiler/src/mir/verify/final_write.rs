//! Independent verification of exact final-field lifecycle updates.

use super::{
    super::model::{
        MirBasicBlock, MirCellWriteAuthorization, MirCopyCapability, MirDefinitionRef,
        MirFinalWriteAuthorization, MirPlace, MirPlaceBase, MirPlaceProjection, MirStorageKind,
    },
    cell_write::{CellWriteFamily, VerifiedWriteAccess},
    context::Verifier,
};
use crate::identity::CallableId;

impl Verifier<'_> {
    /// Verifies both exceptional field-write capabilities without conflating
    /// them. Only cell evidence can relax destination access; final evidence
    /// proves lifecycle ownership of an otherwise mutable direct `self` field.
    pub(super) fn verify_field_write_authorizations(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination: &MirPlace,
        cell: Option<MirCellWriteAuthorization>,
        final_update: Option<MirFinalWriteAuthorization>,
        family: CellWriteFamily,
    ) -> VerifiedWriteAccess {
        let cell_authorized =
            self.verify_cell_write_authorization(function, block, destination, cell, family);
        self.verify_final_write_authorization(function, block, destination, final_update, family);
        if cell.is_some() && final_update.is_some() {
            self.block_error(
                function.callable(),
                block.id,
                "one field replacement cannot carry both cell and final-update authorization",
            );
        }
        VerifiedWriteAccess::from_cell_authorized(cell_authorized)
    }

    fn verify_final_write_authorization(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination: &MirPlace,
        authorization: Option<MirFinalWriteAuthorization>,
        family: CellWriteFamily,
    ) {
        let endpoint = match destination.projections.last() {
            Some(MirPlaceProjection::Field(field)) => self.program.field(*field),
            _ => None,
        };
        let endpoint_is_final = endpoint.is_some_and(|field| field.final_span.is_some());
        let Some(authorization) = authorization else {
            if endpoint_is_final
                && !self.is_direct_final_initialization(function, destination, family)
            {
                self.block_error(
                    function.callable(),
                    block.id,
                    "final field replacement lacks exact copy-assignment authorization",
                );
            }
            return;
        };

        let receiver_is_exact = match destination.base {
            MirPlaceBase::Storage(storage) => function
                .storage(storage)
                .is_some_and(|storage| storage.kind == MirStorageKind::Receiver),
            _ => false,
        };
        let direct_endpoint =
            destination.projections.as_slice() == [MirPlaceProjection::Field(authorization.field)];
        let selected_user_operation =
            self.program
                .class(authorization.field.class())
                .is_some_and(|class| {
                    matches!(
                        class.copy_assignment,
                        MirCopyCapability::User(ref copy)
                            if copy.operation == authorization.operation
                    )
                });
        let valid = endpoint_is_final
            && direct_endpoint
            && receiver_is_exact
            && function.callable() == authorization.operation.into()
            && authorization.operation.class() == authorization.field.class()
            && selected_user_operation
            && endpoint.is_some_and(|field| family.accepts(field.ty));
        if !valid {
            self.block_error(
                function.callable(),
                block.id,
                "final-update authorization does not match an exact direct field of the selected declaring-class copy assignment",
            );
        }
    }

    fn is_direct_final_initialization(
        &self,
        function: MirDefinitionRef<'_>,
        destination: &MirPlace,
        family: CellWriteFamily,
    ) -> bool {
        let class = match function.callable() {
            CallableId::Initializer(initializer) => initializer.class(),
            CallableId::CopyConstructor(copy) => copy.class(),
            CallableId::Function(_)
            | CallableId::StaticInitializer(_)
            | CallableId::CopyAssignment(_)
            | CallableId::Destructor(_)
            | CallableId::Method(_) => return false,
        };
        let receiver_is_exact = match destination.base {
            MirPlaceBase::Storage(storage) => function
                .storage(storage)
                .is_some_and(|storage| storage.kind == MirStorageKind::Receiver),
            _ => false,
        };
        let [MirPlaceProjection::Field(field)] = destination.projections.as_slice() else {
            return false;
        };
        receiver_is_exact
            && field.class() == class
            && self.program.field(*field).is_some_and(|declaration| {
                declaration.final_span.is_some() && family.accepts(declaration.ty)
            })
    }
}
