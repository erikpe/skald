//! String language-item, literal-data, and publication verification.

use super::{
    super::model::{
        MirAliasAccess, MirBasicBlock, MirCopyCapability, MirDefinitionRef, MirPlaceBase,
        MirSharedStatic, MirSharedTarget, MirStaticAllocationOrigin, MirStaticDataMutability,
        MirStorageKind, MirStringInitialize, MirType,
    },
    context::Verifier,
};

impl Verifier<'_> {
    pub(super) fn verify_string_declarations(&mut self) {
        let item = self.program.string_language_item;
        if item.is_none() && self.program.literal_data.is_empty() {
            return;
        }
        let Some(item) = item else {
            self.program_error("literal data requires string language-item metadata");
            return;
        };
        let Some(class) = self.program.class(item.class) else {
            self.program_error("string language-item class is not declared");
            return;
        };
        if class.direct_base.is_some() {
            self.program_error("string language-item class must be a root class");
        }
        let expected = [
            (
                item.storage_field,
                MirType::Shared(MirSharedTarget::Array(item.storage_array)),
            ),
            (item.start_field, MirType::U64),
            (item.length_field, MirType::U64),
        ];
        if class.fields.len() != expected.len()
            || class
                .fields
                .iter()
                .zip(expected)
                .any(|(field, (id, ty))| field.id != id || field.ty != ty)
        {
            self.program_error(
                "string language-item fields must be the exact shared u8[]/u64/u64 descriptor",
            );
        }
        if self
            .program
            .array_type(item.storage_array)
            .is_none_or(|array| array.element != MirType::U8)
        {
            self.program_error("string language-item storage array must have u8 elements");
        }
        let constructor_is_exact = matches!(
            &class.copy_constructor,
            MirCopyCapability::Synthesized(copy)
                if copy.class == item.class
                    && copy.base.is_none()
        );
        let assignment_is_exact = matches!(
            &class.copy_assignment,
            MirCopyCapability::Synthesized(copy)
                if copy.class == item.class
                    && copy.base.is_none()
        );
        if !constructor_is_exact || !assignment_is_exact || class.destruction.destructor.is_some() {
            self.program_error(
                "string language-item class must retain its exact synthesized descriptor lifecycle",
            );
        }
        for (index, data) in self.program.literal_data.iter().enumerate() {
            if data.id.index() != index {
                self.program_error(format!(
                    "literal-data table index {index} contains {}",
                    data.id
                ));
            }
            if data.array != item.storage_array {
                self.program_error(format!(
                    "literal data {} does not use the string storage-array identity",
                    data.id
                ));
            }
            if data.length != u64::try_from(data.bytes.len()).unwrap_or(u64::MAX) {
                self.program_error(format!(
                    "literal data {} length does not match its exact byte payload",
                    data.id
                ));
            }
            if data.mutability != MirStaticDataMutability::Immutable {
                self.program_error(format!("literal data {} is not immutable", data.id));
            }
            if data.origin != MirStaticAllocationOrigin::Immortal {
                self.program_error(format!("literal data {} is not immortal", data.id));
            }
        }
    }

    pub(super) fn verify_shared_static(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        static_owner: &MirSharedStatic,
    ) {
        let expected_target = self
            .program
            .literal_data
            .get(static_owner.data)
            .map(|data| MirSharedTarget::Array(data.array));
        if expected_target.is_none() {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "static shared owner references undeclared literal data {}",
                    static_owner.data
                ),
            );
        }
        if expected_target != Some(static_owner.target) {
            self.block_error(
                function.callable(),
                block.id,
                "static shared owner target does not match its literal data",
            );
        }
        if static_owner.origin != MirStaticAllocationOrigin::Immortal {
            self.block_error(
                function.callable(),
                block.id,
                "static shared owner must have immortal provenance",
            );
        }
        if function
            .storage(static_owner.destination)
            .is_none_or(|storage| {
                storage.kind != MirStorageKind::Temporary
                    || storage.ty != MirType::Shared(static_owner.target)
            })
        {
            self.block_error(
                function.callable(),
                block.id,
                "static shared owner destination must be a fresh exact shared temporary",
            );
        }
    }

    pub(super) fn verify_string_initialize(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        initialize: &MirStringInitialize,
    ) {
        let Some(item) = self.program.string_language_item else {
            self.block_error(
                function.callable(),
                block.id,
                "string initialization requires language-item metadata",
            );
            return;
        };
        let data = self.program.literal_data.get(initialize.data);
        if data.is_none() {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "string initialization references undeclared literal data {}",
                    initialize.data
                ),
            );
        }
        if initialize.class != item.class
            || initialize.storage_field != item.storage_field
            || initialize.start_field != item.start_field
            || initialize.length_field != item.length_field
        {
            self.block_error(
                function.callable(),
                block.id,
                "string initialization does not use the exact language-item identities",
            );
        }
        if initialize.start != 0 || data.is_none_or(|data| initialize.length != data.length) {
            self.block_error(
                function.callable(),
                block.id,
                "string initialization has invalid start or length metadata",
            );
        }
        let destination = self.verify_place(function, block, &initialize.destination);
        if destination.is_none_or(|place| {
            place.ty != MirType::Class(item.class)
                || place.access != MirAliasAccess::Mutable
                || matches!(
                    initialize.destination.base,
                    MirPlaceBase::SharedPointee(_) | MirPlaceBase::SharedAllocationPayload(_)
                )
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "string initialization destination must be mutable owning string storage",
            );
        }
        if function.storage(initialize.backing).is_none_or(|storage| {
            storage.kind != MirStorageKind::Temporary
                || storage.ty != MirType::Shared(MirSharedTarget::Array(item.storage_array))
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "string initialization backing must be the exact shared u8[] temporary",
            );
        }
    }
}
