//! Per-module top-level declaration indexes and direct public surfaces.

use crate::{
    id_table::DenseIdTable,
    identity::{ClassId, FunctionId, InterfaceId, ModuleId},
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
}
