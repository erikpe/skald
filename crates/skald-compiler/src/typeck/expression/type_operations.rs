//! Type-test classification and checked non-owning narrowing.

use super::{alias::ViewSourceUse, *};
use crate::{
    hir::{
        HirAccess, HirExpressionKind, HirNarrowingFailure, HirNarrowingKind, HirObjectView,
        HirTypeTest, HirTypeTestKind, HirViewTarget,
    },
    identity::ClassId,
    resolve::{ResolvedNarrowing, ResolvedTypeTestExpr},
    typeck::program::{
        lower_type, INSUFFICIENT_ALIAS_ACCESS, INVALID_NARROWING, INVALID_TYPE_TEST,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TypeRelation {
    StaticSuccess,
    StaticFailure,
    Runtime,
}

impl CallableChecker<'_, '_> {
    pub(super) fn check_type_test(&mut self, test: &ResolvedTypeTestExpr) -> Option<HirExpression> {
        let source = self.check_object_view_source(&test.source, ViewSourceUse::TypeTest)?;
        let target = self.check_view_target(&test.target, test.target_span, INVALID_TYPE_TEST)?;
        let relation = self.classify_type_relation(&source, target);
        let kind = match relation {
            TypeRelation::StaticSuccess => HirTypeTestKind::StaticSuccess,
            TypeRelation::StaticFailure => HirTypeTestKind::StaticFailure,
            TypeRelation::Runtime => HirTypeTestKind::Runtime,
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

        let relation = self.classify_type_relation(&source, target);
        if relation == TypeRelation::StaticFailure {
            self.diagnostics.push(
                Diagnostic::error(INVALID_NARROWING, "checked narrowing can never succeed")
                    .with_primary_label(target_span, "no possible dynamic class provides this view")
                    .with_secondary_label(source.span(), "source view"),
            );
            return None;
        }

        let access = if mutable {
            HirAccess::Mutable
        } else {
            HirAccess::ReadOnly
        };
        if !source.access().permits(access) {
            self.diagnostics.push(
                Diagnostic::error(
                    INSUFFICIENT_ALIAS_ACCESS,
                    "checked narrowing cannot increase alias access",
                )
                .with_primary_label(source.span(), "this source provides read-only access")
                .with_secondary_label(name_span, "mutable narrowed alias requested here"),
            );
            return None;
        }

        let kind = match relation {
            TypeRelation::StaticSuccess => HirNarrowingKind::Static,
            TypeRelation::Runtime => HirNarrowingKind::Runtime {
                failure: HirNarrowingFailure::Terminate,
            },
            TypeRelation::StaticFailure => unreachable!("static failure returned above"),
        };
        let view = match (relation, source, target) {
            (
                TypeRelation::StaticSuccess,
                alias::CheckedObjectViewSource::Class { place, origin },
                HirViewTarget::Class(target),
            ) => {
                let place = self
                    .project_place_to_ancestor(place, target)
                    .expect("statically successful class narrowing must select an ancestor");
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
        Some((view, kind))
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

    fn classify_type_relation(
        &self,
        source: &alias::CheckedObjectViewSource,
        target: HirViewTarget,
    ) -> TypeRelation {
        if target == HirViewTarget::Obj {
            return TypeRelation::StaticSuccess;
        }
        if let Some(class) = source.exact_dynamic_class() {
            return if self.class_provides_view(class, target) {
                TypeRelation::StaticSuccess
            } else {
                TypeRelation::StaticFailure
            };
        }
        if self.view_guarantees_target(source.static_target(), target) {
            return TypeRelation::StaticSuccess;
        }

        let mut any_source = false;
        let mut any_success = false;
        let mut any_failure = false;
        for class in self.program.classes.iter().map(|class| class.id) {
            if !self.class_can_inhabit_view(class, source.static_target()) {
                continue;
            }
            any_source = true;
            if self.class_provides_view(class, target) {
                any_success = true;
            } else {
                any_failure = true;
            }
        }
        match (any_source, any_success, any_failure) {
            (_, true, true) => TypeRelation::Runtime,
            (_, true, false) => TypeRelation::StaticSuccess,
            _ => TypeRelation::StaticFailure,
        }
    }

    fn view_guarantees_target(&self, source: HirViewTarget, target: HirViewTarget) -> bool {
        match (source, target) {
            (_, HirViewTarget::Obj) => true,
            (HirViewTarget::Class(class), target) => self.class_provides_view(class, target),
            (HirViewTarget::Interface(source), HirViewTarget::Interface(target)) => {
                source == target
            }
            (HirViewTarget::Interface(_), HirViewTarget::Class(_))
            | (HirViewTarget::Obj, HirViewTarget::Class(_))
            | (HirViewTarget::Obj, HirViewTarget::Interface(_)) => false,
        }
    }

    fn class_can_inhabit_view(&self, class: ClassId, view: HirViewTarget) -> bool {
        match view {
            HirViewTarget::Class(target) => self
                .program
                .hierarchy
                .is_subtype(class, target)
                .unwrap_or(false),
            HirViewTarget::Interface(interface) => self.class_conforms_to(class, interface),
            HirViewTarget::Obj => true,
        }
    }

    fn class_provides_view(&self, class: ClassId, target: HirViewTarget) -> bool {
        self.class_can_inhabit_view(class, target)
    }
}
