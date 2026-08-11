//! Checked projection from shared-owner values to non-owning object places.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirAccess, HirObjectOrigin, HirObjectView, HirSharedPlace, HirSharedSource,
        HirSharedTarget, HirViewSource, HirViewTarget,
    },
    identity::BindingId,
    object_path::ObjectProjection,
    resolve::{ResolvedDereferenceExpr, ResolvedExpression},
    source::Span,
};

use super::super::function::CallableChecker;

/// A checked shared owner together with the lifetime strategy required before
/// its pointee can be exposed as a non-owning object place.
///
/// Stable bindings borrow directly. Replaceable places and produced owners
/// retain their complete allocation through an explicit hidden anchor.
pub(super) struct CheckedSharedPointee {
    source: CheckedSharedPointeeSource,
    target: HirViewTarget,
    access: HirAccess,
    projections: Vec<ObjectProjection>,
    span: Span,
}

enum CheckedSharedPointeeSource {
    Stable(BindingId),
    Anchored(HirSharedSource),
}

impl CheckedSharedPointee {
    pub(super) fn stable(
        binding: BindingId,
        target: HirViewTarget,
        access: HirAccess,
        projections: Vec<ObjectProjection>,
        span: Span,
    ) -> Self {
        Self {
            source: CheckedSharedPointeeSource::Stable(binding),
            target,
            access,
            projections,
            span,
        }
    }

    pub(super) const fn access(&self) -> HirAccess {
        self.access
    }

    pub(super) const fn span(&self) -> Span {
        self.span
    }

    pub(super) const fn static_target(&self) -> HirViewTarget {
        self.target
    }

    pub(super) fn exact_dynamic_class(&self) -> Option<crate::identity::ClassId> {
        match &self.source {
            CheckedSharedPointeeSource::Stable(_) => None,
            CheckedSharedPointeeSource::Anchored(source) => source.exact_dynamic_class(),
        }
    }

    pub(super) const fn stable_binding(&self) -> Option<BindingId> {
        match &self.source {
            CheckedSharedPointeeSource::Stable(binding) => Some(*binding),
            CheckedSharedPointeeSource::Anchored(_) => None,
        }
    }

    pub(super) fn set_span(&mut self, span: Span) {
        self.span = span;
    }

    pub(super) fn set_projections(&mut self, projections: Vec<ObjectProjection>) {
        self.projections = projections;
    }

    pub(super) fn projections(&self) -> &[ObjectProjection] {
        &self.projections
    }

    pub(super) fn origin(&self) -> HirObjectOrigin {
        match &self.source {
            CheckedSharedPointeeSource::Stable(binding) => HirObjectOrigin::Shared {
                binding: *binding,
                static_target: self.target,
                access: self.access,
                span: self.span,
            },
            CheckedSharedPointeeSource::Anchored(_) => HirObjectOrigin::AnchoredShared {
                static_target: self.target,
                access: self.access,
                span: self.span,
            },
        }
    }

    pub(super) fn into_view(self, target: HirViewTarget, access: HirAccess) -> HirObjectView {
        let origin = Box::new(self.origin());
        match self.source {
            CheckedSharedPointeeSource::Stable(binding) => HirObjectView {
                source: HirViewSource::Shared {
                    binding,
                    target: self.target,
                    access: self.access,
                    projections: self.projections,
                    span: self.span,
                },
                origin,
                target,
                access,
                span: self.span,
            },
            CheckedSharedPointeeSource::Anchored(source) => HirObjectView {
                source: HirViewSource::AnchoredShared {
                    source: Box::new(source),
                    target: self.target,
                    access: self.access,
                    projections: self.projections,
                    span: self.span,
                },
                origin,
                target,
                access,
                span: self.span,
            },
        }
    }
}

impl CallableChecker<'_, '_> {
    pub(super) fn check_explicit_shared_pointee(
        &mut self,
        dereference: &ResolvedDereferenceExpr,
        projections: Vec<ObjectProjection>,
        span: Span,
    ) -> Option<CheckedSharedPointee> {
        if matches!(
            dereference.target,
            crate::resolve::ResolvedSharedTarget::OptionalBox(_)
        ) {
            self.diagnostics.push(
                Diagnostic::error(
                    crate::typeck::SHARED_OPTIONAL_BOX_UNAVAILABLE,
                    "shared optional-box pointee access is enabled in roadmap task BX5",
                )
                .with_primary_label(
                    dereference.operator_span,
                    "BX1 permits this dereference only as an exact `new P?(*source)` copy source",
                ),
            );
            return None;
        }
        let pointee = self.check_shared_pointee(&dereference.source, projections, span)?;
        let resolved_target = shared_target_view(crate::typeck::shared::lower_shared_target(
            dereference.target,
        ));
        if pointee.static_target() != resolved_target {
            self.diagnostics.push(
                Diagnostic::error(
                    crate::typeck::program::INVALID_OBJECT_CONTEXT,
                    "resolved dereference target does not match its shared owner",
                )
                .with_primary_label(
                    dereference.operator_span,
                    "dereference target is inconsistent with this owner",
                ),
            );
            return None;
        }
        Some(pointee)
    }

    fn check_shared_pointee(
        &mut self,
        expression: &ResolvedExpression,
        projections: Vec<ObjectProjection>,
        span: Span,
    ) -> Option<CheckedSharedPointee> {
        let source = self.check_shared_source(expression, false)?;
        self.check_shared_pointee_source(source, projections, span)
    }

    fn check_shared_pointee_source(
        &mut self,
        source: HirSharedSource,
        projections: Vec<ObjectProjection>,
        span: Span,
    ) -> Option<CheckedSharedPointee> {
        let target = shared_target_view(source.target());
        match source {
            HirSharedSource::Place(HirSharedPlace::Binding { binding, .. }) => {
                let access = self.binding_access(binding, false, span)?;
                Some(CheckedSharedPointee::stable(
                    binding,
                    target,
                    access,
                    projections,
                    span,
                ))
            }
            source => Some(CheckedSharedPointee {
                source: CheckedSharedPointeeSource::Anchored(source),
                target,
                access: HirAccess::Mutable,
                projections,
                span,
            }),
        }
    }
}

pub(super) const fn shared_target_view(target: HirSharedTarget) -> HirViewTarget {
    match target {
        HirSharedTarget::Obj => HirViewTarget::Obj,
        HirSharedTarget::Class(class) => HirViewTarget::Class(class),
        HirSharedTarget::Interface(interface) => HirViewTarget::Interface(interface),
        HirSharedTarget::Array(_) => {
            panic!("array pointees do not enter object-view conversion")
        }
        HirSharedTarget::OptionalBox(_) => {
            panic!("optional-box pointees are gated until roadmap task BX5")
        }
    }
}

pub(super) const fn view_shared_target(target: HirViewTarget) -> HirSharedTarget {
    match target {
        HirViewTarget::Obj => HirSharedTarget::Obj,
        HirViewTarget::Class(class) => HirSharedTarget::Class(class),
        HirViewTarget::Interface(interface) => HirSharedTarget::Interface(interface),
    }
}
