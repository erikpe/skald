use super::identity::MirPassIdentity;

/// Supported target-independent final-MIR optimization policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MirOptimizationProfile {
    None,
    #[default]
    Default,
}

const NO_PASSES: &[MirPassIdentity] = &[];

impl MirOptimizationProfile {
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Default => "default",
        }
    }

    pub(super) const fn identities(self) -> &'static [MirPassIdentity] {
        match self {
            Self::None | Self::Default => NO_PASSES,
        }
    }
}
