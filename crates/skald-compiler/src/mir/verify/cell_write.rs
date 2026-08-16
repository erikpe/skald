//! Independent verification of read-only whole-field write authorization.

use super::{
    super::model::{
        MirAliasAccess, MirBasicBlock, MirCellWriteAuthorization, MirDefinitionRef, MirPlace,
        MirPlaceProjection, MirType,
    },
    context::Verifier,
};

#[derive(Clone, Copy)]
pub(super) enum CellWriteFamily {
    Scalar,
    Class,
    Optional,
    Shared,
    Array,
}

#[derive(Clone, Copy)]
pub(super) enum VerifiedWriteAccess {
    Ordinary,
    Cell,
}

impl VerifiedWriteAccess {
    pub(super) const fn from_cell_authorized(authorized: bool) -> Self {
        if authorized {
            Self::Cell
        } else {
            Self::Ordinary
        }
    }

    pub(super) const fn allows_read_only(self) -> bool {
        matches!(self, Self::Cell)
    }
}

impl CellWriteFamily {
    const fn accepts(self, ty: MirType) -> bool {
        match self {
            Self::Scalar => ty.is_scalar_value(),
            Self::Class => matches!(ty, MirType::Class(_)),
            Self::Optional => matches!(ty, MirType::Optional(_)),
            Self::Shared => matches!(ty, MirType::Shared(_)),
            Self::Array => matches!(ty, MirType::Array(_)),
        }
    }
}

impl Verifier<'_> {
    /// Returns whether valid cell evidence authorizes a read-only destination.
    /// Ordinary mutable destinations carry no evidence and continue through
    /// their existing access checks.
    pub(super) fn verify_cell_write_authorization(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination: &MirPlace,
        authorization: Option<MirCellWriteAuthorization>,
        family: CellWriteFamily,
    ) -> bool {
        let Some(authorization) = authorization else {
            return false;
        };
        let endpoint_matches = matches!(
            destination.projections.last(),
            Some(MirPlaceProjection::Field(field)) if *field == authorization.field
        );
        let declaration = self.program.field(authorization.field);
        let valid = endpoint_matches
            && declaration.is_some_and(|field| {
                field.cell_span.is_some()
                    && family.accepts(field.ty)
                    && function.class_owner() == Some(field.id.class())
            })
            && self
                .verify_place(function, block, destination)
                .is_some_and(|place| place.access == MirAliasAccess::ReadOnly);
        if !valid {
            self.block_error(
                function.callable(),
                block.id,
                "cell write authorization does not match the exact read-only declaring-class field destination",
            );
        }
        valid
    }
}
