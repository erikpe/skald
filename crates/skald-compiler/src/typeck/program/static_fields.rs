//! Static-field storage validation and typed initializer selection.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    hir::{HirStaticFieldDeclaration, HirStaticFieldInitializer, Type},
    resolve::ResolvedStaticFieldDeclaration,
    typeck::{capabilities::CopyCapabilities, function::CallableChecker},
};

use super::{lower_type, INVALID_STATIC_FIELD_TYPE};

pub(super) fn lower_static_fields(
    program: &crate::resolve::ResolvedProgram,
    copy_capabilities: &CopyCapabilities,
    fields: &[ResolvedStaticFieldDeclaration],
    diagnostics: &mut Diagnostics,
) -> Option<Vec<HirStaticFieldDeclaration>> {
    let mut valid = true;
    let fields = fields
        .iter()
        .map(|field| {
            let ty = lower_type(program, &field.type_syntax);
            let initializer = if let Some(initializer) = &field.initializer {
                if !is_stored_value_type(ty) {
                    report_invalid_explicit_storage(field, ty, diagnostics);
                    valid = false;
                    None
                } else {
                    let value = CallableChecker::new_static_initializer(
                        program,
                        copy_capabilities,
                        initializer,
                        diagnostics,
                    )
                    .check_static_initializer(ty, &initializer.expression);
                    if value.is_none() {
                        valid = false;
                    }
                    value.map(|value| HirStaticFieldInitializer {
                        id: initializer.id,
                        equal_span: initializer.equal_span,
                        value,
                        span: initializer.span,
                    })
                }
            } else if !has_zero_default(ty) {
                report_missing_zero_default(field, ty, diagnostics);
                valid = false;
                None
            } else {
                None
            };
            HirStaticFieldDeclaration {
                id: field.id,
                static_span: field.static_span,
                name: field.name.clone(),
                name_span: field.name_span,
                ty,
                initializer,
                span: field.span,
            }
        })
        .collect();
    valid.then_some(fields)
}

fn report_missing_zero_default(
    field: &ResolvedStaticFieldDeclaration,
    ty: Type,
    diagnostics: &mut Diagnostics,
) {
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
            "add an explicit initializer or use a primitive, inline optional, optional shared owner, or inline array",
        ),
    );
}

fn report_invalid_explicit_storage(
    field: &ResolvedStaticFieldDeclaration,
    ty: Type,
    diagnostics: &mut Diagnostics,
) {
    diagnostics.push(
        Diagnostic::error(
            INVALID_STATIC_FIELD_TYPE,
            format!(
                "static field `{}` cannot store type `{}`",
                field.name,
                ty.name()
            ),
        )
        .with_primary_label(
            field.type_syntax.span,
            "`unit`, `Obj`, and interfaces are not stored value types",
        )
        .with_note("an explicit initializer does not make a non-owning view into stored data"),
    );
}

pub(in crate::typeck) const fn is_stored_value_type(ty: Type) -> bool {
    !matches!(ty, Type::Unit | Type::Obj | Type::Interface(_))
}

pub(in crate::typeck) const fn has_zero_default(ty: Type) -> bool {
    matches!(
        ty,
        Type::I64
            | Type::U64
            | Type::U8
            | Type::F64
            | Type::Bool
            | Type::Array(_)
            | Type::Optional(_)
    )
}
