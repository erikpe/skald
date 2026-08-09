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
        MirType::Class(class) | MirType::OptionalClass(class) => vec![StaticEffectNode::class(
            class,
            StaticClassLifecycleOperation::CompleteFinalizer,
        )],
        MirType::Shared(target) | MirType::OptionalShared(target) => {
            shared_destruction_roots(program, target)
        }
        MirType::Array(array) => vec![StaticEffectNode::array(
            array,
            StaticArrayLifecycleOperation::Destruction,
        )],
        MirType::I64
        | MirType::U64
        | MirType::U8
        | MirType::F64
        | MirType::Bool
        | MirType::OptionalPrimitive(_) => Vec::new(),
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
        .map(|target| match target {
            PreliminaryMirSharedLifecycleTarget::Class(class) => {
                StaticEffectNode::class(class, StaticClassLifecycleOperation::CompleteFinalizer)
            }
            PreliminaryMirSharedLifecycleTarget::Array(array) => {
                StaticEffectNode::array(array, StaticArrayLifecycleOperation::Destruction)
            }
        })
        .collect()
}
