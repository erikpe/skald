//! Complete selected inventories retain receipts, allowing callable storage release.
use super::{SelectedReceipt, VerifiedSelectedCallable};
use crate::backend::selected::SelectionContext;
use crate::backend::{
    lir::{ProgramError, VerifiedProgram},
    plan::{ArtifactId, LirCallableId},
};
use std::collections::{BTreeMap, BTreeSet};
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct SelectedProgramBuilder<'p> {
    context: &'p SelectionContext<'p>,
    // Edits remain bound to the original finalized parent authority.
    parent: Option<&'p VerifiedProgram<'p>>,
    expected: BTreeSet<LirCallableId>,
    completed: BTreeMap<LirCallableId, SelectedReceipt<'p>>,
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct VerifiedSelectedProgram<'p> {
    context: &'p SelectionContext<'p>,
    parent: &'p VerifiedProgram<'p>,
    receipts: BTreeMap<LirCallableId, SelectedReceipt<'p>>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p> SelectedProgramBuilder<'p> {
    pub(in crate::backend) fn new(context: &'p SelectionContext<'p>) -> Self {
        let expected = context
            .catalog
            .declarations()
            .filter_map(|decl| {
                if let ArtifactId::Callable(key) = decl.key {
                    Some(key)
                } else {
                    None
                }
            })
            .collect();
        Self {
            context,
            parent: None,
            expected,
            completed: BTreeMap::new(),
        }
    }
    pub(in crate::backend) fn complete<P>(
        &mut self,
        product: &VerifiedSelectedCallable<'p, P>,
        receipt: &SelectedReceipt<'p>,
    ) -> Result<(), ProgramError> {
        receipt.require_context(self.context)?;
        if !receipt.matches(product) {
            return Err(ProgramError::StaleReceipt);
        }
        self.context.catalog.artifact(
            ArtifactId::Callable(receipt.key()),
            crate::backend::plan::ArtifactCategory::Callable,
        )?;
        if self.completed.contains_key(&receipt.key()) {
            return Err(ProgramError::DuplicateDefinition);
        }
        self.completed.insert(receipt.key(), receipt.clone());
        Ok(())
    }
    pub(in crate::backend) fn finish(
        self,
        parent: &'p VerifiedProgram<'p>,
    ) -> Result<VerifiedSelectedProgram<'p>, ProgramError> {
        self.context.catalog.require_plan(parent.parent())?;
        if let Some(original) = self.parent {
            original.require_same_snapshot(parent)?;
        }
        let mut expected = parent
            .receipts()
            .map(|(key, _)| *key)
            .collect::<BTreeSet<_>>();
        expected.extend(self.expected);
        for key in &expected {
            if !self.completed.contains_key(key) {
                return Err(ProgramError::MissingDefinition(ArtifactId::Callable(*key)));
            }
        }
        for key in self.completed.keys() {
            if !expected.contains(key) {
                return Err(ProgramError::UnexpectedDefinition(ArtifactId::Callable(
                    *key,
                )));
            }
        }
        // Local selection proves derivation from a genuine callable. Only closure
        // proves that it is the executable parent's chosen snapshot.
        for receipt in self.completed.values() {
            if let Some(input) = receipt.input() {
                parent.require_input(input)?;
            }
            for reference in receipt.references() {
                self.context
                    .catalog
                    .artifact(*reference, reference.category())?;
            }
        }
        Ok(VerifiedSelectedProgram {
            parent,
            context: self.context,
            receipts: self.completed,
        })
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p> VerifiedSelectedProgram<'p> {
    pub(in crate::backend) fn edit<P: crate::backend::selected::EditablePayload>(
        self,
        body: VerifiedSelectedCallable<'p, P>,
    ) -> Result<
        (
            SelectedProgramBuilder<'p>,
            crate::backend::selected::SelectedEditor<'p, P>,
        ),
        ProgramError,
    > {
        let receipt = body.receipt();
        self.require_input(&receipt)?;
        let mut builder = SelectedProgramBuilder::new(self.context);
        builder.parent = Some(self.parent);
        builder.completed = self.receipts;
        builder.completed.remove(&receipt.key());
        Ok((builder, body.into_editor()))
    }
    pub(in crate::backend) fn require_input(
        &self,
        receipt: &SelectedReceipt<'p>,
    ) -> Result<(), ProgramError> {
        receipt.require_context(self.context)?;
        let chosen = self
            .receipts
            .get(&receipt.key())
            .ok_or(ProgramError::MissingDefinition(ArtifactId::Callable(
                receipt.key(),
            )))?;
        if !chosen.same_snapshot(receipt) {
            return Err(ProgramError::StaleReceipt);
        }
        Ok(())
    }
    pub(in crate::backend) fn parent(&self) -> &'p VerifiedProgram<'p> {
        self.parent
    }
    pub(in crate::backend) fn context(&self) -> &'p SelectionContext<'p> {
        self.context
    }
    pub(in crate::backend) fn receipts(
        &self,
    ) -> impl ExactSizeIterator<Item = &SelectedReceipt<'p>> {
        self.receipts.values()
    }
}
