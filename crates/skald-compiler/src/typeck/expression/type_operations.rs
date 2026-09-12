//! Type-test checking and checked non-owning view selection.

use super::object_view::{
    plan_checked_object_view, plan_resolved_object_view, CheckedObjectViewPlanKind,
    CheckedObjectViewProblem, ObjectViewRequest, ObjectViewRetention, ObjectViewSourceAdmission,
    ObjectViewSourceDiagnosticContext,
};
use super::*;

use crate::{
    hir::{
        HirAccess, HirCheckedObjectView, HirCheckedObjectViewKind, HirExpressionKind, HirTypeTest,
        HirTypeTestKind, HirViewTarget,
    },
    resolve::{ResolvedObjectCastExpr, ResolvedObjectCastTargetMode, ResolvedTypeTestExpr},
    typeck::{
        conversion::lower_type,
        diagnostic_codes::{INVALID_COPY_CONSTRUCTION, INVALID_OBJECT_CAST, INVALID_TYPE_TEST},
    },
};

const COPY_OBJECT_VIEW_SOURCE: ObjectViewSourceDiagnosticContext =
    ObjectViewSourceDiagnosticContext::new(
        "copy-construction source",
        INVALID_COPY_CONSTRUCTION,
        "copy-construction source must designate an object",
        "copy-construction source must be an object place or produced object",
    );

const CAST_OBJECT_VIEW_SOURCE: ObjectViewSourceDiagnosticContext =
    ObjectViewSourceDiagnosticContext::new(
        "object-cast source",
        INVALID_OBJECT_CAST,
        "object-cast source must designate an object",
        "object-cast source must be an existing object place",
    );

const TYPE_TEST_OBJECT_VIEW_SOURCE: ObjectViewSourceDiagnosticContext =
    ObjectViewSourceDiagnosticContext::new(
        "type-test source",
        INVALID_TYPE_TEST,
        "type-test source must designate an object",
        "type-test source must be an existing object place",
    );

impl CallableChecker<'_, '_> {
    pub(in crate::typeck) fn check_copy_construction_view(
        &mut self,
        expression: &crate::resolve::ResolvedExpression,
        target: crate::identity::ClassId,
        target_span: Span,
        span: Span,
    ) -> Option<HirCheckedObjectView> {
        let source = self.check_object_view_source(
            expression,
            ObjectViewSourceAdmission::ExistingOrProducedObject,
            COPY_OBJECT_VIEW_SOURCE,
        )?;
        let source_span = source.span();
        let target_class = target;
        let target = HirViewTarget::Class(target_class);
        let request = ObjectViewRequest::new(
            target,
            HirAccess::ReadOnly,
            ObjectViewRetention::ImmediateConsumer,
        );
        let operation = match plan_checked_object_view(self.program, source, request) {
            Ok(operation) => operation,
            Err(CheckedObjectViewProblem::IncompatibleTarget) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_COPY_CONSTRUCTION,
                        "copy-construction source can never provide the target class",
                    )
                    .with_primary_label(
                        target_span,
                        "no possible dynamic class provides this object",
                    )
                    .with_secondary_label(source_span, "source view"),
                );
                return None;
            }
            Err(CheckedObjectViewProblem::InsufficientAccess) => {
                unreachable!("every object source permits read-only copy access")
            }
        };
        let (view, kind, projections) = operation.into_parts();
        Some(HirCheckedObjectView {
            class: Some(target_class),
            view,
            consumer_target: target,
            consumer_access: HirAccess::ReadOnly,
            kind: match kind {
                CheckedObjectViewPlanKind::Static => HirCheckedObjectViewKind::Static,
                CheckedObjectViewPlanKind::Runtime => HirCheckedObjectViewKind::RuntimeTerminate,
            },
            projections,
            span,
        })
    }

    pub(in crate::typeck) fn check_object_cast(
        &mut self,
        cast: &ResolvedObjectCastExpr,
    ) -> Option<HirCheckedObjectView> {
        self.check_object_cast_with_retention(cast, ObjectViewRetention::ImmediateConsumer)
    }

    pub(in crate::typeck) fn check_loop_object_cast(
        &mut self,
        cast: &ResolvedObjectCastExpr,
    ) -> Option<HirCheckedObjectView> {
        self.check_object_cast_with_retention(cast, ObjectViewRetention::LoopBody)
    }

    fn check_object_cast_with_retention(
        &mut self,
        cast: &ResolvedObjectCastExpr,
        retention: ObjectViewRetention,
    ) -> Option<HirCheckedObjectView> {
        if let ResolvedObjectCastTargetMode::Shared { shared_span } = cast.target_mode {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_OBJECT_CAST,
                    "a shared-owner cast must be consumed as a shared value",
                )
                .with_primary_label(shared_span, "this context requires a non-owning place")
                .with_note("store, pass, or return `(shared T) source` as `shared T`")
                .with_note("plain `(T) source` casts produce a bounded borrowed place"),
            );
            return None;
        }
        let source = self.check_object_view_source(
            &cast.source,
            ObjectViewSourceAdmission::ExistingOrProducedObject,
            CAST_OBJECT_VIEW_SOURCE,
        )?;
        let target = self.check_view_target(&cast.target, cast.target_span, INVALID_OBJECT_CAST)?;
        let source_span = source.span();
        let access = source.access();
        let request = ObjectViewRequest::new(target, access, retention);
        let operation = match plan_checked_object_view(self.program, source, request) {
            Ok(operation) => operation,
            Err(CheckedObjectViewProblem::IncompatibleTarget) => {
                self.diagnostics.push(
                    Diagnostic::error(INVALID_OBJECT_CAST, "object cast can never succeed")
                        .with_primary_label(
                            cast.target_span,
                            "no possible dynamic class provides this view",
                        )
                        .with_secondary_label(source_span, "source view"),
                );
                return None;
            }
            Err(CheckedObjectViewProblem::InsufficientAccess) => {
                unreachable!("a plain object cast preserves the source access")
            }
        };
        let (view, kind, projections) = operation.into_parts();
        Some(HirCheckedObjectView {
            class: match target {
                HirViewTarget::Class(class) => Some(class),
                HirViewTarget::Interface(_) | HirViewTarget::Obj => None,
            },
            view,
            consumer_target: target,
            consumer_access: access,
            kind: match kind {
                CheckedObjectViewPlanKind::Static => HirCheckedObjectViewKind::Static,
                CheckedObjectViewPlanKind::Runtime => HirCheckedObjectViewKind::RuntimeTerminate,
            },
            projections,
            span: cast.span,
        })
    }

    pub(super) fn check_type_test(&mut self, test: &ResolvedTypeTestExpr) -> Option<HirExpression> {
        let source = self.check_object_view_source(
            &test.source,
            ObjectViewSourceAdmission::ExistingObjectPlace,
            TYPE_TEST_OBJECT_VIEW_SOURCE,
        )?;
        let target = self.check_view_target(&test.target, test.target_span, INVALID_TYPE_TEST)?;
        let relation =
            classify_object_view_relation(self.program, source.relation_source(), target);
        let kind = match relation {
            ObjectViewRelation::StaticSuccess => HirTypeTestKind::StaticSuccess,
            ObjectViewRelation::StaticFailure => HirTypeTestKind::StaticFailure,
            ObjectViewRelation::Runtime => HirTypeTestKind::Runtime,
        };
        let access = source.access();
        let source_target = source.static_target();
        let source = plan_resolved_object_view(
            self.program,
            source,
            ObjectViewRequest::new(
                source_target,
                access,
                ObjectViewRetention::ImmediateConsumer,
            ),
        );
        Some(HirExpression {
            kind: HirExpressionKind::TypeTest(HirTypeTest {
                source,
                target,
                kind,
            }),
            ty: Type::Bool,
            span: test.span,
        })
    }

    pub(in crate::typeck) fn check_view_target(
        &mut self,
        target: &ResolvedType,
        span: Span,
        diagnostic_code: &'static str,
    ) -> Option<HirViewTarget> {
        match lower_type(target) {
            Type::Class(class) => Some(HirViewTarget::Class(class)),
            Type::Interface(interface) => Some(HirViewTarget::Interface(interface)),
            Type::Obj => Some(HirViewTarget::Obj),
            primitive => {
                self.diagnostics.push(
                    Diagnostic::error(
                        diagnostic_code,
                        "type-operation target must be a class, interface, or `Obj`",
                    )
                    .with_primary_label(span, format!("`{}` is a value type", primitive.name())),
                );
                None
            }
        }
    }
}
