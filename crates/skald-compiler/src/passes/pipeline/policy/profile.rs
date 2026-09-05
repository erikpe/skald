use super::identity::MirPassIdentity;
use crate::passes::pipeline::optimizations::{
    checked_integer_folding, conservative_cfg_cleanup, constant_short_circuit_folding,
    dead_pure_definition_elimination, post_proof_basic_block_merging,
    post_proof_empty_block_forwarding, post_proof_unreachable_block_elimination,
    primitive_algebraic_simplification, primitive_constant_folding, whole_world_reachability,
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
    primitive_constant_folding::IDENTITY,
    primitive_algebraic_simplification::IDENTITY,
    primitive_constant_folding::IDENTITY,
    checked_integer_folding::IDENTITY,
    dead_pure_definition_elimination::IDENTITY,
    conservative_cfg_cleanup::IDENTITY,
    dead_pure_definition_elimination::IDENTITY,
    constant_short_circuit_folding::IDENTITY,
    post_proof_unreachable_block_elimination::IDENTITY,
    post_proof_empty_block_forwarding::IDENTITY,
    post_proof_basic_block_merging::IDENTITY,
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
