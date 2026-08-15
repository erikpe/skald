//! Deterministic lowering of canonical HIR optional identities into MIR metadata.

use crate::{
    hir::{
        HirOptionalAssignmentPlan, HirOptionalBoundaryPlan, HirOptionalCheckedAccess,
        HirOptionalCopyPlan, HirOptionalDestructionPlan, HirOptionalInitializationPlan,
        HirOptionalInjectionPlan, HirOptionalPresenceTestPlan, HirOptionalRepresentation,
        HirOptionalStorageCategory, HirOptionalTypeTable, HirOptionalUnwrapPlan, Type,
    },
    mir::{
        MirOptionalAssignmentPlan, MirOptionalBoundaryPlan, MirOptionalBoundaryPlans,
        MirOptionalCheckedAccess, MirOptionalCleanupPlan, MirOptionalCopyPlan,
        MirOptionalInitializationPlan, MirOptionalInjectionPlan, MirOptionalLifecycle,
        MirOptionalPresencePlan, MirOptionalRepresentation, MirOptionalStorage, MirOptionalType,
        MirOptionalTypeTable, MirOptionalUnwrapPlan, MirType,
    },
};

use super::{lower_selected_copy_operation, lower_shared_target};

pub(super) fn scalar_id(
    types: &HirOptionalTypeTable,
    payload: crate::hir::HirPrimitiveType,
) -> crate::identity::OptionalTypeId {
    let payload = match payload {
        crate::hir::HirPrimitiveType::I64 => Type::I64,
        crate::hir::HirPrimitiveType::U64 => Type::U64,
        crate::hir::HirPrimitiveType::U8 => Type::U8,
        crate::hir::HirPrimitiveType::F64 => Type::F64,
        crate::hir::HirPrimitiveType::Bool => Type::Bool,
    };
    id_for_payload(types, payload)
}

pub(super) fn class_id(
    types: &HirOptionalTypeTable,
    class: crate::identity::ClassId,
) -> crate::identity::OptionalTypeId {
    id_for_payload(types, Type::Class(class))
}

pub(super) fn shared_id(
    types: &HirOptionalTypeTable,
    target: crate::hir::HirSharedTarget,
) -> crate::identity::OptionalTypeId {
    id_for_payload(types, Type::Shared(target))
}

fn id_for_payload(types: &HirOptionalTypeTable, payload: Type) -> crate::identity::OptionalTypeId {
    types
        .iter()
        .find(|optional| optional.payload == payload)
        .map(|optional| optional.id)
        .expect("typed optional operation must name interned payload metadata")
}

pub(super) fn primitive_payload(
    types: &HirOptionalTypeTable,
    operand: &crate::hir::HirOptionalOperand,
) -> crate::hir::HirPrimitiveType {
    match operand {
        crate::hir::HirOptionalOperand::Place(place) => place.payload,
        crate::hir::HirOptionalOperand::Produced(expression) => {
            let Type::Optional(optional) = expression.ty else {
                unreachable!("produced optional operand must have optional type")
            };
            match types
                .get(optional)
                .expect("typed optional identity must have metadata")
                .payload
            {
                Type::I64 => crate::hir::HirPrimitiveType::I64,
                Type::U64 => crate::hir::HirPrimitiveType::U64,
                Type::U8 => crate::hir::HirPrimitiveType::U8,
                Type::F64 => crate::hir::HirPrimitiveType::F64,
                Type::Bool => crate::hir::HirPrimitiveType::Bool,
                _ => unreachable!("primitive optional operand must have primitive metadata"),
            }
        }
        _ => unreachable!("expected primitive optional operand"),
    }
}

pub(super) fn class_payload(
    types: &HirOptionalTypeTable,
    operand: &crate::hir::HirOptionalOperand,
) -> crate::identity::ClassId {
    match operand {
        crate::hir::HirOptionalOperand::ClassPlace(place) => place.class,
        crate::hir::HirOptionalOperand::ClassProduced(expression) => {
            let Type::Optional(optional) = expression.ty else {
                unreachable!("produced optional operand must have optional type")
            };
            let HirOptionalStorageCategory::InlineClass(class) = types
                .get(optional)
                .expect("typed optional identity must have metadata")
                .storage
            else {
                unreachable!("class optional operand must have class metadata")
            };
            class
        }
        _ => unreachable!("expected class optional operand"),
    }
}

pub(super) fn shared_payload(
    types: &HirOptionalTypeTable,
    operand: &crate::hir::HirOptionalOperand,
) -> crate::hir::HirSharedTarget {
    match operand {
        crate::hir::HirOptionalOperand::SharedPlace(place) => place.target,
        crate::hir::HirOptionalOperand::SharedProduced(expression) => {
            let Type::Optional(optional) = expression.ty else {
                unreachable!("produced optional operand must have optional type")
            };
            let HirOptionalStorageCategory::SharedOwner(target) = types
                .get(optional)
                .expect("typed optional identity must have metadata")
                .storage
            else {
                unreachable!("shared optional operand must have shared-owner metadata")
            };
            target
        }
        _ => unreachable!("expected shared optional operand"),
    }
}

pub(super) fn lower_type(ty: Type) -> MirType {
    match ty {
        Type::I64 => MirType::I64,
        Type::U64 => MirType::U64,
        Type::U8 => MirType::U8,
        Type::F64 => MirType::F64,
        Type::Bool => MirType::Bool,
        Type::Array(array) => MirType::Array(array),
        Type::Class(class) => MirType::Class(class),
        Type::Interface(interface) => MirType::Interface(interface),
        Type::Function(function) => MirType::Function(function),
        Type::Obj => MirType::Obj,
        Type::Shared(target) => MirType::Shared(lower_shared_target(target)),
        Type::Optional(optional) => MirType::Optional(optional),
        Type::Unit => MirType::Unit,
    }
}

pub(super) fn lower_optional_types(types: &HirOptionalTypeTable) -> MirOptionalTypeTable {
    MirOptionalTypeTable::new(
        types
            .iter()
            .map(|optional| MirOptionalType {
                id: optional.id,
                payload: lower_type(optional.payload),
                storage: match optional.storage {
                    HirOptionalStorageCategory::Scalar => MirOptionalStorage::Scalar,
                    HirOptionalStorageCategory::InlineClass(class) => {
                        MirOptionalStorage::InlineClass(class)
                    }
                    HirOptionalStorageCategory::InlineArray(array) => {
                        MirOptionalStorage::InlineArray(array)
                    }
                    HirOptionalStorageCategory::SharedOwner(target) => {
                        MirOptionalStorage::SharedOwner(lower_shared_target(target))
                    }
                    HirOptionalStorageCategory::Nested(nested) => {
                        MirOptionalStorage::Nested(nested)
                    }
                },
                representation: match optional.representation {
                    HirOptionalRepresentation::TaggedPayload => {
                        MirOptionalRepresentation::TaggedPayload
                    }
                    HirOptionalRepresentation::NullableSharedOwner => {
                        MirOptionalRepresentation::NullableSharedOwner
                    }
                },
                lifecycle: MirOptionalLifecycle {
                    initialization: match optional.lifecycle.initialization {
                        HirOptionalInitializationPlan::TaggedAbsentOrPresent => {
                            MirOptionalInitializationPlan::TaggedAbsentOrPresent
                        }
                        HirOptionalInitializationPlan::NullableSharedOwner => {
                            MirOptionalInitializationPlan::NullableSharedOwner
                        }
                    },
                    injection: lower_injection(optional.lifecycle.injection),
                    copy: optional.lifecycle.copy.map(lower_copy),
                    assignment: optional.lifecycle.assignment.map(lower_assignment),
                    cleanup: match optional.lifecycle.destruction {
                        HirOptionalDestructionPlan::Trivial => MirOptionalCleanupPlan::Trivial,
                        HirOptionalDestructionPlan::Class(class) => {
                            MirOptionalCleanupPlan::Class(class)
                        }
                        HirOptionalDestructionPlan::Array(array) => {
                            MirOptionalCleanupPlan::Array(array)
                        }
                        HirOptionalDestructionPlan::Shared(target) => {
                            MirOptionalCleanupPlan::Shared(lower_shared_target(target))
                        }
                        HirOptionalDestructionPlan::Optional(nested) => {
                            MirOptionalCleanupPlan::Optional(nested)
                        }
                    },
                    presence: match optional.lifecycle.presence_test {
                        HirOptionalPresenceTestPlan::OuterTag => MirOptionalPresencePlan::OuterTag,
                        HirOptionalPresenceTestPlan::SharedOwnerNull => {
                            MirOptionalPresencePlan::SharedOwnerNull
                        }
                    },
                    unwrap: lower_unwrap(optional.lifecycle.unwrap),
                },
                checked_access: match optional.checked_access {
                    HirOptionalCheckedAccess::Value => MirOptionalCheckedAccess::Value,
                    HirOptionalCheckedAccess::GuardedInline => {
                        MirOptionalCheckedAccess::GuardedInline
                    }
                    HirOptionalCheckedAccess::SecuredSharedOwner => {
                        MirOptionalCheckedAccess::SecuredSharedOwner
                    }
                },
                boundaries: MirOptionalBoundaryPlans {
                    argument: lower_boundary(optional.boundaries.argument),
                    result: lower_boundary(optional.boundaries.result),
                    static_storage: lower_boundary(optional.boundaries.static_storage),
                    array_element: lower_boundary(optional.boundaries.array_element),
                },
            })
            .collect(),
    )
}

fn lower_injection(plan: HirOptionalInjectionPlan) -> MirOptionalInjectionPlan {
    match plan {
        HirOptionalInjectionPlan::StoreScalar => MirOptionalInjectionPlan::StoreScalar,
        HirOptionalInjectionPlan::ConstructClass(class) => {
            MirOptionalInjectionPlan::ConstructClass(class)
        }
        HirOptionalInjectionPlan::ConstructArray(array) => {
            MirOptionalInjectionPlan::ConstructArray(array)
        }
        HirOptionalInjectionPlan::RetainShared(target) => {
            MirOptionalInjectionPlan::RetainShared(lower_shared_target(target))
        }
        HirOptionalInjectionPlan::ConstructNested(nested) => {
            MirOptionalInjectionPlan::ConstructNested(nested)
        }
    }
}

fn lower_copy(plan: HirOptionalCopyPlan) -> MirOptionalCopyPlan {
    match plan {
        HirOptionalCopyPlan::Trivial => MirOptionalCopyPlan::Trivial,
        HirOptionalCopyPlan::Class { class, operation } => MirOptionalCopyPlan::Class {
            class,
            operation: lower_selected_copy_operation(operation),
        },
        HirOptionalCopyPlan::Array(array) => MirOptionalCopyPlan::Array(array),
        HirOptionalCopyPlan::Shared(target) => {
            MirOptionalCopyPlan::Shared(lower_shared_target(target))
        }
        HirOptionalCopyPlan::Optional(nested) => MirOptionalCopyPlan::Optional(nested),
    }
}

fn lower_assignment(plan: HirOptionalAssignmentPlan) -> MirOptionalAssignmentPlan {
    match plan {
        HirOptionalAssignmentPlan::Trivial => MirOptionalAssignmentPlan::Trivial,
        HirOptionalAssignmentPlan::Class {
            class,
            copy_constructor,
            copy_assignment,
        } => MirOptionalAssignmentPlan::Class {
            class,
            copy_constructor: lower_selected_copy_operation(copy_constructor),
            copy_assignment: lower_selected_copy_operation(copy_assignment),
        },
        HirOptionalAssignmentPlan::Array(array) => MirOptionalAssignmentPlan::Array(array),
        HirOptionalAssignmentPlan::Shared(target) => {
            MirOptionalAssignmentPlan::Shared(lower_shared_target(target))
        }
        HirOptionalAssignmentPlan::Optional(nested) => MirOptionalAssignmentPlan::Optional(nested),
    }
}

fn lower_unwrap(plan: HirOptionalUnwrapPlan) -> MirOptionalUnwrapPlan {
    match plan {
        HirOptionalUnwrapPlan::ExtractScalar => MirOptionalUnwrapPlan::ExtractScalar,
        HirOptionalUnwrapPlan::CheckedInlineClass(class) => {
            MirOptionalUnwrapPlan::CheckedInlineClass(class)
        }
        HirOptionalUnwrapPlan::CheckedInlineArray(array) => {
            MirOptionalUnwrapPlan::CheckedInlineArray(array)
        }
        HirOptionalUnwrapPlan::SecureSharedOwner(target) => {
            MirOptionalUnwrapPlan::SecureSharedOwner(lower_shared_target(target))
        }
        HirOptionalUnwrapPlan::CheckedNested(nested) => {
            MirOptionalUnwrapPlan::CheckedNested(nested)
        }
    }
}

fn lower_boundary(plan: HirOptionalBoundaryPlan) -> MirOptionalBoundaryPlan {
    match plan {
        HirOptionalBoundaryPlan::Copy(copy) => MirOptionalBoundaryPlan::Copy(lower_copy(copy)),
        HirOptionalBoundaryPlan::MoveOnly => MirOptionalBoundaryPlan::MoveOnly,
    }
}
