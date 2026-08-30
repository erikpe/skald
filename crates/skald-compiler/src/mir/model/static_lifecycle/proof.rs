//! Compact, immutable authority carried across the final-MIR boundary.

use std::collections::{BTreeMap, BTreeSet};

use crate::identity::{ArrayTypeId, CallableId, ClassId, StaticFieldId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StaticClassLifecycleOperation {
    CopyConstructor,
    CopyAssignment,
    CompleteFinalizer,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StaticArrayLifecycleOperation {
    Default,
    Copy,
    Assignment,
    Destruction,
}

/// Stable identity of a callable or implicit lifecycle root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StaticEffectNode {
    Callable(CallableId),
    ClassLifecycle {
        class: ClassId,
        operation: StaticClassLifecycleOperation,
    },
    ArrayLifecycle {
        array: ArrayTypeId,
        operation: StaticArrayLifecycleOperation,
    },
}

impl StaticEffectNode {
    pub const fn callable(callable: CallableId) -> Self {
        Self::Callable(callable)
    }

    pub const fn class(class: ClassId, operation: StaticClassLifecycleOperation) -> Self {
        Self::ClassLifecycle { class, operation }
    }

    pub const fn array(array: ArrayTypeId, operation: StaticArrayLifecycleOperation) -> Self {
        Self::ArrayLifecycle { array, operation }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StaticAccessKind {
    Read,
    Write,
    Borrow,
    Initialize,
    Replace,
    Destroy,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StaticEffectPhase {
    Ordinary,
    InitializerBeforePublication,
    InitializerAfterPublication,
    Copy,
    Destruction,
    ArrayLifecycle,
}

/// One semantic static effect authorized for a lifecycle root.
///
/// Source locations, witness paths, directness, edge kinds, and intermediate
/// nodes are intentionally excluded because they may change without changing
/// static-lifecycle safety.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StaticLifecycleEffectFact {
    target: StaticFieldId,
    access: StaticAccessKind,
    phase: StaticEffectPhase,
    lifecycle_owned: bool,
}

impl StaticLifecycleEffectFact {
    pub(crate) const fn new(
        target: StaticFieldId,
        access: StaticAccessKind,
        phase: StaticEffectPhase,
        lifecycle_owned: bool,
    ) -> Self {
        Self {
            target,
            access,
            phase,
            lifecycle_owned,
        }
    }

    pub const fn target(&self) -> StaticFieldId {
        self.target
    }

    pub const fn access(&self) -> StaticAccessKind {
        self.access
    }

    pub const fn phase(&self) -> StaticEffectPhase {
        self.phase
    }

    pub const fn is_lifecycle_owned(&self) -> bool {
        self.lifecycle_owned
    }

    #[cfg(test)]
    pub(crate) fn set_target_for_test(&mut self, target: StaticFieldId) {
        self.target = target;
    }

    #[cfg(test)]
    pub(crate) fn set_access_for_test(&mut self, access: StaticAccessKind) {
        self.access = access;
    }

    #[cfg(test)]
    pub(crate) fn set_phase_for_test(&mut self, phase: StaticEffectPhase) {
        self.phase = phase;
    }

    #[cfg(test)]
    pub(crate) fn set_lifecycle_owned_for_test(&mut self, lifecycle_owned: bool) {
        self.lifecycle_owned = lifecycle_owned;
    }
}

/// The exact normalized effects authorized for one lifecycle root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticLifecycleRootAuthority {
    root: StaticEffectNode,
    effects: Vec<StaticLifecycleEffectFact>,
}

impl StaticLifecycleRootAuthority {
    pub(crate) fn new(root: StaticEffectNode, mut effects: Vec<StaticLifecycleEffectFact>) -> Self {
        effects.sort_unstable();
        effects.dedup();
        Self { root, effects }
    }

    pub const fn root(&self) -> StaticEffectNode {
        self.root
    }

    pub fn effects(&self) -> &[StaticLifecycleEffectFact] {
        &self.effects
    }

    #[cfg(test)]
    pub(crate) fn set_root_for_test(&mut self, root: StaticEffectNode) {
        self.root = root;
    }

    #[cfg(test)]
    pub(crate) fn effects_mut_for_test(&mut self) -> &mut Vec<StaticLifecycleEffectFact> {
        &mut self.effects
    }
}

/// Immutable baseline authority issued from verified preliminary MIR.
///
/// Roots and their fact sets are stored in deterministic sorted, unique order.
/// Public consumers can inspect the authority but cannot construct or mutate it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticLifecycleAuthority {
    roots: Vec<StaticLifecycleRootAuthority>,
}

impl StaticLifecycleAuthority {
    pub(crate) fn new(roots: Vec<StaticLifecycleRootAuthority>) -> Self {
        let mut by_root = BTreeMap::<StaticEffectNode, BTreeSet<StaticLifecycleEffectFact>>::new();
        for root in roots {
            by_root.entry(root.root).or_default().extend(root.effects);
        }
        Self {
            roots: by_root
                .into_iter()
                .map(|(root, effects)| {
                    StaticLifecycleRootAuthority::new(root, effects.into_iter().collect())
                })
                .collect(),
        }
    }

    pub fn roots(&self) -> impl ExactSizeIterator<Item = &StaticLifecycleRootAuthority> {
        self.roots.iter()
    }

    pub fn root(&self, root: StaticEffectNode) -> Option<&StaticLifecycleRootAuthority> {
        self.roots
            .binary_search_by_key(&root, StaticLifecycleRootAuthority::root)
            .ok()
            .map(|index| &self.roots[index])
    }

    #[cfg(test)]
    pub(crate) fn roots_mut_for_test(&mut self) -> &mut Vec<StaticLifecycleRootAuthority> {
        &mut self.roots
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStaticLifecycleProof {
    authority: StaticLifecycleAuthority,
}

impl MirStaticLifecycleProof {
    pub(crate) const fn new(authority: StaticLifecycleAuthority) -> Self {
        Self { authority }
    }

    pub fn authority(&self) -> &StaticLifecycleAuthority {
        &self.authority
    }

    #[cfg(test)]
    pub(crate) fn authority_mut_for_test(&mut self) -> &mut StaticLifecycleAuthority {
        &mut self.authority
    }
}
