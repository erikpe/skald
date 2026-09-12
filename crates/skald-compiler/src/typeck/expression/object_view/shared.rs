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

use crate::typeck::function::CallableChecker;

/// A checked shared owner together with the lifetime strategy required before
/// its pointee can be exposed as a non-owning object place.
///
/// Stable bindings borrow directly. Replaceable places and produced owners
/// retain their complete allocation through an explicit hidden anchor. The
/// owner target remains separate from the target selected by inline
/// projections so HIR lifetime provenance never loses the allocation root.
pub(in crate::typeck) struct CheckedSharedPointee {
    source: CheckedSharedPointeeSource,
    owner_target: HirViewTarget,
    target: HirViewTarget,
    dynamic_class: Option<crate::identity::ClassId>,
    access: HirAccess,
    projections: Vec<ObjectProjection>,
    span: Span,
}

// Anchored sources retain exact place evidence; keeping them inline avoids a
// heap allocation on every checked shared-pointee selection.
#[allow(clippy::large_enum_variant)]
enum CheckedSharedPointeeSource {
    Stable(BindingId),
    Anchored(HirSharedSource),
}

impl CheckedSharedPointee {
    pub(in crate::typeck) fn stable(
        binding: BindingId,
        owner_target: HirViewTarget,
        access: HirAccess,
        projections: Vec<ObjectProjection>,
        span: Span,
    ) -> Self {
        Self {
            source: CheckedSharedPointeeSource::Stable(binding),
            owner_target,
            target: owner_target,
            dynamic_class: None,
            access,
            projections,
            span,
        }
    }

    pub(in crate::typeck) const fn access(&self) -> HirAccess {
        self.access
    }

    pub(in crate::typeck) const fn span(&self) -> Span {
        self.span
    }

    pub(in crate::typeck) const fn static_target(&self) -> HirViewTarget {
        self.target
    }

    const fn owner_target(&self) -> HirViewTarget {
        self.owner_target
    }

    pub(in crate::typeck) fn exact_dynamic_class(&self) -> Option<crate::identity::ClassId> {
        self.dynamic_class
    }

    pub(in crate::typeck) const fn stable_binding(&self) -> Option<BindingId> {
        match &self.source {
            CheckedSharedPointeeSource::Stable(binding) => Some(*binding),
            CheckedSharedPointeeSource::Anchored(_) => None,
        }
    }

    pub(in crate::typeck) fn set_span(&mut self, span: Span) {
        self.span = span;
    }

    pub(in crate::typeck) fn select_target(
        &mut self,
        target: HirViewTarget,
        projections: impl IntoIterator<Item = ObjectProjection>,
    ) {
        self.target = target;
        self.projections.extend(projections);
    }

    pub(in crate::typeck) fn projections(&self) -> &[ObjectProjection] {
        &self.projections
    }

    pub(in crate::typeck) fn origin(&self) -> HirObjectOrigin {
        match &self.source {
            CheckedSharedPointeeSource::Stable(binding) => HirObjectOrigin::Shared {
                binding: *binding,
                static_target: self.owner_target,
                access: self.access,
                span: self.span,
            },
            CheckedSharedPointeeSource::Anchored(_) => HirObjectOrigin::AnchoredShared {
                static_target: self.owner_target,
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
                    target: self.owner_target,
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
                    target: self.owner_target,
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

    /// Force a strong owner anchor for a view retained across a loop body.
    /// Even a syntactically stable binding may be replaced by that body.
    pub(in crate::typeck) fn into_iteration_source(mut self) -> Self {
        if let Some(binding) = self.stable_binding() {
            self.source = CheckedSharedPointeeSource::Anchored(HirSharedSource::Place(
                HirSharedPlace::Binding {
                    binding,
                    target: view_shared_target(self.owner_target),
                    span: self.span,
                },
            ));
        }
        self
    }
}

impl CallableChecker<'_, '_> {
    pub(in crate::typeck) fn check_explicit_shared_pointee(
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
                    crate::typeck::program::INVALID_OBJECT_CONTEXT,
                    "an optional-box dereference yields an optional wrapper, not an object",
                )
                .with_primary_label(
                    dereference.operator_span,
                    "apply a presence test, copy the wrapper, or unwrap one optional layer",
                ),
            );
            return None;
        }
        let pointee = self.check_shared_pointee(&dereference.source, projections, span)?;
        let resolved_target = shared_target_view(crate::typeck::shared::lower_shared_target(
            dereference.target,
        ));
        if pointee.owner_target() != resolved_target {
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
        let owner_target = shared_target_view(source.target());
        let target = selected_target(self.program, owner_target, &projections);
        let dynamic_class =
            selected_dynamic_class(self.program, source.exact_dynamic_class(), &projections);
        match source {
            HirSharedSource::Place(HirSharedPlace::Binding { binding, .. }) => {
                let access = self.binding_access(binding, false, span)?;
                let mut pointee =
                    CheckedSharedPointee::stable(binding, owner_target, access, projections, span);
                pointee.target = target;
                Some(pointee)
            }
            source => Some(CheckedSharedPointee {
                source: CheckedSharedPointeeSource::Anchored(source),
                owner_target,
                target,
                dynamic_class,
                access: HirAccess::Mutable,
                projections,
                span,
            }),
        }
    }
}

fn selected_dynamic_class(
    program: &crate::resolve::ResolvedProgram,
    owner: Option<crate::identity::ClassId>,
    projections: &[ObjectProjection],
) -> Option<crate::identity::ClassId> {
    projections
        .iter()
        .fold(owner, |dynamic, projection| match projection {
            ObjectProjection::Base(_) => dynamic,
            ObjectProjection::Field(field) => Some(projected_field_class(program, *field)),
        })
}

fn selected_target(
    program: &crate::resolve::ResolvedProgram,
    owner: HirViewTarget,
    projections: &[ObjectProjection],
) -> HirViewTarget {
    projections
        .iter()
        .fold(owner, |_, projection| match projection {
            ObjectProjection::Base(class) => HirViewTarget::Class(*class),
            ObjectProjection::Field(field) => {
                HirViewTarget::Class(projected_field_class(program, *field))
            }
        })
}

fn projected_field_class(
    program: &crate::resolve::ResolvedProgram,
    field: crate::identity::FieldId,
) -> crate::identity::ClassId {
    let declaration = program
        .field(field)
        .expect("resolved shared projection must reference a field");
    let crate::resolve::ResolvedTypeKind::Class(class) = declaration.type_syntax.kind else {
        unreachable!("resolved shared object projection must have a class type")
    };
    class
}

pub(in crate::typeck::expression) const fn shared_target_view(
    target: HirSharedTarget,
) -> HirViewTarget {
    match target {
        HirSharedTarget::Obj => HirViewTarget::Obj,
        HirSharedTarget::Class(class) => HirViewTarget::Class(class),
        HirSharedTarget::Interface(interface) => HirViewTarget::Interface(interface),
        HirSharedTarget::Array(_) => {
            panic!("array pointees do not enter object-view conversion")
        }
        HirSharedTarget::OptionalBox(_) => {
            panic!("optional-box pointees do not enter object-view conversion")
        }
    }
}

pub(in crate::typeck::expression) const fn view_shared_target(
    target: HirViewTarget,
) -> HirSharedTarget {
    match target {
        HirViewTarget::Obj => HirSharedTarget::Obj,
        HirViewTarget::Class(class) => HirSharedTarget::Class(class),
        HirViewTarget::Interface(interface) => HirSharedTarget::Interface(interface),
    }
}
