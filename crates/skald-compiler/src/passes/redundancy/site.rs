//! Shared owned locations retained for deterministic census examples.

use crate::{
    identity::CallableId,
    mir::{BlockId, ValueId},
};

/// Maximum examples retained for each classification in one observation.
pub const REDUNDANCY_SITE_EXAMPLES_PER_CLASSIFICATION: usize = 8;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RedundancySiteClassification {
    Proven,
    Blocked,
}

/// An owned, revision-local example of one interesting census site.
///
/// Dense MIR identities are suitable for auditing one compiler result, but
/// callers must not compare them across unrelated rewrites or revisions.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RedundancySiteExample<R> {
    callable: CallableId,
    block: BlockId,
    instruction: usize,
    value: Option<ValueId>,
    classification: RedundancySiteClassification,
    reasons: Vec<R>,
}

impl<R> RedundancySiteExample<R> {
    pub const fn callable(&self) -> CallableId {
        self.callable
    }

    pub const fn block(&self) -> BlockId {
        self.block
    }

    pub const fn instruction(&self) -> usize {
        self.instruction
    }

    pub const fn value(&self) -> Option<ValueId> {
        self.value
    }

    pub const fn classification(&self) -> RedundancySiteClassification {
        self.classification
    }

    pub fn reasons(&self) -> &[R] {
        &self.reasons
    }

    pub(crate) fn new(
        callable: CallableId,
        block: BlockId,
        instruction: usize,
        value: Option<ValueId>,
        classification: RedundancySiteClassification,
        reasons: Vec<R>,
    ) -> Self {
        Self {
            callable,
            block,
            instruction,
            value,
            classification,
            reasons,
        }
    }
}

pub(super) fn merge_examples<R: Clone + Ord>(
    target: &mut Vec<RedundancySiteExample<R>>,
    source: &[RedundancySiteExample<R>],
) {
    target.extend_from_slice(source);
    target.sort();
    target.dedup();

    let mut proven = 0;
    let mut blocked = 0;
    target.retain(|example| {
        let count = match example.classification {
            RedundancySiteClassification::Proven => &mut proven,
            RedundancySiteClassification::Blocked => &mut blocked,
        };
        *count += 1;
        *count <= REDUNDANCY_SITE_EXAMPLES_PER_CLASSIFICATION
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::FunctionId;

    #[test]
    fn examples_are_sorted_deduplicated_and_bounded_per_classification() {
        let callable = CallableId::Function(FunctionId::new(0));
        let block = BlockId::new(callable, 0);
        let mut examples = Vec::new();
        for instruction in (0..12).rev() {
            for classification in [
                RedundancySiteClassification::Blocked,
                RedundancySiteClassification::Proven,
            ] {
                let reasons = match classification {
                    RedundancySiteClassification::Proven => Vec::new(),
                    RedundancySiteClassification::Blocked => vec![1_u8],
                };
                let example = RedundancySiteExample::new(
                    callable,
                    block,
                    instruction,
                    None,
                    classification,
                    reasons,
                );
                merge_examples(&mut examples, &[example.clone(), example]);
            }
        }

        assert_eq!(examples.len(), 16);
        assert!(examples.windows(2).all(|pair| pair[0] < pair[1]));
        for classification in [
            RedundancySiteClassification::Proven,
            RedundancySiteClassification::Blocked,
        ] {
            assert_eq!(
                examples
                    .iter()
                    .filter(|example| example.classification() == classification)
                    .count(),
                REDUNDANCY_SITE_EXAMPLES_PER_CLASSIFICATION
            );
        }
    }
}
