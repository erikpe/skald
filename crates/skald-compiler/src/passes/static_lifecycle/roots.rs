//! Shared lifecycle-root semantics used by planning and verification.

use crate::mir::{
    MirProgram, MirSharedTarget, MirType, PreliminaryMirSharedLifecycleTarget,
    StaticAccessEvidence, StaticAccessKind, StaticArrayLifecycleOperation,
    StaticClassLifecycleOperation, StaticEffectNode, StaticEffectPhase,
};

pub(crate) fn is_lifecycle_destination_or_published_self(
    root: crate::identity::StaticFieldId,
    effect: &StaticAccessEvidence,
) -> bool {
    effect.field == root
        && ((!effect.lifecycle_owned
            && effect.phase == StaticEffectPhase::InitializerAfterPublication)
            || (effect.lifecycle_owned
                && effect.phase == StaticEffectPhase::InitializerBeforePublication
                && effect.access == StaticAccessKind::Initialize))
}

pub(crate) fn destruction_roots(program: &MirProgram, ty: MirType) -> Vec<StaticEffectNode> {
    match ty {
        MirType::Class(class) => vec![StaticEffectNode::class(
            class,
            StaticClassLifecycleOperation::CompleteFinalizer,
        )],
        MirType::Shared(target) => shared_destruction_roots(program, target),
        MirType::Array(array) => vec![StaticEffectNode::array(
            array,
            StaticArrayLifecycleOperation::Destruction,
        )],
        MirType::I64 | MirType::U64 | MirType::U8 | MirType::F64 | MirType::Bool => Vec::new(),
        MirType::Optional(optional) => match program.optional_type(optional) {
            Some(metadata) => match metadata.storage {
                crate::mir::MirOptionalStorage::InlineClass(class) => {
                    vec![StaticEffectNode::class(
                        class,
                        StaticClassLifecycleOperation::CompleteFinalizer,
                    )]
                }
                crate::mir::MirOptionalStorage::SharedOwner(target) => {
                    shared_destruction_roots(program, target)
                }
                crate::mir::MirOptionalStorage::InlineArray(array) => {
                    vec![StaticEffectNode::array(
                        array,
                        StaticArrayLifecycleOperation::Destruction,
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
) -> Vec<StaticEffectNode> {
    program
        .shared_lifecycle_targets(target)
        .into_iter()
        .filter_map(|target| match target {
            PreliminaryMirSharedLifecycleTarget::Class(class) => Some(StaticEffectNode::class(
                class,
                StaticClassLifecycleOperation::CompleteFinalizer,
            )),
            PreliminaryMirSharedLifecycleTarget::Array(array) => Some(StaticEffectNode::array(
                array,
                StaticArrayLifecycleOperation::Destruction,
            )),
            PreliminaryMirSharedLifecycleTarget::OptionalBox(_) => None,
        })
        .collect()
}
