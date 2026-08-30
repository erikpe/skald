use super::identity::MirPassIdentity;

/// Supported target-independent final-MIR optimization policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum MirOptimizationProfile {
    None,
    #[default]
    Default,
}

const NO_PASSES: &[MirPassIdentity] = &[];

impl MirOptimizationProfile {
    pub(super) const fn identities(self) -> &'static [MirPassIdentity] {
        match self {
            Self::None | Self::Default => NO_PASSES,
        }
    }
}
