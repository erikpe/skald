//! Per-module top-level declaration indexes and direct public surfaces.

use crate::{
    id_table::DenseIdTable,
    identity::{ClassId, FunctionId, InterfaceId, ModuleId},
    module::ModulePath,
    source::Span,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedVisibility {
    Private,
    Public,
}

impl ResolvedVisibility {
    pub const fn is_public(self) -> bool {
        matches!(self, Self::Public)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedTopLevelId {
    Function(FunctionId),
    Class(ClassId),
    Interface(InterfaceId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedModuleDeclaration {
    pub name: String,
    pub name_span: Span,
    pub visibility: ResolvedVisibility,
    pub declaration: ResolvedTopLevelId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedModuleDeclarations {
    pub module: ModuleId,
    declarations: Vec<ResolvedModuleDeclaration>,
}

impl ResolvedModuleDeclarations {
    pub(crate) fn new(module: ModuleId, declarations: Vec<ResolvedModuleDeclaration>) -> Self {
        Self {
            module,
            declarations,
        }
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedModuleDeclaration> {
        self.declarations.iter()
    }

    pub fn public_surface(&self) -> impl Iterator<Item = &ResolvedModuleDeclaration> {
        self.declarations
            .iter()
            .filter(|declaration| declaration.visibility.is_public())
    }

    pub fn get(&self, name: &str) -> Option<&ResolvedModuleDeclaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.name == name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedModuleDeclarationTable {
    entries: DenseIdTable<ModuleId, ResolvedModuleDeclarations>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedOrdinaryBinding {
    pub local_name: String,
    pub target_module: ModuleId,
    pub target: ResolvedTopLevelId,
    pub name_span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedOrdinaryBindings {
    pub module: ModuleId,
    bindings: Vec<ResolvedOrdinaryBinding>,
}

impl ResolvedOrdinaryBindings {
    pub(crate) fn new(module: ModuleId, bindings: Vec<ResolvedOrdinaryBinding>) -> Self {
        Self { module, bindings }
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedOrdinaryBinding> {
        self.bindings.iter()
    }

    pub fn get(&self, local_name: &str) -> Option<&ResolvedOrdinaryBinding> {
        self.bindings
            .iter()
            .find(|binding| binding.local_name == local_name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedOrdinaryBindingTable {
    entries: DenseIdTable<ModuleId, ResolvedOrdinaryBindings>,
}

impl ResolvedOrdinaryBindingTable {
    pub(crate) fn new(entries: Vec<ResolvedOrdinaryBindings>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.module),
        }
    }

    pub fn get(&self, module: ModuleId) -> Option<&ResolvedOrdinaryBindings> {
        self.entries.get(module, |entry| entry.module)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedOrdinaryBindings> {
        self.entries.iter()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedModuleBinding {
    pub local_path: ModulePath,
    pub target: ModuleId,
    pub name_span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedModuleBindings {
    pub module: ModuleId,
    bindings: Vec<ResolvedModuleBinding>,
}

impl ResolvedModuleBindings {
    pub(crate) fn new(module: ModuleId, bindings: Vec<ResolvedModuleBinding>) -> Self {
        Self { module, bindings }
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedModuleBinding> {
        self.bindings.iter()
    }

    pub fn get(&self, local_path: &ModulePath) -> Option<&ResolvedModuleBinding> {
        self.bindings
            .iter()
            .find(|binding| &binding.local_path == local_path)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedModuleBindingTable {
    entries: DenseIdTable<ModuleId, ResolvedModuleBindings>,
}

impl ResolvedModuleBindingTable {
    pub(crate) fn new(entries: Vec<ResolvedModuleBindings>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.module),
        }
    }

    pub fn get(&self, module: ModuleId) -> Option<&ResolvedModuleBindings> {
        self.entries.get(module, |entry| entry.module)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedModuleBindings> {
        self.entries.iter()
    }
}

impl ResolvedModuleDeclarationTable {
    pub(crate) fn new(entries: Vec<ResolvedModuleDeclarations>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.module),
        }
    }

    pub fn get(&self, module: ModuleId) -> Option<&ResolvedModuleDeclarations> {
        self.entries.get(module, |entry| entry.module)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedModuleDeclarations> {
        self.entries.iter()
    }

    pub fn declaration(
        &self,
        module: ModuleId,
        declaration: ResolvedTopLevelId,
    ) -> Option<&ResolvedModuleDeclaration> {
        self.get(module)?
            .iter()
            .find(|candidate| candidate.declaration == declaration)
    }
}
