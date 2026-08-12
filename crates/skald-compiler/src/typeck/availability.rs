//! Deliberate availability boundaries for staged shared optional boxes.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    resolve::{
        ResolvedFunctionLinkage, ResolvedMemberDefinition, ResolvedProgram, ResolvedType,
        ResolvedTypeKind,
    },
};

pub const SHARED_OPTIONAL_BOX_UNAVAILABLE: &str = "TYP044";

/// Keeps shared optional boxes out of the deliberately unsupported external
/// ABI and array-element boundary. Ordinary internal stored and callable
/// positions use the same ownership machinery as every other shared target.
pub(super) fn validate(program: &ResolvedProgram, diagnostics: &mut Diagnostics) -> bool {
    let mut valid = true;

    for declaration in program.declarations.iter() {
        let external = matches!(
            declaration.linkage,
            ResolvedFunctionLinkage::External { .. }
        );
        if external {
            for parameter in &declaration.parameters {
                valid &= reject_if_box(
                    program,
                    &parameter.type_syntax,
                    "external signatures cannot contain shared optional boxes",
                    diagnostics,
                );
            }
            valid &= reject_if_box(
                program,
                &declaration.return_type,
                "external signatures cannot contain shared optional boxes",
                diagnostics,
            );
        }
    }

    for definition in program.definitions.iter() {
        valid &= validate_locals(program, &definition.locals, diagnostics);
    }
    for definition in program.class_definitions.iter() {
        for member in definition.initializers.iter() {
            valid &= validate_member_locals(program, member, diagnostics);
        }
        if let Some(member) = &definition.copy_constructor {
            valid &= validate_member_locals(program, member, diagnostics);
        }
        if let Some(member) = &definition.copy_assignment {
            valid &= validate_member_locals(program, member, diagnostics);
        }
        if let Some(member) = &definition.destructor {
            valid &= validate_member_locals(program, member, diagnostics);
        }
        for member in &definition.methods {
            valid &= validate_member_locals(program, member, diagnostics);
        }
    }

    valid
}

fn validate_member_locals(
    program: &ResolvedProgram,
    member: &ResolvedMemberDefinition,
    diagnostics: &mut Diagnostics,
) -> bool {
    validate_locals(program, &member.locals, diagnostics)
}

fn validate_locals(
    program: &ResolvedProgram,
    locals: &[crate::resolve::ResolvedLocal],
    diagnostics: &mut Diagnostics,
) -> bool {
    let mut valid = true;
    for local in locals {
        if type_contains_box(program, local.type_syntax.kind)
            && !is_local_box_owner(program, local.type_syntax.kind)
        {
            diagnostics.push(
                Diagnostic::error(
                    SHARED_OPTIONAL_BOX_UNAVAILABLE,
                    "this containing type cannot store shared optional boxes yet",
                )
                .with_primary_label(
                    local.type_syntax.span,
                    "shared optional boxes are not array element types yet",
                ),
            );
            valid = false;
        }
    }
    valid
}

fn reject_if_box(
    program: &ResolvedProgram,
    ty: &ResolvedType,
    message: &'static str,
    diagnostics: &mut Diagnostics,
) -> bool {
    if !type_contains_box(program, ty.kind) {
        return true;
    }
    diagnostics.push(
        Diagnostic::error(SHARED_OPTIONAL_BOX_UNAVAILABLE, message).with_primary_label(
            ty.span,
            "this position remains outside the supported shared-box boundary",
        ),
    );
    false
}

fn is_local_box_owner(program: &ResolvedProgram, kind: ResolvedTypeKind) -> bool {
    match kind {
        ResolvedTypeKind::Shared(crate::resolve::ResolvedSharedTarget::OptionalBox(_)) => true,
        ResolvedTypeKind::Optional(optional) => program
            .optional_types
            .get(optional)
            .is_some_and(|optional| is_local_box_owner(program, optional.payload.kind)),
        _ => false,
    }
}

fn type_contains_box(program: &ResolvedProgram, kind: ResolvedTypeKind) -> bool {
    match kind {
        ResolvedTypeKind::Shared(crate::resolve::ResolvedSharedTarget::OptionalBox(_)) => true,
        ResolvedTypeKind::Optional(optional) => program
            .optional_types
            .get(optional)
            .is_some_and(|optional| type_contains_box(program, optional.payload.kind)),
        ResolvedTypeKind::Array(array) => program
            .array_types
            .get(array)
            .is_some_and(|array| type_contains_box(program, array.element.kind)),
        _ => false,
    }
}
