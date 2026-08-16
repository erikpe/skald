//! Complete-compilation feature gates over already verified MIR.
//!
//! Lowering retains declaration metadata even when the backend-facing write
//! semantics are intentionally deferred. Keeping this boundary after the MIR
//! pipeline makes ignored finality impossible to execute.

mod final_assignment;

use crate::diagnostics::{Diagnostic, Diagnostics};

use super::MirProgram;

pub(crate) const FINAL_FIELD_EXECUTION_UNAVAILABLE: &str = "MIR002";
pub(crate) const FINAL_ASSIGNMENT_EXECUTION_UNAVAILABLE: &str = "MIR003";

pub(crate) fn validate_final_field_execution_support(program: &MirProgram) -> Diagnostics {
    let mut diagnostics = Diagnostics::new();
    for class in program.classes.iter() {
        for field in &class.static_fields {
            if let Some(span) = field.final_span {
                diagnostics.push(final_static_diagnostic(span));
            }
        }
    }
    for span in final_assignment::unsupported_assignment_spans(program) {
        diagnostics.push(final_assignment_diagnostic(span));
    }
    diagnostics
}

fn final_static_diagnostic(span: crate::source::Span) -> Diagnostic {
    Diagnostic::error(
        FINAL_FIELD_EXECUTION_UNAVAILABLE,
        "final static fields cannot be emitted yet",
    )
    .with_primary_label(
        span,
        "final-static publication and replacement semantics are not implemented",
    )
    .with_note("final instance construction and reads are supported independently")
}

fn final_assignment_diagnostic(span: crate::source::Span) -> Diagnostic {
    Diagnostic::error(
        FINAL_ASSIGNMENT_EXECUTION_UNAVAILABLE,
        "assignment of a value containing final fields cannot be emitted yet",
    )
    .with_primary_label(
        span,
        "complete-value final-field update evidence is not implemented",
    )
    .with_note("construction, copy construction, reads, and destruction remain supported")
}
