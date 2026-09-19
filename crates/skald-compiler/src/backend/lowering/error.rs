use crate::{
    backend::{
        lir::{BuildError, ProgramError, VerificationFailure},
        plan::PlanError,
    },
    identity::CallableId,
};

#[derive(Debug)]
pub(in crate::backend) enum LowerError {
    MissingBody(CallableId),
    Plan(PlanError),
    Build(BuildError),
    Program(ProgramError),
    Verification(Vec<VerificationFailure>),
}
impl From<BuildError> for LowerError {
    fn from(error: BuildError) -> Self {
        Self::Build(error)
    }
}
impl From<ProgramError> for LowerError {
    fn from(error: ProgramError) -> Self {
        Self::Program(error)
    }
}
impl From<PlanError> for LowerError {
    fn from(error: PlanError) -> Self {
        Self::Plan(error)
    }
}
impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingBody(callable) => write!(
                f,
                "low-level lowering requires a retained body for {callable}"
            ),
            Self::Plan(error) => write!(f, "lower planning invariant: {error:?}"),
            Self::Build(error) => write!(f, "lower construction invariant: {error:?}"),
            Self::Program(error) => write!(f, "lower inventory invariant: {error:?}"),
            Self::Verification(errors) => write!(f, "lower publication failed: {errors:?}"),
        }
    }
}
impl std::error::Error for LowerError {}
