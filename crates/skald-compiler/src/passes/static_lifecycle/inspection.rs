//! Borrowed inspection of the verified static-activation planning decision.

use std::{fmt, fmt::Write};

use super::{
    activation::dump_static_activation, plan::PlannedMirProgram, VerifiedPlannedMirProgram,
};

/// Stable identity of the request-local activation inspection checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticActivationInspectionLabel {
    /// Lifecycle planning has passed independent planned-MIR verification.
    VerifiedPlanning,
}

impl fmt::Display for StaticActivationInspectionLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VerifiedPlanning => formatter.write_str("verified-static-activation"),
        }
    }
}

/// One immutable, verified static-activation checkpoint.
///
/// The planning product is borrowed only for the callback. Rendering remains
/// opt-in through [`activation_dump`](Self::activation_dump), so an inspector
/// that needs only typed facts or the label performs no formatting work.
#[derive(Clone, Copy, Debug)]
pub struct StaticActivationInspection<'a> {
    verified: &'a VerifiedPlannedMirProgram,
}

impl<'a> StaticActivationInspection<'a> {
    pub const fn label(self) -> StaticActivationInspectionLabel {
        StaticActivationInspectionLabel::VerifiedPlanning
    }

    pub const fn verified(self) -> &'a VerifiedPlannedMirProgram {
        self.verified
    }

    pub fn planned(self) -> &'a PlannedMirProgram {
        self.verified.program()
    }

    /// Renders the exact activation graph, canonical witnesses, and resulting
    /// lifecycle order for focused compiler tools and tests.
    pub fn activation_dump(self) -> String {
        let planned = self.planned();
        let mut output = dump_static_activation(
            planned.preliminary(),
            planned.planning_report().activation(),
        );
        output.push_str("  ActivationOrder\n");
        for field in planned.lifecycle().activation() {
            write_ordered_field(&mut output, planned, *field);
        }
        output.push_str("  ShutdownOrder\n");
        for field in planned.lifecycle().shutdown() {
            write_ordered_field(&mut output, planned, field);
        }
        output
    }

    pub(crate) const fn new(verified: &'a VerifiedPlannedMirProgram) -> Self {
        Self { verified }
    }
}

fn write_ordered_field(
    output: &mut String,
    planned: &PlannedMirProgram,
    field: crate::identity::StaticFieldId,
) {
    let _ = write!(output, "    Field {field}");
    if let Some(name) = planned.preliminary().static_field_qualified_name(field) {
        let _ = write!(output, " ({name})");
    }
    output.push('\n');
}

/// Request-local consumer of one borrowed verified activation checkpoint.
///
/// Implementations receive no mutation, reporting, target, request, or
/// filesystem capability from the compiler.
pub trait StaticActivationInspector {
    fn inspect(&mut self, inspection: StaticActivationInspection<'_>);
}

impl<F> StaticActivationInspector for F
where
    F: for<'a> FnMut(StaticActivationInspection<'a>),
{
    fn inspect(&mut self, inspection: StaticActivationInspection<'_>) {
        self(inspection);
    }
}

#[cfg(test)]
mod tests;
