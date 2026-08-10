//! Selection and queries for canonical typed optional metadata.

use crate::{
    hir::{
        HirOptionalAssignmentPlan, HirOptionalBoundaryPlan, HirOptionalBoundaryPlans,
        HirOptionalCheckedAccess, HirOptionalCopyPlan, HirOptionalDestructionPlan,
        HirOptionalInitializationPlan, HirOptionalInjectionPlan, HirOptionalLifecycle,
        HirOptionalPresenceTestPlan, HirOptionalRepresentation, HirOptionalStorageCategory,
        HirOptionalType, HirOptionalTypeTable, HirOptionalUnwrapPlan, HirPrimitiveType,
        HirSharedTarget, Type,
    },
    identity::OptionalTypeId,
    resolve::{ResolvedProgram, ResolvedSharedTarget, ResolvedTypeKind},
};

use super::{capabilities::CopyCapabilities, program::lower_type};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OptionalPayloadKind {
    Primitive(HirPrimitiveType),
    Class(crate::identity::ClassId),
    Shared(HirSharedTarget),
    Nested(OptionalTypeId),
    Array(crate::identity::ArrayTypeId),
}

pub(super) fn classify_payload(
    program: &ResolvedProgram,
    optional: OptionalTypeId,
) -> Option<OptionalPayloadKind> {
    match resolved_payload_kind(program, optional) {
        ResolvedTypeKind::I64 => Some(OptionalPayloadKind::Primitive(HirPrimitiveType::I64)),
        ResolvedTypeKind::U64 => Some(OptionalPayloadKind::Primitive(HirPrimitiveType::U64)),
        ResolvedTypeKind::U8 => Some(OptionalPayloadKind::Primitive(HirPrimitiveType::U8)),
        ResolvedTypeKind::F64 => Some(OptionalPayloadKind::Primitive(HirPrimitiveType::F64)),
        ResolvedTypeKind::Bool => Some(OptionalPayloadKind::Primitive(HirPrimitiveType::Bool)),
        ResolvedTypeKind::Class(class) => Some(OptionalPayloadKind::Class(class)),
        ResolvedTypeKind::Shared(target) => Some(OptionalPayloadKind::Shared(lower_shared(target))),
        ResolvedTypeKind::Optional(nested) => Some(OptionalPayloadKind::Nested(nested)),
        ResolvedTypeKind::Array(array) => Some(OptionalPayloadKind::Array(array)),
        _ => None,
    }
}

pub(super) fn payload_type(program: &ResolvedProgram, optional: OptionalTypeId) -> Type {
    lower_type(
        program,
        &program
            .optional_types
            .get(optional)
            .expect("optional identity must name resolved metadata")
            .payload,
    )
}

pub(super) fn optional_id(ty: Type) -> Option<OptionalTypeId> {
    let Type::Optional(optional) = ty else {
        return None;
    };
    Some(optional)
}

pub(super) fn lower_optional_types(
    program: &ResolvedProgram,
    capabilities: &CopyCapabilities,
) -> HirOptionalTypeTable {
    let mut entries = Vec::with_capacity(program.optional_types.len());
    for optional in program.optional_types.iter() {
        entries.push(lower_optional_type(
            program,
            capabilities,
            &entries,
            optional.id,
        ));
    }
    HirOptionalTypeTable::new(entries)
}

fn lower_optional_type(
    program: &ResolvedProgram,
    capabilities: &CopyCapabilities,
    previous: &[HirOptionalType],
    id: OptionalTypeId,
) -> HirOptionalType {
    let payload = resolved_payload_kind(program, id);
    let (storage, representation, injection, copy, assignment, destruction, unwrap, access) =
        match payload {
            ResolvedTypeKind::I64
            | ResolvedTypeKind::U64
            | ResolvedTypeKind::U8
            | ResolvedTypeKind::F64
            | ResolvedTypeKind::Bool => (
                HirOptionalStorageCategory::Scalar,
                HirOptionalRepresentation::TaggedPayload,
                HirOptionalInjectionPlan::StoreScalar,
                Some(HirOptionalCopyPlan::Trivial),
                Some(HirOptionalAssignmentPlan::Trivial),
                HirOptionalDestructionPlan::Trivial,
                HirOptionalUnwrapPlan::ExtractScalar,
                HirOptionalCheckedAccess::Value,
            ),
            ResolvedTypeKind::Class(class) => (
                HirOptionalStorageCategory::InlineClass(class),
                HirOptionalRepresentation::TaggedPayload,
                HirOptionalInjectionPlan::ConstructClass(class),
                capabilities
                    .constructor(class)
                    .selected()
                    .map(|operation| HirOptionalCopyPlan::Class { class, operation }),
                capabilities
                    .constructor(class)
                    .selected()
                    .zip(capabilities.assignment(class).selected())
                    .map(
                        |(copy_constructor, copy_assignment)| HirOptionalAssignmentPlan::Class {
                            class,
                            copy_constructor,
                            copy_assignment,
                        },
                    ),
                HirOptionalDestructionPlan::Class(class),
                HirOptionalUnwrapPlan::CheckedInlineClass(class),
                HirOptionalCheckedAccess::GuardedInline,
            ),
            ResolvedTypeKind::Array(array) => {
                let lifecycle = &capabilities.array(array).lifecycle;
                (
                    HirOptionalStorageCategory::InlineArray(array),
                    HirOptionalRepresentation::TaggedPayload,
                    HirOptionalInjectionPlan::ConstructArray(array),
                    lifecycle.copy.map(|_| HirOptionalCopyPlan::Array(array)),
                    lifecycle
                        .assignment
                        .map(|_| HirOptionalAssignmentPlan::Array(array)),
                    HirOptionalDestructionPlan::Array(array),
                    HirOptionalUnwrapPlan::CheckedInlineArray(array),
                    HirOptionalCheckedAccess::GuardedInline,
                )
            }
            ResolvedTypeKind::Shared(target) => {
                let target = lower_shared(target);
                (
                    HirOptionalStorageCategory::SharedOwner(target),
                    HirOptionalRepresentation::NullableSharedOwner,
                    HirOptionalInjectionPlan::RetainShared(target),
                    Some(HirOptionalCopyPlan::Shared(target)),
                    Some(HirOptionalAssignmentPlan::Shared(target)),
                    HirOptionalDestructionPlan::Shared(target),
                    HirOptionalUnwrapPlan::SecureSharedOwner(target),
                    HirOptionalCheckedAccess::SecuredSharedOwner,
                )
            }
            ResolvedTypeKind::Optional(nested) => {
                let nested_type = previous
                    .get(nested.index())
                    .expect("nested optional identities must precede containing identities");
                (
                    HirOptionalStorageCategory::Nested(nested),
                    HirOptionalRepresentation::TaggedPayload,
                    HirOptionalInjectionPlan::ConstructNested(nested),
                    nested_type
                        .lifecycle
                        .copy
                        .map(|_| HirOptionalCopyPlan::Optional(nested)),
                    nested_type
                        .lifecycle
                        .assignment
                        .map(|_| HirOptionalAssignmentPlan::Optional(nested)),
                    HirOptionalDestructionPlan::Optional(nested),
                    HirOptionalUnwrapPlan::CheckedNested(nested),
                    HirOptionalCheckedAccess::GuardedInline,
                )
            }
            ResolvedTypeKind::Unit | ResolvedTypeKind::Obj | ResolvedTypeKind::Interface(_) => {
                unreachable!("invalid optional payload must be rejected before HIR planning")
            }
        };
    let initialization = match representation {
        HirOptionalRepresentation::TaggedPayload => {
            HirOptionalInitializationPlan::TaggedAbsentOrPresent
        }
        HirOptionalRepresentation::NullableSharedOwner => {
            HirOptionalInitializationPlan::NullableSharedOwner
        }
    };
    let presence_test = match representation {
        HirOptionalRepresentation::TaggedPayload => HirOptionalPresenceTestPlan::OuterTag,
        HirOptionalRepresentation::NullableSharedOwner => {
            HirOptionalPresenceTestPlan::SharedOwnerNull
        }
    };
    let boundary = copy
        .map(HirOptionalBoundaryPlan::Copy)
        .unwrap_or(HirOptionalBoundaryPlan::MoveOnly);
    HirOptionalType {
        id,
        payload: payload_type(program, id),
        storage,
        representation,
        lifecycle: HirOptionalLifecycle {
            initialization,
            injection,
            copy,
            assignment,
            destruction,
            presence_test,
            unwrap,
        },
        checked_access: access,
        boundaries: HirOptionalBoundaryPlans {
            argument: boundary,
            result: boundary,
            static_storage: boundary,
            array_element: boundary,
        },
    }
}

fn resolved_payload_kind(program: &ResolvedProgram, optional: OptionalTypeId) -> ResolvedTypeKind {
    program
        .optional_types
        .get(optional)
        .expect("optional identity must name resolved metadata")
        .payload
        .kind
}

fn lower_shared(target: ResolvedSharedTarget) -> HirSharedTarget {
    super::shared::lower_shared_target(target)
}
