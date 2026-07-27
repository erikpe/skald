//! Entry selection and deterministic reachable parsed-module loading.

mod cycle;
mod diagnostic;
mod dump;
mod entry;
mod load;
mod model;

pub use dump::dump_module_graph;
pub use load::load_module_graph;
pub use model::{LoadedModule, ModuleGraph, ModuleGraphLoadFailure, ModuleImportEdge};

#[cfg(test)]
mod tests;
