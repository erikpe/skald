//! Direct object-view planning after source checking.

use crate::{
    hir::{HirAccess, HirObjectPlace, HirObjectView, HirViewTarget},
    object_path::ObjectProjection,
    resolve::ResolvedProgram,
};

use super::relation::view_guarantees_target;
use super::{
    classify_object_view_relation, ObjectViewRelation, ObjectViewRelationSource, ObjectViewSource,
};

/// How long the direct consumer must keep a view's owner alive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::typeck) enum ObjectViewRetention {
    ImmediateConsumer,
    LoopBody,
}

/// Consumer-owned input to direct object-view planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::typeck) struct ObjectViewRequest {
    target: HirViewTarget,
    access: HirAccess,
    retention: ObjectViewRetention,
}

impl ObjectViewRequest {
    pub(in crate::typeck) const fn new(
        target: HirViewTarget,
        access: HirAccess,
        retention: ObjectViewRetention,
    ) -> Self {
        Self {
            target,
            access,
            retention,
        }
    }

    pub(in crate::typeck) const fn target(self) -> HirViewTarget {
        self.target
    }

    pub(in crate::typeck) const fn access(self) -> HirAccess {
        self.access
    }
}

/// A source and request that cannot form a direct object view.
pub(in crate::typeck) enum ObjectViewProblem {
    InsufficientAccess(Box<ObjectViewSource>, ObjectViewRequest),
    IncompatibleTarget(Box<ObjectViewSource>, ObjectViewRequest),
    RequiresExplicitCheckedOperation(Box<ObjectViewSource>, ObjectViewRequest),
}

/// A fully checked direct view, consumed when its existing HIR carrier is made.
pub(in crate::typeck) struct ObjectViewPlan {
    source: ObjectViewSource,
    request: ObjectViewRequest,
    target_projections: Vec<ObjectProjection>,
}

impl ObjectViewPlan {
    pub(in crate::typeck) fn into_view(self) -> HirObjectView {
        self.source.into_view_with_target_projections(
            self.request.target,
            self.request.access,
            self.target_projections,
        )
    }
}

pub(in crate::typeck) fn plan_object_view(
    program: &ResolvedProgram,
    source: ObjectViewSource,
    request: ObjectViewRequest,
) -> Result<ObjectViewPlan, ObjectViewProblem> {
    if !source.access().permits(request.access) {
        return Err(ObjectViewProblem::InsufficientAccess(
            Box::new(source),
            request,
        ));
    }

    let relation_source = direct_relation_source(&source);
    let relation = classify_object_view_relation(program, relation_source, request.target);
    let relation = if relation == ObjectViewRelation::StaticSuccess
        && uses_published_target(&source)
        && !view_guarantees_target(program, source.static_target(), request.target)
    {
        // Closed-world class enumeration can prove this conversion safe for
        // today's program without making it an allowed implicit conversion.
        ObjectViewRelation::Runtime
    } else {
        relation
    };
    match relation {
        ObjectViewRelation::StaticFailure => {
            return Err(ObjectViewProblem::IncompatibleTarget(
                Box::new(source),
                request,
            ));
        }
        ObjectViewRelation::Runtime => {
            return Err(ObjectViewProblem::RequiresExplicitCheckedOperation(
                Box::new(source),
                request,
            ));
        }
        ObjectViewRelation::StaticSuccess => {}
    }

    let actual = source.static_target();
    let target_projections = static_class_up_projections(program, actual, request.target);
    let source = prepare_source(program, source, request, &target_projections);
    let source = apply_object_view_retention(source, request.retention);
    Ok(ObjectViewPlan {
        source,
        request,
        target_projections,
    })
}

pub(in crate::typeck) fn plan_resolved_object_view(
    program: &ResolvedProgram,
    source: ObjectViewSource,
    request: ObjectViewRequest,
) -> HirObjectView {
    match plan_object_view(program, source, request) {
        Ok(plan) => plan.into_view(),
        Err(_) => panic!("resolved object receiver must produce a valid direct-view plan"),
    }
}

pub(in crate::typeck) fn apply_object_view_retention(
    source: ObjectViewSource,
    retention: ObjectViewRetention,
) -> ObjectViewSource {
    match (source, retention) {
        (ObjectViewSource::Shared(source), ObjectViewRetention::LoopBody) => {
            ObjectViewSource::Shared(source.into_iteration_source())
        }
        (source, ObjectViewRetention::ImmediateConsumer | ObjectViewRetention::LoopBody) => source,
    }
}

fn uses_published_target(source: &ObjectViewSource) -> bool {
    matches!(
        source,
        ObjectViewSource::Obj { .. }
            | ObjectViewSource::Interface { .. }
            | ObjectViewSource::Shared(_)
            | ObjectViewSource::OptionalBox { .. }
    )
}

fn direct_relation_source(source: &ObjectViewSource) -> ObjectViewRelationSource {
    match source {
        // An inline place can name a subobject whose class differs from the
        // complete-object origin. Direct compatibility follows the selected
        // place, just as its ancestor projections do.
        ObjectViewSource::Class { place, .. } => {
            ObjectViewRelationSource::ExactClass(place.class())
        }
        ObjectViewSource::Static { class, .. }
        | ObjectViewSource::Produced { class, .. }
        | ObjectViewSource::Optional { class, .. }
        | ObjectViewSource::ArrayElement { class, .. } => {
            ObjectViewRelationSource::ExactClass(*class)
        }
        // Direct shared-backed views are limited by their published static
        // target even when a freshly produced owner exposes its exact class.
        ObjectViewSource::Shared(_) | ObjectViewSource::OptionalBox { .. } => {
            ObjectViewRelationSource::Dynamic(source.static_target())
        }
        _ => source.relation_source(),
    }
}

fn prepare_source(
    program: &ResolvedProgram,
    source: ObjectViewSource,
    request: ObjectViewRequest,
    projections: &[ObjectProjection],
) -> ObjectViewSource {
    match source {
        ObjectViewSource::Class { place, origin } => {
            let place = match request.target {
                HirViewTarget::Class(target) => project_place_to_ancestor(program, place, target)
                    .expect("statically compatible class view must select an ancestor"),
                HirViewTarget::Interface(_) | HirViewTarget::Obj => place,
            };
            ObjectViewSource::Class { place, origin }
        }
        ObjectViewSource::Shared(mut source) => {
            source.select_target(request.target, projections.iter().copied());
            ObjectViewSource::Shared(source)
        }
        ObjectViewSource::Optional {
            view,
            dynamic_class,
            class,
            projections: mut source_projections,
        } => {
            source_projections.extend_from_slice(projections);
            ObjectViewSource::Optional {
                view,
                dynamic_class,
                class,
                projections: source_projections,
            }
        }
        source => source,
    }
}

pub(in crate::typeck::expression) fn project_place_to_ancestor(
    program: &ResolvedProgram,
    mut place: HirObjectPlace,
    target: crate::identity::ClassId,
) -> Option<HirObjectPlace> {
    if place.class() == target {
        return Some(place);
    }
    let span = place.span();
    for base in program.hierarchy.base_chain(place.class())? {
        place.path = place.path.project_base(base, span);
        if base == target {
            return Some(place);
        }
    }
    None
}

fn static_class_up_projections(
    program: &ResolvedProgram,
    actual: HirViewTarget,
    expected: HirViewTarget,
) -> Vec<ObjectProjection> {
    let (HirViewTarget::Class(actual), HirViewTarget::Class(expected)) = (actual, expected) else {
        return Vec::new();
    };
    if actual == expected {
        return Vec::new();
    }
    program
        .hierarchy
        .base_chain(actual)
        .expect("compatible class view must have valid ancestry")
        .take_while(|class| *class != expected)
        .chain(std::iter::once(expected))
        .map(ObjectProjection::Base)
        .collect()
}

#[cfg(test)]
mod tests;
