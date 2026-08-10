//! Canonical typed optional identities and their selected semantic plans.

use crate::{
    id_table::DenseIdTable,
    identity::{ArrayTypeId, ClassId, CopyAssignmentId, CopyConstructorId, OptionalTypeId},
};

use super::{HirSelectedCopyOperation, HirSharedTarget, Type};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirOptionalTypeTable {
    entries: DenseIdTable<OptionalTypeId, HirOptionalType>,
}

impl HirOptionalTypeTable {
    pub(crate) fn new(entries: Vec<HirOptionalType>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: OptionalTypeId) -> Option<&HirOptionalType> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirOptionalType> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirOptionalType {
    pub id: OptionalTypeId,
    pub payload: Type,
    pub storage: HirOptionalStorageCategory,
    pub representation: HirOptionalRepresentation,
    pub lifecycle: HirOptionalLifecycle,
    pub checked_access: HirOptionalCheckedAccess,
    pub boundaries: HirOptionalBoundaryPlans,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOptionalStorageCategory {
    Scalar,
    InlineClass(ClassId),
    InlineArray(ArrayTypeId),
    SharedOwner(HirSharedTarget),
    Nested(OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOptionalRepresentation {
    TaggedPayload,
    NullableSharedOwner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirOptionalLifecycle {
    pub initialization: HirOptionalInitializationPlan,
    pub injection: HirOptionalInjectionPlan,
    pub copy: Option<HirOptionalCopyPlan>,
    pub assignment: Option<HirOptionalAssignmentPlan>,
    pub destruction: HirOptionalDestructionPlan,
    pub presence_test: HirOptionalPresenceTestPlan,
    pub unwrap: HirOptionalUnwrapPlan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOptionalInitializationPlan {
    TaggedAbsentOrPresent,
    NullableSharedOwner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOptionalInjectionPlan {
    StoreScalar,
    ConstructClass(ClassId),
    ConstructArray(ArrayTypeId),
    RetainShared(HirSharedTarget),
    ConstructNested(OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOptionalCopyPlan {
    Trivial,
    Class {
        class: ClassId,
        operation: HirSelectedCopyOperation<CopyConstructorId>,
    },
    Array(ArrayTypeId),
    Shared(HirSharedTarget),
    Optional(OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOptionalAssignmentPlan {
    Trivial,
    Class {
        class: ClassId,
        copy_constructor: HirSelectedCopyOperation<CopyConstructorId>,
        copy_assignment: HirSelectedCopyOperation<CopyAssignmentId>,
    },
    Array(ArrayTypeId),
    Shared(HirSharedTarget),
    Optional(OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOptionalDestructionPlan {
    Trivial,
    Class(ClassId),
    Array(ArrayTypeId),
    Shared(HirSharedTarget),
    Optional(OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOptionalPresenceTestPlan {
    OuterTag,
    SharedOwnerNull,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOptionalUnwrapPlan {
    ExtractScalar,
    CheckedInlineClass(ClassId),
    CheckedInlineArray(ArrayTypeId),
    SecureSharedOwner(HirSharedTarget),
    CheckedNested(OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOptionalCheckedAccess {
    Value,
    GuardedInline,
    SecuredSharedOwner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirOptionalBoundaryPlans {
    pub argument: HirOptionalBoundaryPlan,
    pub result: HirOptionalBoundaryPlan,
    pub static_storage: HirOptionalBoundaryPlan,
    pub array_element: HirOptionalBoundaryPlan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOptionalBoundaryPlan {
    Copy(HirOptionalCopyPlan),
    MoveOnly,
}
