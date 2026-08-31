use super::identity::MirPassIdentity;
use crate::passes::pipeline::optimizations::{
    dead_pure_definition_elimination, whole_world_reachability,
};

/// Supported target-independent final-MIR optimization policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MirOptimizationProfile {
    None,
    #[default]
    Default,
}

const NO_PASSES: &[MirPassIdentity] = &[];
const DEFAULT_PASSES: &[MirPassIdentity] = &[
    dead_pure_definition_elimination::IDENTITY,
    whole_world_reachability::IDENTITY,
];

impl MirOptimizationProfile {
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Default => "default",
        }
    }

    pub(super) const fn identities(self) -> &'static [MirPassIdentity] {
        match self {
            Self::None => NO_PASSES,
            Self::Default => DEFAULT_PASSES,
        }
    }
}
