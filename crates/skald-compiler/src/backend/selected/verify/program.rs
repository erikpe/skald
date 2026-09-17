//! Complete selected inventories retain receipts, allowing callable storage release.
use super::{SelectedReceipt, VerifiedSelectedCallable};
use crate::backend::selected::SelectionContext;
use crate::backend::{
    lir::ProgramError,
    plan::{ArtifactId, LirCallableId},
};
use std::collections::{BTreeMap, BTreeSet};
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct SelectedProgramBuilder<'p> {
    context: &'p SelectionContext<'p>,
    expected: BTreeSet<LirCallableId>,
    completed: BTreeMap<LirCallableId, SelectedReceipt<'p>>,
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct VerifiedSelectedProgram<'p> {
    context: &'p SelectionContext<'p>,
    receipts: BTreeMap<LirCallableId, SelectedReceipt<'p>>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p> SelectedProgramBuilder<'p> {
    pub(in crate::backend) fn new(context: &'p SelectionContext<'p>) -> Self {
        let mut expected: BTreeSet<_> = context
            .extension
            .parent()
            .receipts()
            .map(|(key, _)| *key)
            .collect();
        expected.extend(context.extension.declarations().filter_map(|decl| {
            if let ArtifactId::Callable(key) = decl.key {
                Some(key)
            } else {
                None
            }
        }));
        Self {
            context,
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
        if !self.expected.contains(&receipt.key()) {
            return Err(crate::backend::plan::PlanError::UnknownDeclaration.into());
        }
        if self.completed.contains_key(&receipt.key()) {
            return Err(ProgramError::DuplicateDefinition);
        }
        self.completed.insert(receipt.key(), receipt.clone());
        Ok(())
    }
    pub(in crate::backend) fn finish(self) -> Result<VerifiedSelectedProgram<'p>, ProgramError> {
        for key in self.expected {
            if !self.completed.contains_key(&key) {
                return Err(ProgramError::MissingDefinition(ArtifactId::Callable(key)));
            }
        }
        Ok(VerifiedSelectedProgram {
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
    pub(in crate::backend) fn context(&self) -> &'p SelectionContext<'p> {
        self.context
    }
    pub(in crate::backend) fn receipts(
        &self,
    ) -> impl ExactSizeIterator<Item = &SelectedReceipt<'p>> {
        self.receipts.values()
    }
}
