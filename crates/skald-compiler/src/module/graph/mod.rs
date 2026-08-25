//! Entry selection and deterministic reachable parsed-module loading.

mod diagnostic;
mod dump;
mod entry;
mod load;
mod model;

pub use dump::dump_module_graph;
pub use load::load_module_graph;
pub use model::{
    CompilerDependencyEvidence, CompilerDependencyKind, LoadedModule, ModuleGraph,
    ModuleGraphLoadFailure, ModuleImportEdge,
};

#[cfg(test)]
mod tests;
