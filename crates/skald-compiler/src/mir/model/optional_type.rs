//! Canonical executable optional identities and target-independent lifecycle metadata.

use crate::{
    id_table::DenseIdTable,
    identity::{ArrayTypeId, ClassId, CopyAssignmentId, CopyConstructorId, OptionalTypeId},
};

use super::{MirSelectedCopyOperation, MirSharedTarget, MirType};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirOptionalTypeTable {
    entries: DenseIdTable<OptionalTypeId, MirOptionalType>,
}

impl MirOptionalTypeTable {
    pub(crate) fn new(entries: Vec<MirOptionalType>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: OptionalTypeId) -> Option<&MirOptionalType> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MirOptionalType> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [MirOptionalType] {
        self.entries.entries_mut_for_test()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalType {
    pub id: OptionalTypeId,
    pub payload: MirType,
    pub storage: MirOptionalStorage,
    pub representation: MirOptionalRepresentation,
    pub lifecycle: MirOptionalLifecycle,
    pub checked_access: MirOptionalCheckedAccess,
    pub boundaries: MirOptionalBoundaryPlans,
}

impl MirOptionalType {
    pub const fn primitive(&self) -> Option<super::MirPrimitiveType> {
        match (self.storage, self.payload) {
            (MirOptionalStorage::Scalar, MirType::I64) => Some(super::MirPrimitiveType::I64),
            (MirOptionalStorage::Scalar, MirType::U64) => Some(super::MirPrimitiveType::U64),
            (MirOptionalStorage::Scalar, MirType::U8) => Some(super::MirPrimitiveType::U8),
            (MirOptionalStorage::Scalar, MirType::F64) => Some(super::MirPrimitiveType::F64),
            (MirOptionalStorage::Scalar, MirType::Bool) => Some(super::MirPrimitiveType::Bool),
            _ => None,
        }
    }

    pub const fn inline_class(&self) -> Option<ClassId> {
        match self.storage {
            MirOptionalStorage::InlineClass(class) => Some(class),
            _ => None,
        }
    }

    pub const fn shared_owner(&self) -> Option<MirSharedTarget> {
        match self.storage {
            MirOptionalStorage::SharedOwner(target) => Some(target),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOptionalStorage {
    Scalar,
    InlineClass(ClassId),
    InlineArray(ArrayTypeId),
    SharedOwner(MirSharedTarget),
    Nested(OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOptionalRepresentation {
    TaggedPayload,
    NullableSharedOwner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirOptionalLifecycle {
    pub initialization: MirOptionalInitializationPlan,
    pub injection: MirOptionalInjectionPlan,
    pub copy: Option<MirOptionalCopyPlan>,
    pub assignment: Option<MirOptionalAssignmentPlan>,
    pub cleanup: MirOptionalCleanupPlan,
    pub presence: MirOptionalPresencePlan,
    pub unwrap: MirOptionalUnwrapPlan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOptionalInitializationPlan {
    TaggedAbsentOrPresent,
    NullableSharedOwner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOptionalInjectionPlan {
    StoreScalar,
    ConstructClass(ClassId),
    ConstructArray(ArrayTypeId),
    RetainShared(MirSharedTarget),
    ConstructNested(OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOptionalCopyPlan {
    Trivial,
    Class {
        class: ClassId,
        operation: MirSelectedCopyOperation<CopyConstructorId>,
    },
    Array(ArrayTypeId),
    Shared(MirSharedTarget),
    Optional(OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOptionalAssignmentPlan {
    Trivial,
    Class {
        class: ClassId,
        copy_constructor: MirSelectedCopyOperation<CopyConstructorId>,
        copy_assignment: MirSelectedCopyOperation<CopyAssignmentId>,
    },
    Array(ArrayTypeId),
    Shared(MirSharedTarget),
    Optional(OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOptionalCleanupPlan {
    Trivial,
    Class(ClassId),
    Array(ArrayTypeId),
    Shared(MirSharedTarget),
    Optional(OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOptionalPresencePlan {
    OuterTag,
    SharedOwnerNull,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOptionalUnwrapPlan {
    ExtractScalar,
    CheckedInlineClass(ClassId),
    CheckedInlineArray(ArrayTypeId),
    SecureSharedOwner(MirSharedTarget),
    CheckedNested(OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOptionalCheckedAccess {
    Value,
    GuardedInline,
    SecuredSharedOwner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirOptionalBoundaryPlans {
    pub argument: MirOptionalBoundaryPlan,
    pub result: MirOptionalBoundaryPlan,
    pub static_storage: MirOptionalBoundaryPlan,
    pub array_element: MirOptionalBoundaryPlan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOptionalBoundaryPlan {
    Copy(MirOptionalCopyPlan),
    MoveOnly,
}
