//! Whole-program physical closure and temporary fragment ownership.
mod render;
mod store;
#[cfg(test)]
mod tests;

use super::verify::{PhysicalReceipt, VerifiedPhysicalCallable};
use crate::backend::{
    lir::ProgramError as InventoryError,
    plan::{ArtifactId, BodyDisposition, LirCallableId, PlanError},
    selected::{SelectionContext, VerifiedSelectedProgram},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, io,
};
pub(super) use store::FragmentStore;
pub(in crate::backend) use store::TempFragmentStore;

#[derive(Debug)]
pub(in crate::backend) enum ProgramError {
    Inventory(InventoryError),
    DuplicateDefinition,
    MissingDefinition(ArtifactId),
    StaleReceipt,
    Render,
    Store(io::Error),
}
impl std::fmt::Display for ProgramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inventory(error) => write!(f, "physical program inventory: {error:?}"),
            Self::DuplicateDefinition => f.write_str("duplicate physical fragment"),
            Self::MissingDefinition(id) => write!(f, "missing physical definition: {id:?}"),
            Self::StaleReceipt => f.write_str("stale physical receipt"),
            Self::Render => f.write_str("physical rendering failed"),
            Self::Store(error) => write!(f, "physical fragment storage: {error}"),
        }
    }
}
impl std::error::Error for ProgramError {}
impl From<InventoryError> for ProgramError {
    fn from(e: InventoryError) -> Self {
        Self::Inventory(e)
    }
}
impl From<PlanError> for ProgramError {
    fn from(e: PlanError) -> Self {
        Self::Inventory(e.into())
    }
}
impl From<io::Error> for ProgramError {
    fn from(e: io::Error) -> Self {
        Self::Store(e)
    }
}
impl From<fmt::Error> for ProgramError {
    fn from(_: fmt::Error) -> Self {
        Self::Render
    }
}

pub(in crate::backend) struct PhysicalProgramBuilder<'p, S = TempFragmentStore> {
    context: &'p SelectionContext<'p>,
    expected: BTreeSet<LirCallableId>,
    completed: BTreeMap<LirCallableId, PhysicalReceipt<'p>>,
    store: S,
    symbols: Symbols,
}
pub(in crate::backend) struct VerifiedAssembly {
    text: String,
}

impl<'p> PhysicalProgramBuilder<'p, TempFragmentStore> {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::backend) fn temporary(
        context: &'p SelectionContext<'p>,
    ) -> Result<Self, ProgramError> {
        Self::with_store_and_symbols(context, TempFragmentStore::create()?, BTreeMap::new())
    }
    pub(in crate::backend) fn temporary_with_external_symbols(
        context: &'p SelectionContext<'p>,
        symbols: BTreeMap<crate::identity::ExternalLinkId, String>,
    ) -> Result<Self, ProgramError> {
        Self::with_store_and_symbols(context, TempFragmentStore::create()?, symbols)
    }
}
impl<'p, S: FragmentStore> PhysicalProgramBuilder<'p, S> {
    #[cfg(test)]
    pub(in crate::backend::x86_64_sysv::native::physical) fn with_store(
        context: &'p SelectionContext<'p>,
        store: S,
        external: BTreeMap<crate::identity::ExternalLinkId, String>,
    ) -> Self {
        Self::with_store_and_symbols(context, store, external)
            .expect("test plans without externals have complete symbol facts")
    }
    fn with_store_and_symbols(
        context: &'p SelectionContext<'p>,
        store: S,
        external: BTreeMap<crate::identity::ExternalLinkId, String>,
    ) -> Result<Self, ProgramError> {
        let mut expected = context
            .catalog()
            .plan()
            .callables()
            .filter(|c| c.body == BodyDisposition::Required)
            .map(|c| c.key)
            .collect::<BTreeSet<_>>();
        expected.extend(
            context
                .catalog()
                .declarations()
                .filter_map(|d| match d.key {
                    ArtifactId::Callable(key) => Some(key),
                    _ => None,
                }),
        );
        Ok(Self {
            context,
            expected,
            completed: BTreeMap::new(),
            store,
            symbols: Symbols::new(context, external)?,
        })
    }
    pub(in crate::backend) fn complete(
        &mut self,
        body: &VerifiedPhysicalCallable<'_, '_, '_, 'p>,
    ) -> Result<(), ProgramError> {
        let receipt = body.receipt();
        receipt.parent().require_context(self.context)?;
        if !receipt.matches(body) {
            return Err(ProgramError::StaleReceipt);
        }
        let key = receipt.parent().key();
        if !self.expected.contains(&key) {
            return Err(ProgramError::MissingDefinition(ArtifactId::Callable(key)));
        }
        if self.completed.contains_key(&key) {
            return Err(ProgramError::DuplicateDefinition);
        }
        let fragment = render::callable(body, &|id| self.symbols.name(id))?;
        self.store.write(key, &fragment)?;
        self.completed.insert(key, receipt);
        Ok(())
    }
    pub(in crate::backend) fn finish(
        mut self,
        parent: &'p VerifiedSelectedProgram<'p>,
    ) -> Result<VerifiedAssembly, ProgramError> {
        if !std::ptr::eq(parent.context(), self.context) {
            return Err(PlanError::WrongContext.into());
        }
        for key in &self.expected {
            let receipt = self
                .completed
                .get(key)
                .ok_or(ProgramError::MissingDefinition(ArtifactId::Callable(*key)))?;
            parent.require_input(receipt.parent())?;
            for reference in receipt.references() {
                self.context
                    .catalog()
                    .artifact(*reference, reference.category())?;
            }
        }
        let mut text = String::from(".intel_syntax noprefix\n.text\n");
        for key in &self.expected {
            text.push_str(&self.store.read(*key)?);
        }
        text.push_str(".section .rodata\n");
        for data in parent.parent().data().chain(self.context.catalog().data()) {
            render::data(&mut text, data, &|id| self.symbols.name(id))?;
        }
        text.push_str(".section .note.GNU-stack,\"\",@progbits\n");
        for key in self.completed.keys().copied().collect::<Vec<_>>() {
            self.store.remove(key)?;
        }
        Ok(VerifiedAssembly { text })
    }
}
impl VerifiedAssembly {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::backend) fn as_str(&self) -> &str {
        &self.text
    }
    pub(in crate::backend) fn into_string(self) -> String {
        self.text
    }
}

struct Symbols {
    names: BTreeMap<ArtifactId, String>,
}
impl Symbols {
    fn new(
        context: &SelectionContext<'_>,
        mut external: BTreeMap<crate::identity::ExternalLinkId, String>,
    ) -> Result<Self, ProgramError> {
        let mut ids = context
            .catalog()
            .plan()
            .artifacts()
            .map(|d| d.key)
            .collect::<BTreeSet<_>>();
        ids.extend(context.catalog().declarations().map(|d| d.key));
        ids.extend(
            context
                .catalog()
                .plan()
                .callables()
                .map(|d| ArtifactId::Callable(d.key)),
        );
        let names = ids
            .into_iter()
            .enumerate()
            .map(|(i, id)| -> Result<_, ProgramError> {
                Ok((
                    id,
                    match id {
                        ArtifactId::Runtime(service) => runtime_symbol(service).to_owned(),
                        ArtifactId::TraceTls => crate::backend::RUNTIME_TRACE_TOP_SYMBOL.to_owned(),
                        ArtifactId::Callable(LirCallableId::Entry) => "main".to_owned(),
                        ArtifactId::External(key) => external
                            .remove(&key)
                            .filter(|name| !name.is_empty())
                            .ok_or(ProgramError::MissingDefinition(id))?,
                        ArtifactId::Callable(_) => format!(".Lska_native_callable_{i}"),
                        ArtifactId::Data(_) => format!(".Lska_native_data_{i}"),
                    },
                ))
            })
            .collect::<Result<_, _>>()?;
        if !external.is_empty() {
            return Err(PlanError::UnknownDeclaration.into());
        }
        Ok(Self { names })
    }
    fn name(&self, id: ArtifactId) -> String {
        self.names
            .get(&id)
            .cloned()
            .expect("closed typed dependency must have a symbol")
    }
}

fn runtime_symbol(service: crate::backend::plan::RuntimeService) -> &'static str {
    use crate::backend::plan::RuntimeService::*;
    match service {
        Allocate => "ska_rt_alloc",
        Free => "ska_rt_free",
        Panic => "ska_rt_panic",
        IoStandardHandle => "ska_rt_io_standard_handle",
        IoOpen => "ska_rt_io_open",
        IoRead => "ska_rt_io_read",
        IoWrite => "ska_rt_io_write",
        IoClose => "ska_rt_io_close",
        AbiMarker => crate::backend::RUNTIME_ABI_MARKER_SYMBOL,
    }
}
