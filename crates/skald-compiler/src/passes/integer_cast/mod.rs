//! Exact composition and canonical recipes for total MIR integer casts.
//!
//! This is the shared semantic authority for read-only redundancy analysis and
//! integer cast-chain optimization. It deliberately models language-level bits
//! rather than target registers or host integer conversions.

mod chain;

pub(in crate::passes) use chain::{
    analyze_integer_cast_chains, IntegerCastChain, IntegerCastChainBoundary, IntegerCastSite,
    IntegerCastSiteInvalidity,
};

use crate::mir::{MirIntegerType, MirPrimitiveCast};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RetainedBits {
    All,
    LowEight,
}

/// The complete effect of an integer-only cast sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::passes) struct IntegerCastTransform {
    source: MirIntegerType,
    target: MirIntegerType,
    retained: RetainedBits,
}

impl IntegerCastTransform {
    /// Starts the empty cast sequence for `source`.
    pub(in crate::passes) const fn identity(source: MirIntegerType) -> Self {
        Self {
            source,
            target: source,
            retained: RetainedBits::All,
        }
    }

    /// Starts a sequence from one semantically valid ordinary integer cast.
    pub(in crate::passes) fn from_operation(operation: MirPrimitiveCast) -> Option<Self> {
        let source = operation.source.integer_type()?;
        Self::identity(source).then(operation)
    }

    /// Appends one semantically valid, type-connected ordinary integer cast.
    pub(in crate::passes) fn then(self, operation: MirPrimitiveCast) -> Option<Self> {
        let operation_source = operation.source.integer_type()?;
        let operation_target = operation.target.integer_type()?;
        if !operation.is_semantically_consistent() || operation_source != self.target {
            return None;
        }

        let retained = if self.retained == RetainedBits::LowEight
            || (self.source != MirIntegerType::U8 && operation_target == MirIntegerType::U8)
        {
            RetainedBits::LowEight
        } else {
            RetainedBits::All
        };
        Some(Self {
            source: self.source,
            target: operation_target,
            retained,
        })
    }

    /// Selects the unique shortest sequence of ordinary integer casts with
    /// this transform's complete-domain semantics.
    pub(in crate::passes) fn canonical_recipe(self) -> IntegerCastRecipe {
        match (self.retained, self.source == self.target, self.target) {
            (RetainedBits::All, true, _) => IntegerCastRecipe::Identity,
            (RetainedBits::All, false, _) | (RetainedBits::LowEight, _, MirIntegerType::U8) => {
                IntegerCastRecipe::Direct(MirPrimitiveCast::new(
                    self.source.into(),
                    self.target.into(),
                ))
            }
            (RetainedBits::LowEight, _, MirIntegerType::I64 | MirIntegerType::U64) => {
                debug_assert_ne!(self.source, MirIntegerType::U8);
                IntegerCastRecipe::NarrowThenWiden {
                    narrow: MirPrimitiveCast::new(self.source.into(), MirIntegerType::U8.into()),
                    widen: MirPrimitiveCast::new(MirIntegerType::U8.into(), self.target.into()),
                }
            }
        }
    }
}

/// The shortest ordinary integer-cast sequence for one exact transform.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::passes) enum IntegerCastRecipe {
    Identity,
    Direct(MirPrimitiveCast),
    NarrowThenWiden {
        narrow: MirPrimitiveCast,
        widen: MirPrimitiveCast,
    },
}

impl IntegerCastRecipe {
    pub(in crate::passes) const fn length(self) -> usize {
        match self {
            Self::Identity => 0,
            Self::Direct(_) => 1,
            Self::NarrowThenWiden { .. } => 2,
        }
    }
}

#[cfg(test)]
mod tests;
