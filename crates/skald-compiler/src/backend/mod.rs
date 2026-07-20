//! Backend contract and target registry.
//!
//! Backends consume verified target-independent MIR and target options. They
//! must not inspect parser AST or type-checker state.

use std::fmt;

use crate::{identity::FunctionId, mir::MirProgram};

mod x86_64_sysv;

pub const DEFAULT_TARGET_NAME: &str = "x86_64-sysv";
pub const SUPPORTED_TARGET_NAMES: &[&str] = &[DEFAULT_TARGET_NAME];

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
    function: Option<FunctionId>,
    message: String,
}

impl BackendError {
    pub fn target(&self) -> Target {
        self.target
    }

    pub fn function(&self) -> Option<FunctionId> {
        self.function
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn new(
        target: Target,
        function: Option<FunctionId>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            target,
            function,
            message: message.into(),
        }
    }
}

impl fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.function {
            Some(function) => write!(
                formatter,
                "{} backend error in {function}: {}",
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

pub fn emit_assembly(target: Target, program: &MirProgram) -> Result<String, BackendError> {
    match target {
        Target::X86_64SysV => x86_64_sysv::emit_assembly(program),
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
}
