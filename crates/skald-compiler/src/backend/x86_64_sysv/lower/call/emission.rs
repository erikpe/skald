//! Audited construction of every target-level call instruction.

use super::super::super::machine::{Instruction, Register};

/// Describes why a generated call needs no trace update at this construction
/// point, or which owner already emitted the required update.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend::x86_64_sysv::lower) enum TraceAttribution {
    /// An eligible source frame records its current MIR operation before this
    /// call. The selecting caller owns emission of that replacement.
    SourceOperation,
    /// An omitted helper inherits the source operation recorded by the source
    /// frame that entered the generated helper chain.
    InheritedSourceOperation,
    /// An omitted helper enters a source-authored body, which pushes its own
    /// visible frame while retaining the inherited outer attribution.
    SourceBodyFromOmittedHelper,
    /// The callee returns status or otherwise cannot invoke the panic reporter.
    NonReporting,
    /// Valid calls cannot report; violated ABI preconditions hard-fail.
    HardDefectOnly,
    /// Process entry or automatic static coordination has no active initiating
    /// source operation and is an explicitly audited boundary exception.
    ProcessBoundary,
}

pub(in crate::backend::x86_64_sysv::lower) fn direct(
    symbol: impl Into<String>,
    attribution: TraceAttribution,
) -> Instruction {
    let _ = attribution;
    Instruction::Call(symbol.into())
}

pub(in crate::backend::x86_64_sysv::lower) fn indirect(
    register: Register,
    attribution: TraceAttribution,
) -> Instruction {
    let _ = attribution;
    Instruction::CallIndirect(register)
}

pub(in crate::backend::x86_64_sysv::lower) const fn is_call(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        Instruction::Call(_) | Instruction::CallIndirect(_)
    )
}
