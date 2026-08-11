//! Deliberate availability boundaries for staged shared optional boxes.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    resolve::{
        ResolvedFunctionLinkage, ResolvedMemberDefinition, ResolvedProgram, ResolvedType,
        ResolvedTypeKind,
    },
};

pub const SHARED_OPTIONAL_BOX_UNAVAILABLE: &str = "TYP044";

/// BX1 admits local box owners and their outer optional layers. Stored and
/// callable positions remain explicit gates until their roadmap owners add
/// lifetime and ABI support.
pub(super) fn validate(program: &ResolvedProgram, diagnostics: &mut Diagnostics) -> bool {
    let mut valid = true;

    for declaration in program.declarations.iter() {
        let external = matches!(
            declaration.linkage,
            ResolvedFunctionLinkage::External { .. }
        );
        for parameter in &declaration.parameters {
            valid &= reject_if_box(
                program,
                &parameter.type_syntax,
                if external {
                    "external signatures cannot contain shared optional boxes"
                } else {
                    "shared optional box parameters are enabled in roadmap task BX7"
                },
                diagnostics,
            );
        }
        valid &= reject_if_box(
            program,
            &declaration.return_type,
            if external {
                "external signatures cannot contain shared optional boxes"
            } else {
                "shared optional box results are enabled in roadmap task BX7"
            },
            diagnostics,
        );
    }

    for interface in program.interfaces.iter() {
        for requirement in &interface.requirements {
            for parameter in &requirement.parameters {
                valid &= reject_if_box(
                    program,
                    &parameter.type_syntax,
                    "shared optional box parameters are enabled in roadmap task BX7",
                    diagnostics,
                );
            }
            valid &= reject_if_box(
                program,
                &requirement.return_type,
                "shared optional box results are enabled in roadmap task BX7",
                diagnostics,
            );
        }
    }

    for class in program.classes.iter() {
        for field in &class.fields {
            valid &= reject_if_box(
                program,
                &field.type_syntax,
                "shared optional box fields are enabled in roadmap task BX7",
                diagnostics,
            );
        }
        for field in &class.static_fields {
            valid &= reject_if_box(
                program,
                &field.type_syntax,
                "shared optional box statics are enabled in roadmap task BX7",
                diagnostics,
            );
        }
        for initializer in &class.initializers {
            for parameter in &initializer.parameters {
                valid &= reject_if_box(
                    program,
                    &parameter.type_syntax,
                    "shared optional box parameters are enabled in roadmap task BX7",
                    diagnostics,
                );
            }
        }
        for method in &class.methods {
            for parameter in &method.parameters {
                valid &= reject_if_box(
                    program,
                    &parameter.type_syntax,
                    "shared optional box parameters are enabled in roadmap task BX7",
                    diagnostics,
                );
            }
            valid &= reject_if_box(
                program,
                &method.return_type,
                "shared optional box results are enabled in roadmap task BX7",
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
                    "BX1 supports direct local box owners and outer optional-owner layers",
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
            "this position remains deliberately gated after BX1",
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
