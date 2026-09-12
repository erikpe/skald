//! Whole-program resolution orchestration and responsibility-oriented stages.

use std::path::Path;

use crate::{module::ModuleGraph, syntax};

use super::ResolveOutput;

mod class;
mod class_body;
mod generic_templates;
mod hierarchy;
mod interface;
mod intrinsic_registry;
mod iterable_language_item;
mod language_item_sources;
mod operator_language_item;
mod range_language_item;
mod resolver;
mod semantic_range_requests;
mod specialization;
mod stages;
mod static_initializer;
mod string_language_item;
mod virtuals;

pub(in crate::resolve::resolver) use generic_templates::TemplateTypeResolver;
use resolver::ProgramResolver;

pub(super) fn resolve_singleton(
    ast: &syntax::CompilationUnit,
    source_path: &Path,
) -> ResolveOutput {
    ProgramResolver::singleton(ast, source_path).resolve()
}

pub(super) fn resolve_graph(graph: &ModuleGraph) -> ResolveOutput {
    ProgramResolver::from_graph(graph).resolve()
}
