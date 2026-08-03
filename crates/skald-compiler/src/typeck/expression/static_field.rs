//! Primitive static-place checking shared by reads, writes, and aliases.

use crate::{
    hir::{
        HirExpression, HirExpressionKind, HirPrimitivePlace, HirPrimitiveStorage, HirStaticPlace,
        Type,
    },
    identity::StaticFieldId,
    resolve::ResolvedStaticFieldAccessExpr,
    source::Span,
};

use super::super::{function::CallableChecker, program::lower_type};

impl CallableChecker<'_, '_> {
    pub(super) fn check_static_field_read(
        &mut self,
        access: &ResolvedStaticFieldAccessExpr,
    ) -> Option<HirExpression> {
        let (place, ty) = self.check_static_place(access.field, access.span)?;
        if !matches!(
            ty,
            Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool | Type::Array(_)
        ) {
            return None;
        }
        Some(HirExpression {
            kind: HirExpressionKind::StaticRead(place),
            ty,
            span: access.span,
        })
    }

    pub(in crate::typeck) fn check_primitive_static_place(
        &mut self,
        field: StaticFieldId,
        span: Span,
    ) -> Option<(HirStaticPlace, Type)> {
        let (place, ty) = self.check_static_place(field, span)?;
        if !matches!(
            ty,
            Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool
        ) {
            return None;
        }
        Some((place, ty))
    }

    pub(in crate::typeck) fn check_static_place(
        &mut self,
        field: StaticFieldId,
        span: Span,
    ) -> Option<(HirStaticPlace, Type)> {
        let declaration = self
            .program
            .static_field(field)
            .expect("resolved static-field use must reference a declaration");
        let ty = lower_type(&declaration.type_syntax);
        Some((HirStaticPlace { field, span }, ty))
    }

    pub(super) fn primitive_static_alias_place(
        &mut self,
        access: &ResolvedStaticFieldAccessExpr,
    ) -> Option<(HirPrimitivePlace, Type)> {
        let (place, ty) = self.check_primitive_static_place(access.field, access.span)?;
        Some((
            HirPrimitivePlace {
                storage: HirPrimitiveStorage::Static(place),
                span: access.span,
            },
            ty,
        ))
    }
}
