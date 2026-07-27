//! Whole-program resolution orchestration and responsibility-oriented stages.

use std::collections::HashMap;

use super::{
    body::{resolve_callable_body, BodyResolutionEnvironment, CallableResolutionContext},
    *,
};
use crate::{
    diagnostics::Diagnostic,
    identity::{CallableId, InterfaceRequirementId, ModuleId, ParameterId},
};

mod class;
mod class_body;
mod hierarchy;
mod interface;
mod resolver;
mod virtuals;

use super::{
    imports::{collect_module_bindings, collect_ordinary_bindings},
    name_lookup::{ModuleLookup, ModuleLookupProgram, TopLevelLookup},
};
use class::{collect_class, ClassWorkItem};
use class_body::resolve_class_bodies;
use hierarchy::build_class_hierarchy;
use interface::{collect_interface_declarations, resolve_interface_claims};
use resolver::{
    resolve_parameter_binding_mode, resolve_parameters, resolve_result_type, resolved_visibility,
    ProgramResolver,
};
use virtuals::resolve_virtual_families;

pub(super) fn resolve_singleton(ast: &syntax::CompilationUnit) -> ResolveOutput {
    ProgramResolver::singleton(ast).resolve()
}

pub(super) fn resolve_graph(graph: &ModuleGraph) -> ResolveOutput {
    ProgramResolver::from_graph(graph).resolve()
}
