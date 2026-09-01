//! Entry selection and deterministic reachable parsed-module loading.

mod compiler_dependencies;
mod diagnostic;
mod dump;
mod entry;
mod load;
mod measurement;
mod model;

pub use dump::dump_module_graph;
pub use load::load_module_graph;
pub(crate) use load::load_module_graph_measured;
pub(crate) use measurement::{
    MeasuredModuleGraphLoad, ModuleLoadMeasurementOptions, ModuleLoadMeasurements, ModuleParseStage,
};
pub use model::{
    CompilerDependencyEvidence, CompilerDependencyKind, LoadedModule, ModuleGraph,
    ModuleGraphLoadFailure, ModuleImportEdge,
};

#[cfg(test)]
mod tests;
