//! Eligibility validation for recursive resolved optionals.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    resolve::{ResolvedProgram, ResolvedTypeKind},
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
    valid
}
