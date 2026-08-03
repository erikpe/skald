//! Zero-default static-field declaration validation and HIR lowering.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    hir::{HirStaticFieldDeclaration, Type},
    resolve::ResolvedStaticFieldDeclaration,
};

use super::{lower_type, INVALID_STATIC_FIELD_TYPE};

pub(super) fn lower_static_fields(
    fields: &[ResolvedStaticFieldDeclaration],
    diagnostics: &mut Diagnostics,
) -> Option<Vec<HirStaticFieldDeclaration>> {
    let mut valid = true;
    let fields = fields
        .iter()
        .map(|field| {
            let ty = lower_type(&field.type_syntax);
            if !has_zero_default(ty) {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_STATIC_FIELD_TYPE,
                        format!(
                            "static field `{}` cannot have type `{}`",
                            field.name,
                            ty.name()
                        ),
                    )
                    .with_primary_label(
                        field.type_syntax.span,
                        "this type has no complete all-zero live value",
                    )
                    .with_note(
                        "static fields support primitives, inline optionals, optional shared owners, and inline arrays",
                    ),
                );
                valid = false;
            }
            HirStaticFieldDeclaration {
                id: field.id,
                static_span: field.static_span,
                name: field.name.clone(),
                name_span: field.name_span,
                ty,
                span: field.span,
            }
        })
        .collect();
    valid.then_some(fields)
}

pub(super) const fn has_zero_default(ty: Type) -> bool {
    matches!(
        ty,
        Type::I64
            | Type::U64
            | Type::U8
            | Type::F64
            | Type::Bool
            | Type::Array(_)
            | Type::OptionalPrimitive(_)
            | Type::OptionalClass(_)
            | Type::OptionalShared(_)
    )
}
