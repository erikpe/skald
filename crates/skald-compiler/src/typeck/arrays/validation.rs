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
        let element = lower_type(&array.element);
        if matches!(element, Type::Unit | Type::Obj | Type::Interface(_)) {
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
                reject_external_array(&parameter.type_syntax, diagnostics);
            }
            reject_external_array(&function.return_type, diagnostics);
        }
    }

    for interface in program.interfaces.iter() {
        for requirement in &interface.requirements {
            for parameter in &requirement.parameters {
                if resolved_type_contains_array(parameter.type_syntax.kind) {
                    diagnostics.push(
                        Diagnostic::error(
                            super::super::program::INVALID_INTERFACE_REQUIREMENT,
                            "array types are not supported in interface requirements",
                        )
                        .with_primary_label(
                            parameter.type_syntax.span,
                            "arrays do not participate in interface dispatch contracts",
                        ),
                    );
                }
            }
            if resolved_type_contains_array(requirement.return_type.kind) {
                diagnostics.push(
                    Diagnostic::error(
                        super::super::program::INVALID_INTERFACE_REQUIREMENT,
                        "array types are not supported in interface requirements",
                    )
                    .with_primary_label(
                        requirement.return_type.span,
                        "arrays do not participate in interface dispatch contracts",
                    ),
                );
            }
        }
    }
}

fn reject_external_array(ty: &ResolvedType, diagnostics: &mut Diagnostics) {
    if resolved_type_contains_array(ty.kind) {
        diagnostics.push(
            Diagnostic::error(
                super::super::program::INVALID_EXTERNAL_DECLARATION,
                "external array signatures are not supported",
            )
            .with_primary_label(ty.span, "arrays have no external ABI mapping"),
        );
    }
}

pub(in crate::typeck) const fn resolved_type_contains_array(kind: ResolvedTypeKind) -> bool {
    matches!(
        kind,
        ResolvedTypeKind::Array(_)
            | ResolvedTypeKind::Shared(crate::resolve::ResolvedSharedTarget::Array(_))
            | ResolvedTypeKind::OptionalShared {
                target: crate::resolve::ResolvedSharedTarget::Array(_),
                ..
            }
    )
}
