//! Shared lifecycle-root analysis semantics used by planning and verification.

use crate::mir::{
    MirArrayLifecycleOperation, MirClassLifecycleOperation, MirExecutionNode, MirProgram,
    MirSharedTarget, MirType, PreliminaryMirSharedLifecycleTarget, StaticAccessKind,
    StaticEffectPhase,
};

use super::model::StaticAccessEvidence;

pub(crate) fn is_lifecycle_destination_or_published_self(
    root: crate::identity::StaticFieldId,
    effect: &StaticAccessEvidence,
) -> bool {
    is_lifecycle_destination_or_published_self_parts(
        root,
        effect.field,
        effect.access,
        effect.phase,
        effect.lifecycle_owned,
    )
}

pub(crate) fn is_lifecycle_destination_or_published_self_parts(
    root: crate::identity::StaticFieldId,
    field: crate::identity::StaticFieldId,
    access: StaticAccessKind,
    phase: StaticEffectPhase,
    lifecycle_owned: bool,
) -> bool {
    field == root
        && ((!lifecycle_owned && phase == StaticEffectPhase::InitializerAfterPublication)
            || (lifecycle_owned
                && phase == StaticEffectPhase::InitializerBeforePublication
                && access == StaticAccessKind::Initialize))
}

pub(crate) fn destruction_roots(program: &MirProgram, ty: MirType) -> Vec<MirExecutionNode> {
    match ty {
        MirType::Class(class) => vec![MirExecutionNode::class(
            class,
            MirClassLifecycleOperation::CompleteFinalizer,
        )],
        MirType::Shared(target) => shared_destruction_roots(program, target),
        MirType::Array(array) => vec![MirExecutionNode::array(
            array,
            MirArrayLifecycleOperation::Destruction,
        )],
        MirType::I64
        | MirType::U64
        | MirType::U8
        | MirType::F64
        | MirType::Bool
        | MirType::Function(_) => Vec::new(),
        MirType::Optional(optional) => match program.optional_type(optional) {
            Some(metadata) => match metadata.storage {
                crate::mir::MirOptionalStorage::InlineClass(class) => {
                    vec![MirExecutionNode::class(
                        class,
                        MirClassLifecycleOperation::CompleteFinalizer,
                    )]
                }
                crate::mir::MirOptionalStorage::SharedOwner(target) => {
                    shared_destruction_roots(program, target)
                }
                crate::mir::MirOptionalStorage::InlineArray(array) => {
                    vec![MirExecutionNode::array(
                        array,
                        MirArrayLifecycleOperation::Destruction,
                    )]
                }
                crate::mir::MirOptionalStorage::Scalar
                | crate::mir::MirOptionalStorage::Nested(_) => Vec::new(),
            },
            None => Vec::new(),
        },
        MirType::Interface(_) | MirType::Obj | MirType::Unit => Vec::new(),
    }
}

fn shared_destruction_roots(
    program: &MirProgram,
    target: MirSharedTarget,
) -> Vec<MirExecutionNode> {
    program
        .shared_lifecycle_targets(target)
        .into_iter()
        .filter_map(|target| match target {
            PreliminaryMirSharedLifecycleTarget::Class(class) => Some(MirExecutionNode::class(
                class,
                MirClassLifecycleOperation::CompleteFinalizer,
            )),
            PreliminaryMirSharedLifecycleTarget::Array(array) => Some(MirExecutionNode::array(
                array,
                MirArrayLifecycleOperation::Destruction,
            )),
            PreliminaryMirSharedLifecycleTarget::OptionalBox(_) => None,
        })
        .collect()
}
