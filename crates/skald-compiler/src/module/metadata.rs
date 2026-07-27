//! Validated module metadata carried by whole-program semantic IR.

use std::{collections::HashSet, fmt};

use crate::{
    identity::{ModuleId, PackageId, ProviderId},
    source::SourceId,
};

use super::{graph::ModuleGraph, ModulePath, ModuleProvenance, ModuleSourceLocation};

/// Dense, request-local module metadata plus the selected entry module.
///
/// The vector order is semantic: entry `n` must own `ModuleId::new(n)`.
/// Selecting another entry therefore changes metadata only; it never changes
/// module or declaration allocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramModuleTable {
    selected: ModuleId,
    entries: Vec<ModuleProvenance>,
}

impl ProgramModuleTable {
    /// Copies semantic module metadata from a successfully loaded graph.
    pub fn from_graph(graph: &ModuleGraph) -> Self {
        Self::new(
            graph.entry(),
            graph
                .modules()
                .iter()
                .map(|module| module.provenance().clone())
                .collect(),
        )
        .expect("loaded module graphs have validated dense metadata")
    }

    pub fn new(
        selected: ModuleId,
        entries: Vec<ModuleProvenance>,
    ) -> Result<Self, ProgramModuleTableError> {
        let table = Self { selected, entries };
        table.validate()?;
        Ok(table)
    }

    /// Revalidates metadata after deserialization or test mutation.
    pub fn validate(&self) -> Result<(), ProgramModuleTableError> {
        let mut paths = HashSet::with_capacity(self.entries.len());
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.module_id().index() != index {
                return Err(ProgramModuleTableError::NonDense {
                    index,
                    module: entry.module_id(),
                });
            }
            if !paths.insert(entry.module_path().clone()) {
                return Err(ProgramModuleTableError::DuplicatePath(
                    entry.module_path().clone(),
                ));
            }
        }
        if self.entries.get(self.selected.index()).is_none() {
            return Err(ProgramModuleTableError::UnknownSelected(self.selected));
        }
        Ok(())
    }

    pub const fn selected(&self) -> ModuleId {
        self.selected
    }

    pub fn get(&self, id: ModuleId) -> Option<&ModuleProvenance> {
        self.entries
            .get(id.index())
            .filter(|entry| entry.module_id() == id)
    }

    pub fn find(&self, path: &ModulePath) -> Option<&ModuleProvenance> {
        self.entries
            .iter()
            .find(|entry| entry.module_path() == path)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ModuleProvenance> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn singleton(source_id: SourceId) -> Self {
        let module = ModuleId::new(0);
        Self::new(
            module,
            vec![ModuleProvenance::new(
                module,
                ModulePath::try_from("main").expect("singleton module path is valid"),
                source_id,
                ProviderId::new(0),
                PackageId::new(0),
                ModuleSourceLocation::new("main.ska".into(), "main.ska".into(), None),
            )],
        )
        .expect("singleton module metadata is valid")
    }

    #[cfg(test)]
    pub(crate) fn set_module_id_for_test(&mut self, index: usize, module: ModuleId) {
        let entry = &self.entries[index];
        self.entries[index] = ModuleProvenance::new(
            module,
            entry.module_path().clone(),
            entry.source_id(),
            entry.provider_id(),
            entry.package_id(),
            entry.source_location().clone(),
        );
    }

    #[cfg(test)]
    pub(crate) fn set_selected_for_test(&mut self, selected: ModuleId) {
        self.selected = selected;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgramModuleTableError {
    NonDense { index: usize, module: ModuleId },
    DuplicatePath(ModulePath),
    UnknownSelected(ModuleId),
}

impl fmt::Display for ProgramModuleTableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonDense { index, module } => {
                write!(formatter, "module table index {index} contains {module}")
            }
            Self::DuplicatePath(path) => {
                write!(formatter, "duplicate logical module path `{path}`")
            }
            Self::UnknownSelected(module) => {
                write!(
                    formatter,
                    "selected entry module {module} is not in the module table"
                )
            }
        }
    }
}

impl std::error::Error for ProgramModuleTableError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceDatabase;

    fn provenance(id: usize, path: &str, source_id: SourceId) -> ModuleProvenance {
        ModuleProvenance::new(
            ModuleId::new(id),
            ModulePath::try_from(path).unwrap(),
            source_id,
            ProviderId::new(0),
            PackageId::new(0),
            ModuleSourceLocation::new(
                format!("{path}.ska").into(),
                format!("{path}.ska").into(),
                None,
            ),
        )
    }

    #[test]
    fn validates_density_paths_and_selected_entry() {
        let mut sources = SourceDatabase::new();
        let first = sources.add("a.ska", "");
        let second = sources.add("b.ska", "");

        assert!(matches!(
            ProgramModuleTable::new(ModuleId::new(0), vec![provenance(1, "a", first)]),
            Err(ProgramModuleTableError::NonDense { .. })
        ));
        assert!(matches!(
            ProgramModuleTable::new(
                ModuleId::new(0),
                vec![provenance(0, "a", first), provenance(1, "a", second)]
            ),
            Err(ProgramModuleTableError::DuplicatePath(_))
        ));
        assert_eq!(
            ProgramModuleTable::new(ModuleId::new(2), vec![provenance(0, "a", first)]).unwrap_err(),
            ProgramModuleTableError::UnknownSelected(ModuleId::new(2))
        );
    }

    #[test]
    fn selected_entry_does_not_change_table_identity() {
        let mut sources = SourceDatabase::new();
        let first = sources.add("a.ska", "");
        let second = sources.add("b.ska", "");
        let entries = vec![provenance(0, "a", first), provenance(1, "b", second)];

        let a = ProgramModuleTable::new(ModuleId::new(0), entries.clone()).unwrap();
        let b = ProgramModuleTable::new(ModuleId::new(1), entries).unwrap();
        assert_eq!(a.iter().collect::<Vec<_>>(), b.iter().collect::<Vec<_>>());
        assert_ne!(a.selected(), b.selected());
    }
}
