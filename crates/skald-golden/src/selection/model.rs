use super::{error::SelectionError, glob};
use crate::{PlannedLeaf, TestPlan};
use std::collections::BTreeSet;

/// Repeatable inclusion, exclusion, exact-ID, and variant restrictions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectionOptions {
    includes: Vec<String>,
    excludes: Vec<String>,
    exact: Option<String>,
    variants: Vec<String>,
    allow_empty: bool,
}

impl SelectionOptions {
    pub fn include(mut self, pattern: impl Into<String>) -> Self {
        self.includes.push(pattern.into());
        self
    }

    pub fn exclude(mut self, pattern: impl Into<String>) -> Self {
        self.excludes.push(pattern.into());
        self
    }

    pub fn exact(mut self, id: impl Into<String>) -> Self {
        self.exact = Some(id.into());
        self
    }

    pub fn variant(mut self, name: impl Into<String>) -> Self {
        self.variants.push(name.into());
        self
    }

    pub fn allow_empty(mut self, allow: bool) -> Self {
        self.allow_empty = allow;
        self
    }
}

/// A side-effect-free view of selected leaves in canonical-ID order.
#[derive(Debug)]
pub struct SelectedPlan<'a> {
    plan: &'a TestPlan,
    leaves: Vec<&'a PlannedLeaf>,
}

impl<'a> SelectedPlan<'a> {
    pub fn plan(&self) -> &'a TestPlan {
        self.plan
    }

    pub fn leaves(&self) -> &[&'a PlannedLeaf] {
        &self.leaves
    }

    pub fn list(&self) -> String {
        lines(self.leaves.iter().map(|leaf| leaf.id()))
    }

    pub fn list_tests(&self) -> String {
        let mut tests = BTreeSet::new();
        let mut builds = BTreeSet::new();
        for leaf in &self.leaves {
            tests.insert(leaf.test_id());
            builds.insert(leaf.build_id());
        }
        let values = tests
            .into_iter()
            .map(|id| format!("test  {id}"))
            .chain(builds.into_iter().map(|id| format!("build {id}")));
        lines(values)
    }

    pub fn explain(&self, id: &str) -> Result<String, SelectionError> {
        if !self.leaves.iter().any(|leaf| leaf.id() == id) {
            return Err(SelectionError::new(format!(
                "leaf {id:?} is not in the current selection"
            )));
        }
        self.plan
            .explain(id)
            .ok_or_else(|| SelectionError::new(format!("unknown leaf ID {id:?}")))
    }
}

/// Applies selection only after a complete plan has been validated.
pub fn select<'a>(
    plan: &'a TestPlan,
    options: &SelectionOptions,
) -> Result<SelectedPlan<'a>, SelectionError> {
    if options.exact.is_some() && !options.includes.is_empty() {
        return Err(SelectionError::new(
            "exact selection cannot be combined with include filters",
        ));
    }

    if let Some(exact) = &options.exact {
        if plan.leaf(exact).is_none() && !options.allow_empty {
            return Err(SelectionError::new(format!(
                "unknown exact leaf ID {exact:?}"
            )));
        }
    }

    let leaves = plan
        .leaves()
        .iter()
        .filter(|leaf| included(leaf, options))
        .filter(|leaf| {
            options.variants.is_empty() || options.variants.iter().any(|v| v == leaf.variant())
        })
        .filter(|leaf| {
            !options
                .excludes
                .iter()
                .any(|pattern| matches_leaf(pattern, leaf))
        })
        .collect::<Vec<_>>();

    if leaves.is_empty() && !options.allow_empty {
        return Err(SelectionError::new(
            "selection matched no golden-test leaves; use --allow-empty to permit this",
        ));
    }
    Ok(SelectedPlan { plan, leaves })
}

fn included(leaf: &PlannedLeaf, options: &SelectionOptions) -> bool {
    if let Some(exact) = &options.exact {
        leaf.id() == exact
    } else if options.includes.is_empty() {
        true
    } else {
        options
            .includes
            .iter()
            .any(|pattern| matches_leaf(pattern, leaf))
    }
}

fn matches_leaf(pattern: &str, leaf: &PlannedLeaf) -> bool {
    glob::matches(pattern, leaf.id())
        || glob::matches(pattern, leaf.spec_id())
        || glob::matches(pattern, leaf.spec_relative_path())
        || leaf
            .source_relative()
            .is_some_and(|source| glob::matches(pattern, source))
}

fn lines(values: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let mut output = String::new();
    for value in values {
        output.push_str(value.as_ref());
        output.push('\n');
    }
    output
}
