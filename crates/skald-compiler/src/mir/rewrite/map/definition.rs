//! Callable definitions, attachments, and local declaration tables.

macro_rules! define_definition_traversal {
    (($($mir_mutability:tt)*)) => {
        pub(crate) fn map_function_local_identities<M: MirLocalIdentityMapper>(
            definition: &$($mir_mutability)* MirFunctionDefinition,
            mapper: &mut M,
        ) -> Result<(), M::Error> {
            let MirFunctionDefinition {
                function: _,
                return_storage,
                parameters,
                storage,
                values,
                body,
                span: _,
            } = definition;
            map_function_attachments(return_storage, parameters, mapper)?;
            map_common_local_identities(storage, values, body, mapper)
        }

        pub(crate) fn map_member_local_identities<M: MirLocalIdentityMapper>(
            definition: &$($mir_mutability)* MirMemberDefinition,
            mapper: &mut M,
        ) -> Result<(), M::Error> {
            let MirMemberDefinition {
                callable: _,
                class_owner: _,
                return_storage,
                receiver,
                parameters,
                storage,
                values,
                body,
                span: _,
            } = definition;
            map_member_attachments(return_storage, receiver, parameters, mapper)?;
            map_common_local_identities(storage, values, body, mapper)
        }

        pub(crate) fn map_static_initializer_local_identities<M: MirLocalIdentityMapper>(
            definition: &$($mir_mutability)* MirStaticInitializerBody,
            mapper: &mut M,
        ) -> Result<(), M::Error> {
            let MirStaticInitializerBody {
                id: _,
                field: _,
                destination_type: _,
                publication,
                storage,
                values,
                body,
                span: _,
            } = definition;
            map_static_publication_attachment(publication, mapper)?;
            map_common_local_identities(storage, values, body, mapper)
        }

        pub(crate) fn map_function_attachments<M: MirLocalIdentityMapper>(
            return_storage: &$($mir_mutability)* Option<StorageId>,
            parameters: &$($mir_mutability)* [StorageId],
            mapper: &mut M,
        ) -> Result<(), M::Error> {
            map_optional_storage(
                mapper,
                MirLocalIdentitySite::ReturnStorage,
                MirStorageUseRole::Attachment,
                return_storage,
            )?;
            map_parameters(parameters, mapper)
        }

        pub(crate) fn map_member_attachments<M: MirLocalIdentityMapper>(
            return_storage: &$($mir_mutability)* Option<StorageId>,
            receiver: &$($mir_mutability)* Option<StorageId>,
            parameters: &$($mir_mutability)* [StorageId],
            mapper: &mut M,
        ) -> Result<(), M::Error> {
            map_optional_storage(
                mapper,
                MirLocalIdentitySite::ReturnStorage,
                MirStorageUseRole::Attachment,
                return_storage,
            )?;
            map_optional_storage(
                mapper,
                MirLocalIdentitySite::Receiver,
                MirStorageUseRole::Attachment,
                receiver,
            )?;
            map_parameters(parameters, mapper)
        }

        pub(crate) fn map_static_publication_attachment<M: MirLocalIdentityMapper>(
            publication: &$($mir_mutability)* MirStaticPublication,
            mapper: &mut M,
        ) -> Result<(), M::Error> {
            let MirStaticPublication {
                initialization_exit,
                cleanup_entry,
                span: _,
            } = publication;
            map_block(
                mapper,
                MirLocalIdentitySite::StaticPublicationInitializationExit,
                initialization_exit,
            )?;
            map_block(
                mapper,
                MirLocalIdentitySite::StaticPublicationCleanupEntry,
                cleanup_entry,
            )
        }

        fn map_parameters<M: MirLocalIdentityMapper>(
            parameters: &$($mir_mutability)* [StorageId],
            mapper: &mut M,
        ) -> Result<(), M::Error> {
            for (index, parameter) in parameters.into_iter().enumerate() {
                map_storage_use(
                    mapper,
                    MirLocalIdentitySite::Parameter(index),
                    MirStorageUseRole::Attachment,
                    parameter,
                )?;
            }
            Ok(())
        }

        pub(crate) fn map_common_local_identities<M: MirLocalIdentityMapper>(
            storage: &$($mir_mutability)* [MirStorage],
            values: &$($mir_mutability)* [MirValue],
            body: &$($mir_mutability)* MirBody,
            mapper: &mut M,
        ) -> Result<(), M::Error> {
            for (index, declaration) in storage.into_iter().enumerate() {
                let MirStorage {
                    id,
                    source: _,
                    name: _,
                    kind: _,
                    ty: _,
                    span: _,
                } = declaration;
                map_storage_use(
                    mapper,
                    MirLocalIdentitySite::StorageDeclaration(index),
                    MirStorageUseRole::Declaration,
                    id,
                )?;
            }
            for (index, declaration) in values.into_iter().enumerate() {
                let MirValue { id, ty: _, span: _ } = declaration;
                map_value(mapper, MirLocalIdentitySite::ValueDeclaration(index), id)?;
            }
            map_body_local_identities(body, mapper)
        }
    };
}

pub(super) use define_definition_traversal;
