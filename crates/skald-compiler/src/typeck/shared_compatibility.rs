//! Compatibility facade for ordinary shared owners and optional-box views.

use crate::{
    hir::{HirSharedTarget, HirViewTarget},
    resolve::ResolvedProgram,
};

use super::expression::{
    class_provides_view, classify_object_view_relation, ObjectViewRelation, ObjectViewSource,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SharedTargetRelation {
    Identity,
    UpView,
    CheckedDowncast,
    Impossible,
}

impl SharedTargetRelation {
    pub(super) const fn is_implicit(self) -> bool {
        matches!(self, Self::Identity | Self::UpView)
    }
}

pub(super) fn relation(
    program: &ResolvedProgram,
    source: HirSharedTarget,
    target: HirSharedTarget,
) -> SharedTargetRelation {
    if source == target {
        return SharedTargetRelation::Identity;
    }
    match (source, target) {
        (HirSharedTarget::OptionalBox(source), HirSharedTarget::OptionalBox(target)) => {
            optional_box_relation(program, source, target)
        }
        (source, target) => ordinary_relation(program, source, target),
    }
}

fn optional_box_relation(
    program: &ResolvedProgram,
    source: crate::identity::OptionalBoxTypeId,
    target: crate::identity::OptionalBoxTypeId,
) -> SharedTargetRelation {
    let source = program
        .optional_box_types
        .get(source)
        .expect("resolved optional-box source must name metadata");
    let target = program
        .optional_box_types
        .get(target)
        .expect("resolved optional-box target must name metadata");
    if source.optional_depth != target.optional_depth {
        return SharedTargetRelation::Impossible;
    }
    let (Some(source_view), Some(target_view)) = (source.object_leaf, target.object_leaf) else {
        return SharedTargetRelation::Impossible;
    };
    object_relation(
        program,
        lower_object_view(source_view),
        lower_object_view(target_view),
    )
}

fn ordinary_relation(
    program: &ResolvedProgram,
    source: HirSharedTarget,
    target: HirSharedTarget,
) -> SharedTargetRelation {
    match (source, target) {
        (HirSharedTarget::Array(source), HirSharedTarget::Array(target)) => {
            if source == target {
                SharedTargetRelation::Identity
            } else {
                SharedTargetRelation::Impossible
            }
        }
        (source, target) => {
            let (Some(source), Some(target)) = (object_view(source), object_view(target)) else {
                return SharedTargetRelation::Impossible;
            };
            object_relation(program, source, target)
        }
    }
}

fn object_relation(
    program: &ResolvedProgram,
    source: HirViewTarget,
    target: HirViewTarget,
) -> SharedTargetRelation {
    if source == target {
        return SharedTargetRelation::Identity;
    }
    if object_target_accepts(program, target, source) {
        return SharedTargetRelation::UpView;
    }
    match classify_object_view_relation(program, ObjectViewSource::Dynamic(source), target) {
        ObjectViewRelation::StaticSuccess => SharedTargetRelation::UpView,
        ObjectViewRelation::Runtime => SharedTargetRelation::CheckedDowncast,
        ObjectViewRelation::StaticFailure => SharedTargetRelation::Impossible,
    }
}

fn object_target_accepts(
    program: &ResolvedProgram,
    expected: HirViewTarget,
    actual: HirViewTarget,
) -> bool {
    match expected {
        HirViewTarget::Obj => true,
        HirViewTarget::Class(expected) => match actual {
            HirViewTarget::Class(actual) => program
                .hierarchy
                .is_subtype(actual, expected)
                .unwrap_or(false),
            HirViewTarget::Obj | HirViewTarget::Interface(_) => false,
        },
        HirViewTarget::Interface(expected) => match actual {
            HirViewTarget::Class(actual) => {
                class_provides_view(program, actual, HirViewTarget::Interface(expected))
            }
            HirViewTarget::Interface(actual) => actual == expected,
            HirViewTarget::Obj => false,
        },
    }
}

const fn object_view(target: HirSharedTarget) -> Option<HirViewTarget> {
    match target {
        HirSharedTarget::Obj => Some(HirViewTarget::Obj),
        HirSharedTarget::Class(class) => Some(HirViewTarget::Class(class)),
        HirSharedTarget::Interface(interface) => Some(HirViewTarget::Interface(interface)),
        HirSharedTarget::Array(_) | HirSharedTarget::OptionalBox(_) => None,
    }
}

const fn lower_object_view(target: crate::resolve::ResolvedObjectTarget) -> HirViewTarget {
    match target {
        crate::resolve::ResolvedObjectTarget::Obj => HirViewTarget::Obj,
        crate::resolve::ResolvedObjectTarget::Class(class) => HirViewTarget::Class(class),
        crate::resolve::ResolvedObjectTarget::Interface(interface) => {
            HirViewTarget::Interface(interface)
        }
    }
}
