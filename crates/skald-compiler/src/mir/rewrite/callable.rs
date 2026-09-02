//! Owned adapters between public executable-definition shapes and common edit state.

use crate::{
    identity::{CallableId, ClassId, FunctionId, StaticFieldId, StaticInitializerId},
    source::Span,
};

use super::super::{
    MirFunctionDefinition, MirMemberDefinition, MirStaticInitializerBody, MirStaticPublication,
    MirType, StorageId,
};
use super::{
    commit::{commit_with_attachments, CommitMapper, MirCommitMaps, MirRewriteChangeSummary},
    edit::MirCallableEdit,
    error::MirRewriteError,
    map::{map_function_attachments, map_member_attachments, map_static_publication_attachment},
};

#[derive(Clone, Debug, Eq, PartialEq)]
enum MirCallableAttachments {
    Function {
        function: FunctionId,
        return_storage: Option<StorageId>,
        parameters: Vec<StorageId>,
        span: Span,
    },
    Member {
        callable: CallableId,
        class_owner: ClassId,
        return_storage: Option<StorageId>,
        receiver: Option<StorageId>,
        parameters: Vec<StorageId>,
        span: Span,
    },
    StaticInitializer {
        id: StaticInitializerId,
        field: StaticFieldId,
        destination_type: MirType,
        publication: MirStaticPublication,
        span: Span,
    },
}

impl MirCallableAttachments {
    const fn callable(&self) -> CallableId {
        match self {
            Self::Function { function, .. } => CallableId::Function(*function),
            Self::Member { callable, .. } => *callable,
            Self::StaticInitializer { id, .. } => CallableId::StaticInitializer(*id),
        }
    }

    fn map(&mut self, mapper: &mut CommitMapper<'_>) -> Result<(), MirRewriteError> {
        match self {
            Self::Function {
                function: _,
                return_storage,
                parameters,
                span: _,
            } => map_function_attachments(return_storage, parameters, mapper),
            Self::Member {
                callable: _,
                class_owner: _,
                return_storage,
                receiver,
                parameters,
                span: _,
            } => map_member_attachments(return_storage, receiver, parameters, mapper),
            Self::StaticInitializer {
                id: _,
                field: _,
                destination_type: _,
                publication,
                span: _,
            } => map_static_publication_attachment(publication, mapper),
        }
    }
}

/// One owned executable definition split into stable attachments and common
/// sparse edit state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MirCallablePackage {
    attachments: MirCallableAttachments,
    edit: MirCallableEdit,
}

impl MirCallablePackage {
    pub(super) fn from_function(
        definition: MirFunctionDefinition,
    ) -> Result<Self, MirRewriteError> {
        let MirFunctionDefinition {
            function,
            return_storage,
            parameters,
            storage,
            values,
            body,
            span,
        } = definition;
        let callable = CallableId::Function(function);
        Ok(Self {
            attachments: MirCallableAttachments::Function {
                function,
                return_storage,
                parameters,
                span,
            },
            edit: MirCallableEdit::from_dense_parts(callable, storage, values, body)?,
        })
    }

    pub(super) fn from_member(definition: MirMemberDefinition) -> Result<Self, MirRewriteError> {
        let MirMemberDefinition {
            callable,
            class_owner,
            return_storage,
            receiver,
            parameters,
            storage,
            values,
            body,
            span,
        } = definition;
        Ok(Self {
            attachments: MirCallableAttachments::Member {
                callable,
                class_owner,
                return_storage,
                receiver,
                parameters,
                span,
            },
            edit: MirCallableEdit::from_dense_parts(callable, storage, values, body)?,
        })
    }

    pub(super) fn from_static_initializer(
        definition: MirStaticInitializerBody,
    ) -> Result<Self, MirRewriteError> {
        let MirStaticInitializerBody {
            id,
            field,
            destination_type,
            publication,
            storage,
            values,
            body,
            span,
        } = definition;
        let callable = CallableId::StaticInitializer(id);
        let edit = MirCallableEdit::from_dense_parts(callable, storage, values, body)?
            .with_attachment_blocks([
                (
                    super::MirLocalIdentitySite::StaticPublicationInitializationExit,
                    publication.initialization_exit,
                ),
                (
                    super::MirLocalIdentitySite::StaticPublicationCleanupEntry,
                    publication.cleanup_entry,
                ),
            ]);
        Ok(Self {
            attachments: MirCallableAttachments::StaticInitializer {
                id,
                field,
                destination_type,
                publication,
                span,
            },
            edit,
        })
    }

    pub(super) const fn callable(&self) -> CallableId {
        self.attachments.callable()
    }

    pub(super) fn edit_mut(&mut self) -> &mut MirCallableEdit {
        &mut self.edit
    }

    pub(super) fn commit(self) -> Result<MirCommittedPackage, MirRewriteError> {
        let (common, attachments) =
            commit_with_attachments(self.edit, self.attachments, |attachments, mapper| {
                attachments.map(mapper)
            })?;
        debug_assert_eq!(common.callable.callable, attachments.callable());
        let definition = match attachments {
            MirCallableAttachments::Function {
                function,
                return_storage,
                parameters,
                span,
            } => MirCommittedDefinition::Function(MirFunctionDefinition {
                function,
                return_storage,
                parameters,
                storage: common.callable.storage,
                values: common.callable.values,
                body: common.callable.body,
                span,
            }),
            MirCallableAttachments::Member {
                callable,
                class_owner,
                return_storage,
                receiver,
                parameters,
                span,
            } => MirCommittedDefinition::Member(MirMemberDefinition {
                callable,
                class_owner,
                return_storage,
                receiver,
                parameters,
                storage: common.callable.storage,
                values: common.callable.values,
                body: common.callable.body,
                span,
            }),
            MirCallableAttachments::StaticInitializer {
                id,
                field,
                destination_type,
                publication,
                span,
            } => MirCommittedDefinition::StaticInitializer(MirStaticInitializerBody {
                id,
                field,
                destination_type,
                publication,
                storage: common.callable.storage,
                values: common.callable.values,
                body: common.callable.body,
                span,
            }),
        };
        Ok(MirCommittedPackage {
            definition,
            maps: common.maps,
            changes: common.changes,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum MirCommittedDefinition {
    Function(MirFunctionDefinition),
    Member(MirMemberDefinition),
    StaticInitializer(MirStaticInitializerBody),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MirCommittedPackage {
    pub(super) definition: MirCommittedDefinition,
    pub(super) maps: MirCommitMaps,
    pub(super) changes: MirRewriteChangeSummary,
}

#[cfg(test)]
mod tests;
