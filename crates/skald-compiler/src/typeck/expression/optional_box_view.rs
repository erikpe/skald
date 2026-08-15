//! Guarded object views selected from immutable shared optional boxes.

use crate::{
    hir::{
        HirAccess, HirObjectOrigin, HirObjectView, HirOptionalBoxObjectView, HirSharedPlace,
        HirSharedSource, HirSharedTarget, HirViewSource, HirViewTarget,
    },
    resolve::{
        ResolvedDereferenceExpr, ResolvedExpression, ResolvedObjectTarget, ResolvedUnwrapExpr,
    },
};

use super::super::function::CallableChecker;

impl CallableChecker<'_, '_> {
    pub(in crate::typeck) fn check_optional_box_presence_source(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<(HirSharedSource, crate::identity::OptionalBoxTypeId)> {
        let dereference = optional_box_dereference(expression)?;
        let crate::resolve::ResolvedSharedTarget::OptionalBox(box_target) = dereference.target
        else {
            return None;
        };
        self.program
            .optional_box_types
            .get(box_target)
            .and_then(|metadata| metadata.object_leaf)?;
        let source = self.check_shared_source(&dereference.source, false)?;
        (source.target() == HirSharedTarget::OptionalBox(box_target))
            .then_some((source, box_target))
    }

    pub(super) fn check_optional_box_object_view(
        &mut self,
        unwrap: &ResolvedUnwrapExpr,
    ) -> Option<HirOptionalBoxObjectView> {
        let (dereference, unwrap_depth) = optional_box_root(&unwrap.source, 1)?;
        let crate::resolve::ResolvedSharedTarget::OptionalBox(box_target) = dereference.target
        else {
            return None;
        };
        let metadata = self
            .program
            .optional_box_types
            .get(box_target)
            .expect("resolved optional-box view must name metadata");
        if metadata.optional_depth != unwrap_depth {
            return None;
        }
        let target = lower_object_target(metadata.object_leaf?);
        let source = self.check_shared_source(&dereference.source, false)?;
        if source.target() != HirSharedTarget::OptionalBox(box_target) {
            return None;
        }
        let access = match &source {
            HirSharedSource::Place(HirSharedPlace::Binding { binding, .. }) => {
                self.binding_access(*binding, false, unwrap.span)?
            }
            // Every non-binding source is first secured in an owned anchor.
            // Access to the containing handle is shallow: after the explicit
            // owning edge, the published optional wrapper stays immutable but
            // its present object keeps ordinary mutable pointee access.
            HirSharedSource::Place(HirSharedPlace::Field { .. })
            | HirSharedSource::Place(HirSharedPlace::ArrayElement { .. })
            | HirSharedSource::Place(HirSharedPlace::Static { .. })
            | HirSharedSource::Produced(_) => HirAccess::Mutable,
        };
        Some(HirOptionalBoxObjectView {
            source,
            box_target,
            target,
            access,
            span: unwrap.span,
        })
    }
}

fn optional_box_dereference(expression: &ResolvedExpression) -> Option<&ResolvedDereferenceExpr> {
    match expression {
        ResolvedExpression::Dereference(dereference) => Some(dereference),
        ResolvedExpression::Grouped(grouped) => optional_box_dereference(&grouped.expression),
        _ => None,
    }
}

pub(super) fn into_object_view(
    view: HirOptionalBoxObjectView,
    target: HirViewTarget,
    access: HirAccess,
    projections: Vec<crate::object_path::ObjectProjection>,
) -> HirObjectView {
    let span = view.span;
    let origin = match &view.source {
        HirSharedSource::Place(HirSharedPlace::Binding { binding, .. }) => {
            HirObjectOrigin::Shared {
                binding: *binding,
                static_target: view.target,
                access: view.access,
                span,
            }
        }
        HirSharedSource::Place(_) | HirSharedSource::Produced(_) => {
            HirObjectOrigin::AnchoredShared {
                static_target: view.target,
                access: view.access,
                span,
            }
        }
    };
    HirObjectView {
        source: HirViewSource::OptionalBoxPayload {
            view: Box::new(view),
            projections,
        },
        origin: Box::new(origin),
        target,
        access,
        span,
    }
}

fn optional_box_root(
    expression: &ResolvedExpression,
    depth: usize,
) -> Option<(&ResolvedDereferenceExpr, usize)> {
    match expression {
        ResolvedExpression::Dereference(dereference) => Some((dereference, depth)),
        ResolvedExpression::Unwrap(unwrap) => {
            optional_box_root(&unwrap.source, depth.checked_add(1)?)
        }
        ResolvedExpression::Grouped(grouped) => optional_box_root(&grouped.expression, depth),
        _ => None,
    }
}

const fn lower_object_target(target: ResolvedObjectTarget) -> HirViewTarget {
    match target {
        ResolvedObjectTarget::Class(class) => HirViewTarget::Class(class),
        ResolvedObjectTarget::Interface(interface) => HirViewTarget::Interface(interface),
        ResolvedObjectTarget::Obj => HirViewTarget::Obj,
    }
}
