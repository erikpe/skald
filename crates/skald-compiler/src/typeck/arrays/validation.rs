//! Array element and signature eligibility diagnostics.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    hir::Type,
    resolve::{ResolvedProgram, ResolvedType, ResolvedTypeKind},
};

use super::super::program::lower_type;

pub const INVALID_ARRAY_ELEMENT: &str = "TYP036";

pub(in crate::typeck) fn validate_array_types(
    program: &ResolvedProgram,
    diagnostics: &mut Diagnostics,
) {
    for array in program.array_types.iter() {
        let element = lower_type(program, &array.element);
        if !is_array_element(element) {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_ARRAY_ELEMENT,
                    format!("`{}` cannot be stored as an array element", element.name()),
                )
                .with_primary_label(
                    array.element.span,
                    "array elements must be owning, storable values",
                ),
            );
        }
    }

    for function in program.declarations.iter() {
        if matches!(
            function.linkage,
            crate::resolve::ResolvedFunctionLinkage::External { .. }
        ) {
            for parameter in &function.parameters {
                reject_external_array(program, &parameter.type_syntax, diagnostics);
            }
            reject_external_array(program, &function.return_type, diagnostics);
        }
    }
}

pub(in crate::typeck) const fn is_array_element(ty: Type) -> bool {
    crate::type_capabilities::supports_array_element(super::super::type_category(ty))
}

fn reject_external_array(
    program: &ResolvedProgram,
    ty: &ResolvedType,
    diagnostics: &mut Diagnostics,
) {
    if resolved_type_contains_array(program, ty.kind) {
        diagnostics.push(
            Diagnostic::error(
                super::super::program::INVALID_EXTERNAL_DECLARATION,
                "external array signatures are not supported",
            )
            .with_primary_label(ty.span, "arrays have no external ABI mapping"),
        );
    }
}

pub(in crate::typeck) fn resolved_type_contains_array(
    program: &ResolvedProgram,
    kind: ResolvedTypeKind,
) -> bool {
    match kind {
        ResolvedTypeKind::Array(_)
        | ResolvedTypeKind::Shared(crate::resolve::ResolvedSharedTarget::Array(_)) => true,
        ResolvedTypeKind::Optional(optional) => program
            .optional_types
            .get(optional)
            .is_some_and(|entry| resolved_type_contains_array(program, entry.payload.kind)),
        _ => false,
    }
}
