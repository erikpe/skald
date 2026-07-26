use super::super::{
    super::model::{
        MirAliasAccess, MirArrayAnchorKind, MirArrayInstruction, MirBasicBlock, MirDefinitionRef,
        MirPlaceBase, MirStorageKind, MirType,
    },
    context::Verifier,
};

impl Verifier<'_> {
    pub(in crate::mir::verify) fn verify_array_anchor_instruction(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        instruction: &MirArrayInstruction,
    ) {
        match instruction {
            MirArrayInstruction::AnchorBegin {
                anchor,
                owner,
                array,
                kind,
                ..
            } => {
                let kind_matches_owner = match kind {
                    MirArrayAnchorKind::InlineOwner | MirArrayAnchorKind::InlineBacking => {
                        !matches!(owner.base, MirPlaceBase::SharedPointee(_))
                            || !owner.projections.is_empty()
                    }
                    MirArrayAnchorKind::StableSharedOwner
                    | MirArrayAnchorKind::CopiedSharedOwner
                    | MirArrayAnchorKind::AdoptedSharedOwner
                    | MirArrayAnchorKind::SecuredOptionalSharedOwner => {
                        matches!(owner.base, MirPlaceBase::SharedPointee(_))
                            && owner.projections.is_empty()
                    }
                };
                let valid = kind_matches_owner
                    && function.storage(*anchor).is_some_and(|storage| {
                        storage.kind == MirStorageKind::ArrayAnchor(*kind)
                            && storage.ty == MirType::Array(*array)
                    })
                    && self
                        .verify_place(function, block, owner)
                        .map(|place| place.ty)
                        == Some(MirType::Array(*array));
                if !valid {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array anchor has incompatible storage, kind, owner, or type",
                    );
                }
            }
            MirArrayInstruction::AnchorEnd { anchor, .. }
                if !function.storage(*anchor).is_some_and(|storage| {
                    matches!(storage.kind, MirStorageKind::ArrayAnchor(_))
                }) =>
            {
                self.block_error(
                    function.callable(),
                    block.id,
                    "array anchor end names non-anchor storage",
                );
            }
            MirArrayInstruction::AliasBind {
                alias,
                source,
                anchor,
                ..
            } => {
                let source = self.verify_place(function, block, source);
                let valid = function.storage(*alias).is_some_and(|storage| {
                    let access = match storage.kind {
                        MirStorageKind::ArrayAlias(access) => access,
                        _ => return false,
                    };
                    source.as_ref().is_some_and(|source| {
                        source.ty == storage.ty
                            && (access == MirAliasAccess::ReadOnly
                                || source.access == MirAliasAccess::Mutable)
                    })
                }) && function
                    .storage(*anchor)
                    .is_some_and(|storage| matches!(storage.kind, MirStorageKind::ArrayAnchor(_)));
                if !valid {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array alias binding has incompatible carrier, source, or anchor",
                    );
                }
            }
            _ => {}
        }
    }
}
