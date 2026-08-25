use crate::{
    diagnostics::Diagnostics,
    identity::ModuleId,
    source::{SourceDatabase, Span},
    syntax::CompilationUnit,
};

use super::super::{ModulePath, ModuleProvenance};

/// Source construct that causes the compiler to load a canonical module
/// without creating a source name binding.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompilerDependencyKind {
    StringLiteral,
    GeneralIteration,
}

/// Evidence for one compiler-owned dependency kind on a canonical edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerDependencyEvidence {
    kind: CompilerDependencyKind,
    spans: Vec<Span>,
}

impl CompilerDependencyEvidence {
    pub(super) fn new(kind: CompilerDependencyKind, spans: Vec<Span>) -> Self {
        debug_assert!(!spans.is_empty());
        Self { kind, spans }
    }

    pub const fn kind(&self) -> CompilerDependencyKind {
        self.kind
    }

    pub fn spans(&self) -> &[Span] {
        &self.spans
    }
}

/// One canonical direct dependency edge in a loaded module graph.
///
/// Repeated source declarations of the same imported module share one edge
/// while retaining explicit-import and typed compiler-owned dependency
/// evidence separately. Only explicit imports participate in source name
/// binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleImportEdge {
    target: ModuleId,
    import_spans: Vec<Span>,
    compiler_dependencies: Vec<CompilerDependencyEvidence>,
}

impl ModuleImportEdge {
    pub(super) fn new(
        target: ModuleId,
        import_spans: Vec<Span>,
        compiler_dependencies: Vec<CompilerDependencyEvidence>,
    ) -> Self {
        debug_assert!(!import_spans.is_empty() || !compiler_dependencies.is_empty());
        debug_assert!(compiler_dependencies
            .windows(2)
            .all(|pair| pair[0].kind < pair[1].kind));
        Self {
            target,
            import_spans,
            compiler_dependencies,
        }
    }

    pub const fn target(&self) -> ModuleId {
        self.target
    }

    pub fn import_spans(&self) -> &[Span] {
        &self.import_spans
    }

    pub fn string_literal_spans(&self) -> &[Span] {
        self.compiler_dependency_spans(CompilerDependencyKind::StringLiteral)
    }

    pub fn compiler_dependencies(&self) -> &[CompilerDependencyEvidence] {
        &self.compiler_dependencies
    }

    pub fn compiler_dependency_spans(&self, kind: CompilerDependencyKind) -> &[Span] {
        self.compiler_dependencies
            .binary_search_by_key(&kind, CompilerDependencyEvidence::kind)
            .ok()
            .map(|index| self.compiler_dependencies[index].spans())
            .unwrap_or_default()
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

/// A complete, parsed, and reachable module graph.
///
/// Modules and sources are dense in the same canonical logical-path order.
/// Direct dependency edges may be cyclic. The selected entry and graph shape
/// are metadata and do not influence allocation order.
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
