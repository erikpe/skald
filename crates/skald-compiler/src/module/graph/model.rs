use crate::{
    diagnostics::Diagnostics,
    identity::ModuleId,
    source::{SourceDatabase, Span},
    syntax::CompilationUnit,
};

use super::super::{ModulePath, ModuleProvenance};

/// One canonical direct dependency edge in a loaded module graph.
///
/// Repeated source declarations of the same imported module share one edge
/// while retaining explicit-import and compiler-owned literal evidence
/// separately. Only explicit imports participate in source name binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleImportEdge {
    target: ModuleId,
    import_spans: Vec<Span>,
    string_literal_spans: Vec<Span>,
}

impl ModuleImportEdge {
    pub(super) fn new(
        target: ModuleId,
        import_spans: Vec<Span>,
        string_literal_spans: Vec<Span>,
    ) -> Self {
        debug_assert!(!import_spans.is_empty() || !string_literal_spans.is_empty());
        Self {
            target,
            import_spans,
            string_literal_spans,
        }
    }

    pub const fn target(&self) -> ModuleId {
        self.target
    }

    pub fn import_spans(&self) -> &[Span] {
        &self.import_spans
    }

    pub fn string_literal_spans(&self) -> &[Span] {
        &self.string_literal_spans
    }

    pub(super) fn first_evidence_span(&self) -> Span {
        self.import_spans
            .first()
            .or_else(|| self.string_literal_spans.first())
            .copied()
            .expect("a dependency edge retains source evidence")
    }
}

/// One parsed source instance in canonical module-path order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedModule {
    provenance: ModuleProvenance,
    ast: CompilationUnit,
    imports: Vec<ModuleImportEdge>,
}

impl LoadedModule {
    pub(super) fn new(
        provenance: ModuleProvenance,
        ast: CompilationUnit,
        imports: Vec<ModuleImportEdge>,
    ) -> Self {
        Self {
            provenance,
            ast,
            imports,
        }
    }

    pub const fn provenance(&self) -> &ModuleProvenance {
        &self.provenance
    }

    pub const fn ast(&self) -> &CompilationUnit {
        &self.ast
    }

    pub fn imports(&self) -> &[ModuleImportEdge] {
        &self.imports
    }
}

/// A complete, parsed, reachable, and acyclic module graph.
///
/// Modules and sources are dense in the same canonical logical-path order.
/// The selected entry is metadata and does not influence allocation order.
#[derive(Debug)]
pub struct ModuleGraph {
    sources: SourceDatabase,
    entry: ModuleId,
    modules: Vec<LoadedModule>,
}

impl ModuleGraph {
    pub(super) fn new(
        sources: SourceDatabase,
        entry: ModuleId,
        modules: Vec<LoadedModule>,
    ) -> Self {
        Self {
            sources,
            entry,
            modules,
        }
    }

    pub const fn sources(&self) -> &SourceDatabase {
        &self.sources
    }

    pub const fn entry(&self) -> ModuleId {
        self.entry
    }

    pub fn modules(&self) -> &[LoadedModule] {
        &self.modules
    }

    pub fn module(&self, id: ModuleId) -> Option<&LoadedModule> {
        self.modules.get(id.index())
    }

    pub fn find(&self, path: &ModulePath) -> Option<&LoadedModule> {
        self.modules
            .binary_search_by(|module| module.provenance().module_path().cmp(path))
            .ok()
            .map(|index| &self.modules[index])
    }

    pub(crate) fn into_sources(self) -> SourceDatabase {
        self.sources
    }
}

/// Structured source diagnostics produced before a graph can be finalized.
#[derive(Debug)]
pub struct ModuleGraphLoadFailure {
    sources: SourceDatabase,
    diagnostics: Diagnostics,
}

impl ModuleGraphLoadFailure {
    pub(super) fn new(sources: SourceDatabase, diagnostics: Diagnostics) -> Self {
        Self {
            sources,
            diagnostics,
        }
    }

    pub const fn sources(&self) -> &SourceDatabase {
        &self.sources
    }

    pub const fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    pub fn into_parts(self) -> (SourceDatabase, Diagnostics) {
        (self.sources, self.diagnostics)
    }
}
