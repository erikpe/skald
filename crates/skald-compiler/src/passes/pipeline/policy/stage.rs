use std::fmt;

/// Verified MIR contract consumed by one selectable pass.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MirPassStage {
    /// Runs while path and logical proof provenance is still available.
    ProofRich,
    /// Consumes proof provenance while producing the normalized final seal.
    ProofTransition,
    /// Runs after mandatory proof-provenance normalization.
    Final,
}

impl MirPassStage {
    pub const fn name(self) -> &'static str {
        match self {
            Self::ProofRich => "proof-rich",
            Self::ProofTransition => "proof-transition",
            Self::Final => "final",
        }
    }
}

impl fmt::Display for MirPassStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}
