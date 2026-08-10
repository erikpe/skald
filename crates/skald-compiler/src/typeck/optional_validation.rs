//! Eligibility and staged-position gates for recursive resolved optionals.

use std::collections::HashSet;

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    identity::{ArrayTypeId, OptionalTypeId},
    resolve::{ResolvedProgram, ResolvedType, ResolvedTypeKind},
};

pub const INVALID_OPTIONAL_TYPE: &str = "TYP043";

pub(super) fn validate_optional_types(
    program: &ResolvedProgram,
    diagnostics: &mut Diagnostics,
) -> bool {
    let mut valid = true;
    for optional in program.optional_types.iter() {
        let (message, label) = match optional.payload.kind {
            ResolvedTypeKind::I64
            | ResolvedTypeKind::U64
            | ResolvedTypeKind::U8
            | ResolvedTypeKind::F64
            | ResolvedTypeKind::Bool
            | ResolvedTypeKind::Class(_)
            | ResolvedTypeKind::Shared(_)
            | ResolvedTypeKind::Optional(_)
            | ResolvedTypeKind::Array(_) => continue,
            ResolvedTypeKind::Interface(_) => (
                "interfaces cannot be inline optional payloads",
                "use an optional shared owner for an optional owning interface view",
            ),
            ResolvedTypeKind::Obj => (
                "`Obj?` is not a valid inline optional type",
                "use `(shared Obj)?` for an optional owning object view",
            ),
            ResolvedTypeKind::Unit => (
                "`unit?` is not a valid optional type",
                "`unit` has no value payload to make optional",
            ),
        };
        diagnostics.push(
            Diagnostic::error(INVALID_OPTIONAL_TYPE, message)
                .with_primary_label(optional.payload.span, label),
        );
        valid = false;
    }
    valid &= validate_optional_array_positions(program, diagnostics);
    valid
}

fn validate_optional_array_positions(
    program: &ResolvedProgram,
    diagnostics: &mut Diagnostics,
) -> bool {
    let mut valid = true;
    for function in program.declarations.iter() {
        for parameter in &function.parameters {
            if !matches!(
                parameter.binding_mode,
                crate::resolve::ResolvedParameterBindingMode::Value
            ) {
                valid &= reject_optional_array(
                    program,
                    &parameter.type_syntax,
                    "optional arrays are not supported in alias parameters yet",
                    diagnostics,
                );
            }
        }
    }
    for array in program.array_types.iter() {
        valid &= reject_optional_array(
            program,
            &array.element,
            "optional arrays are not supported as array elements yet",
            diagnostics,
        );
    }
    for class in program.classes.iter() {
        for field in &class.fields {
            valid &= reject_optional_array(
                program,
                &field.type_syntax,
                "optional arrays are not supported in class fields yet",
                diagnostics,
            );
        }
        for field in &class.static_fields {
            valid &= reject_optional_array(
                program,
                &field.type_syntax,
                "optional arrays are not supported in static fields yet",
                diagnostics,
            );
        }
        for initializer in &class.initializers {
            for parameter in &initializer.parameters {
                valid &= reject_optional_array(
                    program,
                    &parameter.type_syntax,
                    "optional arrays are not supported in initializer parameters yet",
                    diagnostics,
                );
            }
        }
        for method in &class.methods {
            for parameter in &method.parameters {
                valid &= reject_optional_array(
                    program,
                    &parameter.type_syntax,
                    "optional arrays are not supported in method parameters yet",
                    diagnostics,
                );
            }
            valid &= reject_optional_array(
                program,
                &method.return_type,
                "optional arrays are not supported as method results yet",
                diagnostics,
            );
        }
    }
    for interface in program.interfaces.iter() {
        for requirement in &interface.requirements {
            for parameter in &requirement.parameters {
                valid &= reject_optional_array(
                    program,
                    &parameter.type_syntax,
                    "optional arrays are not supported in interface parameters yet",
                    diagnostics,
                );
            }
            valid &= reject_optional_array(
                program,
                &requirement.return_type,
                "optional arrays are not supported as interface results yet",
                diagnostics,
            );
        }
    }
    valid
}

fn reject_optional_array(
    program: &ResolvedProgram,
    ty: &ResolvedType,
    message: &'static str,
    diagnostics: &mut Diagnostics,
) -> bool {
    if !contains_optional_array(program, ty.kind) {
        return true;
    }
    diagnostics.push(
        Diagnostic::error(INVALID_OPTIONAL_TYPE, message).with_primary_label(
            ty.span,
            "this position is part of the later aggregate integration",
        ),
    );
    false
}

fn contains_optional_array(program: &ResolvedProgram, root: ResolvedTypeKind) -> bool {
    let mut pending = vec![root];
    let mut arrays = HashSet::<ArrayTypeId>::new();
    let mut optionals = HashSet::<OptionalTypeId>::new();
    while let Some(ty) = pending.pop() {
        match ty {
            ResolvedTypeKind::Optional(optional) if optionals.insert(optional) => {
                let payload = program
                    .optional_types
                    .get(optional)
                    .expect("resolved optional identity must exist")
                    .payload
                    .kind;
                if matches!(payload, ResolvedTypeKind::Array(_)) {
                    return true;
                }
                pending.push(payload);
            }
            ResolvedTypeKind::Array(array) if arrays.insert(array) => {
                pending.push(
                    program
                        .array_types
                        .get(array)
                        .expect("resolved array identity must exist")
                        .element
                        .kind,
                );
            }
            _ => {}
        }
    }
    false
}
