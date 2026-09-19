//! Canonical construction states and verifier-bound completion reconciliation.
use super::{data, DataDefinition};
use crate::backend::lir::{CompletionReceipt, VerifiedCallable};
use crate::backend::plan::{
    ArtifactId, BodyDisposition, CallableBinding, DataKey, LirCallableId, PlanError, PlanView,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum InventoryState {
    Declared,
    Building,
    Verified,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::backend) enum ProgramError {
    Plan(PlanError),
    AlreadyBuilding,
    DuplicateDefinition,
    StaleReceipt,
    DependencyMismatch,
    MissingDefinition(ArtifactId),
    InvalidInitializer,
    InvalidAddend,
    SizeOverflow,
}
impl From<PlanError> for ProgramError {
    fn from(value: PlanError) -> Self {
        Self::Plan(value)
    }
}
enum WorkEntry<'p> {
    Declared,
    Building,
    Verified(CompletionReceipt<'p>),
}
pub(in crate::backend) struct ProgramBuilder<'p> {
    parent: PlanView<'p>,
    states: BTreeMap<LirCallableId, WorkEntry<'p>>,
    data: BTreeMap<DataKey, DataDefinition>,
    references: BTreeSet<ArtifactId>,
}
/// Fields and construction stay with inventory verification. No emission authority.
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct VerifiedProgram<'p> {
    parent: PlanView<'p>,
    snapshot: Arc<()>,
    chosen: BTreeMap<LirCallableId, CompletionReceipt<'p>>,
    data: BTreeMap<DataKey, DataDefinition>,
}
impl<'p> ProgramBuilder<'p> {
    pub(in crate::backend) fn new(parent: PlanView<'p>) -> Self {
        Self {
            parent,
            states: parent
                .callables()
                .filter(|c| c.body == BodyDisposition::Required)
                .map(|c| (c.key, WorkEntry::Declared))
                .collect(),
            data: BTreeMap::new(),
            references: BTreeSet::new(),
        }
    }
    /// Recursive discovery returns the reserved state, never reenters construction.
    pub(in crate::backend) fn request(
        &self,
        key: LirCallableId,
    ) -> Result<InventoryState, ProgramError> {
        self.parent.callable(key)?;
        self.states
            .get(&key)
            .map(|state| match state {
                WorkEntry::Declared => InventoryState::Declared,
                WorkEntry::Building => InventoryState::Building,
                WorkEntry::Verified(_) => InventoryState::Verified,
            })
            .ok_or(PlanError::UnknownDeclaration.into())
    }
    pub(in crate::backend) fn next(&self) -> Option<LirCallableId> {
        self.states
            .iter()
            .find_map(|(key, state)| matches!(state, WorkEntry::Declared).then_some(*key))
    }
    pub(in crate::backend) fn begin(
        &mut self,
        key: LirCallableId,
    ) -> Result<CallableBinding<'p>, ProgramError> {
        match self.request(key)? {
            InventoryState::Building => return Err(ProgramError::AlreadyBuilding),
            InventoryState::Verified => return Err(ProgramError::DuplicateDefinition),
            InventoryState::Declared => {}
        }
        let owner = self.parent.callable(key)?;
        self.states.insert(key, WorkEntry::Building);
        Ok(owner)
    }
    /// The body is the chosen definition; a separately supplied receipt must match it.
    /// Registration retains its witness and requirements, not its predecessor payload.
    pub(in crate::backend) fn complete(
        &mut self,
        body: &VerifiedCallable<'p>,
        receipt: &CompletionReceipt<'p>,
    ) -> Result<(), ProgramError> {
        self.parent
            .require_same_context(body.draft().owner().context())?;
        self.parent
            .require_same_context(receipt.owner().context())?;
        if !receipt.matches(body) {
            return Err(ProgramError::StaleReceipt);
        }
        let key = receipt.owner().key();
        if let Some(generated) = self
            .parent
            .resources()
            .generated
            .iter()
            .find(|fact| fact.callable == key)
        {
            if receipt.references() != &generated.dependencies {
                return Err(ProgramError::DependencyMismatch);
            }
        }
        match self.request(key)? {
            InventoryState::Declared => {
                return Err(ProgramError::MissingDefinition(ArtifactId::Callable(key)))
            }
            InventoryState::Verified => return Err(ProgramError::DuplicateDefinition),
            InventoryState::Building => {}
        }
        self.references.extend(receipt.references().iter().copied());
        self.states
            .insert(key, WorkEntry::Verified(receipt.clone()));
        Ok(())
    }
    pub(in crate::backend) fn define_data(
        &mut self,
        definition: DataDefinition,
    ) -> Result<(), ProgramError> {
        if self.data.contains_key(&definition.key) {
            return Err(ProgramError::DuplicateDefinition);
        }
        if let DataKey::Static(field) = definition.key {
            if self.parent.static_storage_disposition(field)
                == Some(crate::backend::plan::StaticStorageDisposition::RetainedInactive)
            {
                return Err(ProgramError::Plan(
                    crate::backend::plan::PlanError::InvalidDomain,
                ));
            }
        }
        let references = data::check(&definition, self.parent, |key, category| {
            data::parent_artifact(self.parent, key, category)
        })?;
        self.references.extend(references);
        self.data.insert(definition.key, definition);
        Ok(())
    }
    pub(in crate::backend) fn finish(self) -> Result<VerifiedProgram<'p>, ProgramError> {
        // Declaration presence alone never satisfies body or initializer completion.
        for declaration in self.parent.artifacts() {
            match declaration.key {
                ArtifactId::Callable(key) if self.states.contains_key(&key) => {
                    if !matches!(self.states.get(&key), Some(WorkEntry::Verified(_))) {
                        return Err(ProgramError::MissingDefinition(declaration.key));
                    }
                }
                ArtifactId::Data(key) => {
                    let required = !matches!(key, DataKey::Static(field) if !self.parent.is_active_static(field));
                    if required && !self.data.contains_key(&key) {
                        return Err(ProgramError::MissingDefinition(declaration.key));
                    }
                }
                _ => {}
            }
        }
        for key in &self.references {
            data::parent_artifact(self.parent, *key, key.category())?;
        }
        Ok(VerifiedProgram {
            parent: self.parent,
            snapshot: Arc::new(()),
            chosen: self
                .states
                .into_iter()
                .filter_map(|(key, state)| match state {
                    WorkEntry::Verified(receipt) => Some((key, receipt)),
                    _ => None,
                })
                .collect(),
            data: self.data,
        })
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p> VerifiedProgram<'p> {
    /// Consuming inventory authority invalidates dependent complete-program publication.
    pub(in crate::backend) fn edit(
        self,
        body: VerifiedCallable<'p>,
    ) -> Result<(ProgramBuilder<'p>, crate::backend::lir::LoweredEditor<'p>), ProgramError> {
        self.require_input(&body.receipt())?;
        let key = body.receipt().owner().key();
        let mut states = self
            .chosen
            .into_iter()
            .map(|(key, receipt)| (key, WorkEntry::Verified(receipt)))
            .collect::<BTreeMap<_, _>>();
        states.insert(key, WorkEntry::Building);
        let mut references = self
            .data
            .values()
            .flat_map(|definition| definition.initializers.iter())
            .filter_map(|i| match i {
                super::DataInitializer::Address { target, .. } => Some(*target),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        for state in states.values() {
            if let WorkEntry::Verified(receipt) = state {
                references.extend(receipt.references().iter().copied());
            }
        }
        Ok((
            ProgramBuilder {
                parent: self.parent,
                states,
                data: self.data,
                references,
            },
            body.into_editor(),
        ))
    }
    pub(in crate::backend) fn require_same_snapshot(
        &self,
        other: &Self,
    ) -> Result<(), ProgramError> {
        self.parent.require_same_context(other.parent)?;
        if !Arc::ptr_eq(&self.snapshot, &other.snapshot) {
            return Err(PlanError::WrongContext.into());
        }
        Ok(())
    }
    pub(in crate::backend) fn parent(&self) -> PlanView<'p> {
        self.parent
    }
    pub(in crate::backend) fn receipts(
        &self,
    ) -> impl ExactSizeIterator<Item = (&LirCallableId, &CompletionReceipt<'p>)> {
        self.chosen.iter()
    }
    pub(in crate::backend) fn data(&self) -> impl ExactSizeIterator<Item = &DataDefinition> {
        self.data.values()
    }
    pub(in crate::backend) fn require_input(
        &self,
        receipt: &CompletionReceipt<'p>,
    ) -> Result<(), ProgramError> {
        self.parent
            .require_same_context(receipt.owner().context())?;
        let expected =
            self.chosen
                .get(&receipt.owner().key())
                .ok_or(ProgramError::MissingDefinition(ArtifactId::Callable(
                    receipt.owner().key(),
                )))?;
        if !expected.same_snapshot(receipt) {
            return Err(ProgramError::StaleReceipt);
        }
        Ok(())
    }
}
