//! Prepared retention, stable summaries, and atomic changed results.

use crate::identity::CallableId;

use super::super::MirProgram;

/// Stable counts for each executable callable-definition kind.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MirDefinitionKindCounts {
    functions: usize,
    static_initializers: usize,
    initializers: usize,
    copy_constructors: usize,
    copy_assignments: usize,
    destructors: usize,
    methods: usize,
}

impl MirDefinitionKindCounts {
    pub(crate) const fn functions(self) -> usize {
        self.functions
    }

    pub(crate) const fn static_initializers(self) -> usize {
        self.static_initializers
    }

    pub(crate) const fn initializers(self) -> usize {
        self.initializers
    }

    pub(crate) const fn copy_constructors(self) -> usize {
        self.copy_constructors
    }

    pub(crate) const fn copy_assignments(self) -> usize {
        self.copy_assignments
    }

    pub(crate) const fn destructors(self) -> usize {
        self.destructors
    }

    pub(crate) const fn methods(self) -> usize {
        self.methods
    }

    pub(crate) const fn total(self) -> usize {
        self.functions
            + self.static_initializers
            + self.initializers
            + self.copy_constructors
            + self.copy_assignments
            + self.destructors
            + self.methods
    }

    pub(super) fn record(&mut self, callable: CallableId) {
        match callable {
            CallableId::Function(_) => self.functions += 1,
            CallableId::StaticInitializer(_) => self.static_initializers += 1,
            CallableId::Initializer(_) => self.initializers += 1,
            CallableId::CopyConstructor(_) => self.copy_constructors += 1,
            CallableId::CopyAssignment(_) => self.copy_assignments += 1,
            CallableId::Destructor(_) => self.destructors += 1,
            CallableId::Method(_) => self.methods += 1,
        }
    }
}

/// Deterministic accounting for one exact retention attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirDefinitionRetentionSummary {
    examined: MirDefinitionKindCounts,
    retained: MirDefinitionKindCounts,
    removed: MirDefinitionKindCounts,
    removed_callables: Vec<CallableId>,
}

impl MirDefinitionRetentionSummary {
    pub(super) fn new(
        examined: MirDefinitionKindCounts,
        retained: MirDefinitionKindCounts,
        removed: MirDefinitionKindCounts,
        mut removed_callables: Vec<CallableId>,
    ) -> Self {
        removed_callables.sort_unstable();
        debug_assert_eq!(examined.total(), retained.total() + removed.total());
        debug_assert!(removed_callables.windows(2).all(|pair| pair[0] < pair[1]));
        debug_assert_eq!(removed.total(), removed_callables.len());
        Self {
            examined,
            retained,
            removed,
            removed_callables,
        }
    }

    pub(crate) const fn examined(&self) -> MirDefinitionKindCounts {
        self.examined
    }

    pub(crate) const fn retained(&self) -> MirDefinitionKindCounts {
        self.retained
    }

    pub(crate) const fn removed(&self) -> MirDefinitionKindCounts {
        self.removed
    }

    pub(crate) fn removed_callables(&self) -> &[CallableId] {
        &self.removed_callables
    }
}

/// Prepared result that has not consumed or mutated the verified program.
pub(crate) enum MirDefinitionRetention {
    Unchanged(MirDefinitionRetentionSummary),
    Changed(MirPreparedDefinitionRetention),
}

/// Opaque validated authority to rebuild only executable definition tables.
pub(crate) struct MirPreparedDefinitionRetention {
    pub(super) summary: MirDefinitionRetentionSummary,
}

impl MirPreparedDefinitionRetention {
    pub(super) const fn new(summary: MirDefinitionRetentionSummary) -> Self {
        Self { summary }
    }
}

/// Complete changed raw MIR awaiting immediate central reverification.
pub(crate) struct MirDefinitionRetentionChange {
    pub(crate) program: MirProgram,
    pub(crate) summary: MirDefinitionRetentionSummary,
}
