//! Type-test checking and checked non-owning view selection.

use super::{
    alias::ViewSourceUse,
    object_view_relation::{classify_object_view_relation, ObjectViewRelation},
    *,
};
use crate::{
    hir::{
        HirAccess, HirCheckedObjectView, HirCheckedObjectViewKind, HirExpressionKind,
        HirObjectView, HirTypeTest, HirTypeTestKind, HirViewTarget,
    },
    resolve::{ResolvedObjectCastExpr, ResolvedObjectCastTargetMode, ResolvedTypeTestExpr},
    typeck::program::{lower_type, INVALID_OBJECT_CAST, INVALID_TYPE_TEST},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CheckedViewKind {
    Static,
    Runtime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CheckedViewOperation {
    view: HirObjectView,
    kind: CheckedViewKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CheckedViewRejection {
    StaticFailure,
    InsufficientAccess,
}

impl CallableChecker<'_, '_> {
    pub(in crate::typeck) fn check_object_cast(
        &mut self,
        cast: &ResolvedObjectCastExpr,
    ) -> Option<HirCheckedObjectView> {
        if let ResolvedObjectCastTargetMode::Shared { shared_span } = cast.target_mode {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_OBJECT_CAST,
                    "shared-owner casts are not implemented",
                )
                .with_primary_label(
                    shared_span,
                    "`shared T` is reserved for a future ownership slice",
                )
                .with_note("plain `(T) source` casts produce only a bounded borrowed place"),
            );
            return None;
        }
        let source = self.check_object_view_source(&cast.source, ViewSourceUse::Cast)?;
        let target = self.check_view_target(&cast.target, cast.target_span, INVALID_OBJECT_CAST)?;
        let source_span = source.span();
        let access = source.access();
        let operation = match self.select_checked_view(source, target, access) {
            Ok(operation) => operation,
            Err(CheckedViewRejection::StaticFailure) => {
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
            Err(CheckedViewRejection::InsufficientAccess) => {
                unreachable!("a plain object cast preserves the source access")
            }
        };
        let projections = match (&operation.view.source, target) {
            (crate::hir::HirViewSource::Produced(producer), HirViewTarget::Class(target))
                if producer.class() != target =>
            {
                self.program
                    .hierarchy
                    .base_chain(producer.class())
                    .expect("statically successful produced cast must have valid ancestry")
                    .take_while(|base| *base != target)
                    .chain(std::iter::once(target))
                    .map(crate::object_path::ObjectProjection::Base)
                    .collect()
            }
            _ => Vec::new(),
        };
        Some(HirCheckedObjectView {
            class: match target {
                HirViewTarget::Class(class) => Some(class),
                HirViewTarget::Interface(_) | HirViewTarget::Obj => None,
            },
            view: operation.view,
            consumer_target: target,
            consumer_access: access,
            kind: match operation.kind {
                CheckedViewKind::Static => HirCheckedObjectViewKind::Static,
                CheckedViewKind::Runtime => HirCheckedObjectViewKind::RuntimeTerminate,
            },
            projections,
            span: cast.span,
        })
    }

    pub(super) fn check_type_test(&mut self, test: &ResolvedTypeTestExpr) -> Option<HirExpression> {
        let source = self.check_object_view_source(&test.source, ViewSourceUse::TypeTest)?;
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
        Some(HirExpression {
            kind: HirExpressionKind::TypeTest(HirTypeTest {
                source: source.into_view(source_target, access),
                target,
                kind,
            }),
            ty: Type::Bool,
            span: test.span,
        })
    }

    fn check_view_target(
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

    fn select_checked_view(
        &self,
        source: alias::CheckedObjectViewSource,
        target: HirViewTarget,
        access: HirAccess,
    ) -> Result<CheckedViewOperation, CheckedViewRejection> {
        let relation =
            classify_object_view_relation(self.program, source.relation_source(), target);
        if relation == ObjectViewRelation::StaticFailure {
            return Err(CheckedViewRejection::StaticFailure);
        }
        if !source.access().permits(access) {
            return Err(CheckedViewRejection::InsufficientAccess);
        }

        let view = match (relation, source, target) {
            (
                ObjectViewRelation::StaticSuccess,
                alias::CheckedObjectViewSource::Class { place, origin },
                HirViewTarget::Class(target),
            ) => {
                let place = self
                    .project_place_to_ancestor(place, target)
                    .expect("statically successful class view must select an ancestor");
                HirObjectView {
                    span: place.span(),
                    source: crate::hir::HirViewSource::Place(place),
                    origin: Box::new(origin),
                    target: HirViewTarget::Class(target),
                    access,
                }
            }
            (_, source, target) => source.into_view(target, access),
        };
        let kind = match relation {
            ObjectViewRelation::StaticSuccess => CheckedViewKind::Static,
            ObjectViewRelation::Runtime => CheckedViewKind::Runtime,
            ObjectViewRelation::StaticFailure => {
                unreachable!("statically impossible views returned above")
            }
        };
        Ok(CheckedViewOperation { view, kind })
    }
}
