//! Complete-compilation feature gates over already verified MIR.
//!
//! Lowering retains declaration metadata even when the backend-facing write
//! semantics are intentionally deferred. Keeping this boundary after the MIR
//! pipeline makes ignored finality impossible to execute.

use crate::diagnostics::{Diagnostic, Diagnostics};

use super::MirProgram;

pub(crate) const FINAL_FIELD_EXECUTION_UNAVAILABLE: &str = "MIR002";

pub(crate) fn validate_final_field_execution_support(program: &MirProgram) -> Diagnostics {
    let mut diagnostics = Diagnostics::new();
    for class in program.classes.iter() {
        for field in &class.fields {
            if let Some(span) = field.final_span {
                diagnostics.push(final_field_diagnostic(span));
            }
        }
        for field in &class.static_fields {
            if let Some(span) = field.final_span {
                diagnostics.push(final_field_diagnostic(span));
            }
        }
    }
    diagnostics
}

fn final_field_diagnostic(span: crate::source::Span) -> Diagnostic {
    Diagnostic::error(
        FINAL_FIELD_EXECUTION_UNAVAILABLE,
        "final fields cannot be emitted yet",
    )
    .with_primary_label(
        span,
        "final-field initialization and replacement semantics are not implemented",
    )
    .with_note("declaration metadata is preserved through verified MIR")
}
