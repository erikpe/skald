//! Deliberate frontend availability boundaries for resolved language forms.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    resolve::ResolvedProgram,
};

pub const SHARED_OPTIONAL_BOX_UNAVAILABLE: &str = "TYP044";

/// Keeps resolved optional-box forms out of HIR until BX1 owns their complete
/// typing and lifecycle contract.
pub(super) fn validate(program: &ResolvedProgram, diagnostics: &mut Diagnostics) -> bool {
    let Some(target) = program.optional_box_types.iter().next() else {
        return true;
    };
    diagnostics.push(
        Diagnostic::error(
            SHARED_OPTIONAL_BOX_UNAVAILABLE,
            "shared optional boxes are not available during type checking yet",
        )
        .with_primary_label(
            target.span,
            "this form is parsed and resolved; typed support starts in roadmap task BX1",
        ),
    );
    false
}
