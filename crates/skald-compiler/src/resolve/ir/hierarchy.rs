//! Canonical, identity-based class ancestry and ordinary-member lookup.

use std::collections::BTreeMap;

use crate::{
    id_table::DenseIdTable,
    identity::{ClassId, FieldId, MethodId, StaticFieldId},
};

/// A selected ordinary class member together with its declaring-class identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResolvedClassMember {
    Field(FieldId),
    StaticField(StaticFieldId),
    Method(MethodId),
}

impl ResolvedClassMember {
    pub const fn declaring_class(self) -> ClassId {
        match self {
            Self::Field(field) => field.class(),
            Self::StaticField(field) => field.class(),
            Self::Method(method) => method.class(),
        }
    }
}

/// The validated class graph used by later target-independent phases.
///
/// Base-chain iterators run from the direct base toward the root. Queries
/// return `None` when the supplied identity is absent or its ancestry depends
/// on an invalid cycle. Chains are traversed from direct edges instead of
/// being copied into every entry.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedClassHierarchy {
    entries: DenseIdTable<ClassId, ResolvedClassHierarchyEntry>,
}

impl ResolvedClassHierarchy {
    pub(crate) fn new(entries: Vec<ResolvedClassHierarchyEntry>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.class),
        }
    }

    pub fn direct_base(&self, class: ClassId) -> Option<ClassId> {
        self.entry(class)?.direct_base
    }

    pub fn base_chain(&self, class: ClassId) -> Option<impl Iterator<Item = ClassId> + '_> {
        let entry = self.entry(class)?;
        entry.ancestry_valid.then_some(BaseChain {
            hierarchy: self,
            next: entry.direct_base,
            remaining: self.entries.len(),
        })
    }

    pub fn is_subtype(&self, class: ClassId, target: ClassId) -> Option<bool> {
        if !self.entry(target)?.ancestry_valid {
            return None;
        }
        Some(class == target || self.base_chain(class)?.any(|base| base == target))
    }

    pub fn member(&self, class: ClassId, name: &str) -> Option<ResolvedClassMember> {
        let entry = self.entry(class)?;
        if !entry.ancestry_valid {
            return None;
        }
        entry
            .members
            .get(name)
            .copied()
            .or_else(|| self.member_in_chain(self.base_chain(class)?, name))
    }

    pub fn inherited_member(&self, class: ClassId, name: &str) -> Option<ResolvedClassMember> {
        let chain = self.base_chain(class)?;
        self.member_in_chain(chain, name)
    }

    fn member_in_chain(
        &self,
        mut chain: impl Iterator<Item = ClassId>,
        name: &str,
    ) -> Option<ResolvedClassMember> {
        chain.find_map(|class| {
            self.entry(class)
                .and_then(|entry| entry.members.get(name))
                .copied()
        })
    }

    fn entry(&self, class: ClassId) -> Option<&ResolvedClassHierarchyEntry> {
        self.entries.get(class, |entry| entry.class)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedClassHierarchyEntry {
    pub(crate) class: ClassId,
    pub(crate) direct_base: Option<ClassId>,
    pub(crate) ancestry_valid: bool,
    pub(crate) members: BTreeMap<String, ResolvedClassMember>,
}

struct BaseChain<'hierarchy> {
    hierarchy: &'hierarchy ResolvedClassHierarchy,
    next: Option<ClassId>,
    remaining: usize,
}

impl Iterator for BaseChain<'_> {
    type Item = ClassId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let class = self.next?;
        self.remaining -= 1;
        self.next = self.hierarchy.entry(class)?.direct_base;
        Some(class)
    }
}
