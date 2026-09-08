//! Shared owned locations retained for deterministic census examples.

use crate::{
    identity::CallableId,
    mir::{BlockId, StorageId, ValueId},
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

/// An owned example centered on one storage declaration.
///
/// Some storage candidates have no executable references, so their audit
/// location is optional rather than being replaced with a fabricated block or
/// instruction position.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RedundancyStorageExample<R> {
    callable: CallableId,
    storage: StorageId,
    block: Option<BlockId>,
    instruction: Option<usize>,
    classification: RedundancySiteClassification,
    reasons: Vec<R>,
}

impl<R> RedundancyStorageExample<R> {
    pub const fn callable(&self) -> CallableId {
        self.callable
    }

    pub const fn storage(&self) -> StorageId {
        self.storage
    }

    pub const fn block(&self) -> Option<BlockId> {
        self.block
    }

    pub const fn instruction(&self) -> Option<usize> {
        self.instruction
    }

    pub const fn classification(&self) -> RedundancySiteClassification {
        self.classification
    }

    pub fn reasons(&self) -> &[R] {
        &self.reasons
    }

    pub(crate) fn new(
        callable: CallableId,
        storage: StorageId,
        location: Option<(BlockId, usize)>,
        classification: RedundancySiteClassification,
        reasons: Vec<R>,
    ) -> Self {
        let (block, instruction) = location
            .map(|(block, instruction)| (Some(block), Some(instruction)))
            .unwrap_or((None, None));
        Self {
            callable,
            storage,
            block,
            instruction,
            classification,
            reasons,
        }
    }
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

pub(super) fn merge_storage_examples<R: Clone + Ord>(
    target: &mut Vec<RedundancyStorageExample<R>>,
    source: &[RedundancyStorageExample<R>],
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

    #[test]
    fn storage_examples_are_bounded_without_fake_declaration_locations() {
        let callable = CallableId::Function(FunctionId::new(0));
        let mut examples = Vec::new();
        for index in (0..12).rev() {
            for classification in [
                RedundancySiteClassification::Blocked,
                RedundancySiteClassification::Proven,
            ] {
                let example = RedundancyStorageExample::<u8>::new(
                    callable,
                    StorageId::new(callable, index),
                    None,
                    classification,
                    Vec::new(),
                );
                merge_storage_examples(&mut examples, &[example.clone(), example]);
            }
        }

        assert_eq!(examples.len(), 16);
        assert!(examples.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(examples
            .iter()
            .all(|example| example.block().is_none() && example.instruction().is_none()));
        for classification in [
            RedundancySiteClassification::Blocked,
            RedundancySiteClassification::Proven,
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
