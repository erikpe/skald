//! Primitive static-place checking shared by reads, writes, and aliases.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirExpression, HirExpressionKind, HirPrimitivePlace, HirPrimitiveStorage, HirStaticPlace,
        Type,
    },
    identity::StaticFieldId,
    resolve::ResolvedStaticFieldAccessExpr,
    source::Span,
};

use super::super::{
    function::CallableChecker,
    program::{lower_type, source_use_is_enabled, STATIC_FIELD_USE_UNAVAILABLE},
};

impl CallableChecker<'_, '_> {
    pub(super) fn check_static_field_read(
        &mut self,
        access: &ResolvedStaticFieldAccessExpr,
    ) -> Option<HirExpression> {
        let (place, ty) =
            self.check_primitive_static_place(access.field, access.member_span, access.span)?;
        Some(HirExpression {
            kind: HirExpressionKind::StaticRead(place),
            ty,
            span: access.span,
        })
    }

    pub(in crate::typeck) fn check_primitive_static_place(
        &mut self,
        field: StaticFieldId,
        member_span: Span,
        span: Span,
    ) -> Option<(HirStaticPlace, Type)> {
        let declaration = self
            .program
            .static_field(field)
            .expect("resolved static-field use must reference a declaration");
        let ty = lower_type(&declaration.type_syntax);
        if !source_use_is_enabled(ty) {
            self.diagnostics.push(
                Diagnostic::error(
                    STATIC_FIELD_USE_UNAVAILABLE,
                    format!(
                        "static field `{}` cannot be used as `{}` storage yet",
                        declaration.name,
                        ty.name()
                    ),
                )
                .with_primary_label(
                    member_span,
                    "only primitive static fields are currently usable",
                )
                .with_secondary_label(declaration.name_span, "static field declared here"),
            );
            return None;
        }
        Some((HirStaticPlace { field, span }, ty))
    }

    pub(super) fn primitive_static_alias_place(
        &mut self,
        access: &ResolvedStaticFieldAccessExpr,
    ) -> Option<(HirPrimitivePlace, Type)> {
        let (place, ty) =
            self.check_primitive_static_place(access.field, access.member_span, access.span)?;
        Some((
            HirPrimitivePlace {
                storage: HirPrimitiveStorage::Static(place),
                span: access.span,
            },
            ty,
        ))
    }
}
