//! Backend contract and target registry.
//!
//! Backends consume verified target-independent MIR and target options. They
//! must not inspect parser AST or type-checker state.
//! The repository contract is documented in `docs/compiler/BACKEND.md`.

use std::fmt;

use crate::{
    identity::CallableId, mir::MirProgram, passes::VerifiedFinalMirProgram, source::SourceDatabase,
};

mod x86_64_sysv;

// Keep this link guard synchronized with runtime/include/skald_runtime.h and
// docs/compiler/RUNTIME_ABI.md. Every incompatible runtime ABI revision must
// use a new symbol name.
pub(crate) const RUNTIME_ABI_MARKER_SYMBOL: &str = "ska_rt_abi_v9";
pub(crate) const RUNTIME_TRACE_TOP_SYMBOL: &str = "ska_rt_trace_top";

pub const DEFAULT_TARGET_NAME: &str = "x86_64-sysv";
pub const SUPPORTED_TARGET_NAMES: &[&str] = &[DEFAULT_TARGET_NAME];

/// Controls whether target emission may construct panic runtime-trace data.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimeTracePolicy {
    #[default]
    Enabled,
    Omitted,
}

/// Complete verified input needed by a target backend.
///
/// The constructors keep source access unavailable when tracing is omitted,
/// making trace-only source lookup impossible on that path. They accept only
/// the sealed product returned by the central final-MIR verifier.
///
/// ```compile_fail
/// use skald_compiler::{backend::BackendInput, mir::MirProgram};
/// let unchecked: MirProgram = todo!();
/// let _ = BackendInput::without_runtime_trace(&unchecked);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct BackendInput<'input> {
    verified: &'input VerifiedFinalMirProgram,
    sources: Option<&'input SourceDatabase>,
    runtime_trace: RuntimeTracePolicy,
    reachable_artifacts_only: bool,
}

impl<'input> BackendInput<'input> {
    pub const fn with_runtime_trace(
        verified: &'input VerifiedFinalMirProgram,
        sources: &'input SourceDatabase,
    ) -> Self {
        Self {
            verified,
            sources: Some(sources),
            runtime_trace: RuntimeTracePolicy::Enabled,
            reachable_artifacts_only: false,
        }
    }

    pub const fn without_runtime_trace(verified: &'input VerifiedFinalMirProgram) -> Self {
        Self {
            verified,
            sources: None,
            runtime_trace: RuntimeTracePolicy::Omitted,
            reachable_artifacts_only: false,
        }
    }

    /// Requests closed-world removal of target artifacts unreachable from an
    /// exported symbol. Complete emission remains the default so phase-owner
    /// diagnostics and tests can inspect lowered but uncalled MIR bodies.
    pub const fn with_reachable_artifacts_only(mut self) -> Self {
        self.reachable_artifacts_only = true;
        self
    }

    pub(crate) const fn program(self) -> &'input MirProgram {
        self.verified.program()
    }

    pub const fn sources(self) -> Option<&'input SourceDatabase> {
        self.sources
    }

    pub const fn runtime_trace(self) -> RuntimeTracePolicy {
        self.runtime_trace
    }

    pub(crate) const fn reachable_artifacts_only(self) -> bool {
        self.reachable_artifacts_only
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Target {
    #[default]
    X86_64SysV,
}

impl Target {
    pub const fn name(self) -> &'static str {
        match self {
            Self::X86_64SysV => DEFAULT_TARGET_NAME,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsupportedTargetError {
    requested: String,
}

impl UnsupportedTargetError {
    pub fn requested(&self) -> &str {
        &self.requested
    }
}

impl fmt::Display for UnsupportedTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unsupported target `{}`; supported targets: {}",
            self.requested,
            SUPPORTED_TARGET_NAMES.join(", ")
        )
    }
}

impl std::error::Error for UnsupportedTargetError {}

pub fn target_by_name(name: &str) -> Result<Target, UnsupportedTargetError> {
    match name {
        DEFAULT_TARGET_NAME => Ok(Target::X86_64SysV),
        _ => Err(UnsupportedTargetError {
            requested: name.to_owned(),
        }),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendError {
    target: Target,
    callable: Option<CallableId>,
    message: String,
}

impl BackendError {
    pub fn target(&self) -> Target {
        self.target
    }

    pub fn callable(&self) -> Option<CallableId> {
        self.callable
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn new(
        target: Target,
        callable: Option<CallableId>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            target,
            callable,
            message: message.into(),
        }
    }
}

impl fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.callable {
            Some(callable) => write!(
                formatter,
                "{} backend error in {callable}: {}",
                self.target.name(),
                self.message
            ),
            None => write!(
                formatter,
                "{} backend error: {}",
                self.target.name(),
                self.message
            ),
        }
    }
}

impl std::error::Error for BackendError {}

pub fn emit_assembly(target: Target, input: BackendInput<'_>) -> Result<String, BackendError> {
    match target {
        Target::X86_64SysV => x86_64_sysv::emit_assembly(input),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_accepts_only_the_initial_target() {
        assert_eq!(target_by_name("x86_64-sysv"), Ok(Target::X86_64SysV));

        let error = target_by_name("aarch64-linux").unwrap_err();
        assert_eq!(error.requested(), "aarch64-linux");
        assert_eq!(
            error.to_string(),
            "unsupported target `aarch64-linux`; supported targets: x86_64-sysv"
        );
    }

    #[test]
    fn backend_input_exposes_sources_only_for_enabled_tracing() {
        let program =
            crate::test_support::lower_source_to_final_mir("fn main() -> i64 { return 0; }");
        let verified = crate::passes::verify_final_mir(program).unwrap();
        let sources = SourceDatabase::new();
        let enabled = BackendInput::with_runtime_trace(&verified, &sources);
        let omitted = BackendInput::without_runtime_trace(&verified);

        assert_eq!(enabled.runtime_trace(), RuntimeTracePolicy::Enabled);
        assert!(enabled.sources().is_some());
        assert!(!enabled.reachable_artifacts_only());
        assert_eq!(omitted.runtime_trace(), RuntimeTracePolicy::Omitted);
        assert!(omitted.sources().is_none());
        assert!(omitted
            .with_reachable_artifacts_only()
            .reachable_artifacts_only());
    }
}
