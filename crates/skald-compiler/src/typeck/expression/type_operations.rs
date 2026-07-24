//! Type-test checking and checked non-owning view selection.

use super::{
    alias::ViewSourceUse,
    object_view_relation::{classify_object_view_relation, ObjectViewRelation},
    *,
};
use crate::{
    hir::{
        HirAccess, HirExpressionKind, HirNarrowingFailure, HirNarrowingKind, HirObjectView,
        HirTypeTest, HirTypeTestKind, HirViewTarget,
    },
    resolve::{ResolvedNarrowing, ResolvedTypeTestExpr},
    typeck::program::{
        lower_type, INSUFFICIENT_ALIAS_ACCESS, INVALID_NARROWING, INVALID_TYPE_TEST,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CheckedViewFailure {
    Terminate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CheckedViewKind {
    Static,
    Runtime { failure: CheckedViewFailure },
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

    pub(in crate::typeck) fn check_narrowing_operation(
        &mut self,
        narrowing: &ResolvedNarrowing,
    ) -> Option<(HirObjectView, HirNarrowingKind)> {
        let source = self.check_object_view_source(&narrowing.source, ViewSourceUse::Narrowing)?;
        let alias = self.narrowed_alias(narrowing.binding);
        let target_syntax = alias.target.clone();
        let target_span = alias.target.span;
        let name_span = alias.name_span;
        let mutable = alias.mutable;
        let target = self.check_view_target(&target_syntax, target_span, INVALID_NARROWING)?;
        if target == HirViewTarget::Obj {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_NARROWING,
                    "checked narrowing must select a class or interface view",
                )
                .with_primary_label(target_span, "`Obj` does not narrow an object view"),
            );
            return None;
        }

        let access = if mutable {
            HirAccess::Mutable
        } else {
            HirAccess::ReadOnly
        };
        let source_span = source.span();
        let operation = match self.select_checked_view(source, target, access) {
            Ok(operation) => operation,
            Err(CheckedViewRejection::StaticFailure) => {
                self.diagnostics.push(
                    Diagnostic::error(INVALID_NARROWING, "checked narrowing can never succeed")
                        .with_primary_label(
                            target_span,
                            "no possible dynamic class provides this view",
                        )
                        .with_secondary_label(source_span, "source view"),
                );
                return None;
            }
            Err(CheckedViewRejection::InsufficientAccess) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INSUFFICIENT_ALIAS_ACCESS,
                        "checked narrowing cannot increase alias access",
                    )
                    .with_primary_label(source_span, "this source provides read-only access")
                    .with_secondary_label(name_span, "mutable narrowed alias requested here"),
                );
                return None;
            }
        };
        let kind = match operation.kind {
            CheckedViewKind::Static => HirNarrowingKind::Static,
            CheckedViewKind::Runtime {
                failure: CheckedViewFailure::Terminate,
            } => HirNarrowingKind::Runtime {
                failure: HirNarrowingFailure::Terminate,
            },
        };
        Some((operation.view, kind))
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
            ObjectViewRelation::Runtime => CheckedViewKind::Runtime {
                failure: CheckedViewFailure::Terminate,
            },
            ObjectViewRelation::StaticFailure => {
                unreachable!("statically impossible views returned above")
            }
        };
        Ok(CheckedViewOperation { view, kind })
    }
}
