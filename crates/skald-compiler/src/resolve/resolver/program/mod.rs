//! Whole-program resolution orchestration and responsibility-oriented stages.

use std::collections::HashMap;
use std::path::Path;

use super::{
    body::{
        resolve_callable_body, resolve_static_initializer_expression, BodyResolutionEnvironment,
        BodySpecializationEnvironment, CallableResolutionContext,
    },
    *,
};
use crate::{
    diagnostics::Diagnostic,
    identity::{CallableId, InterfaceRequirementId, ModuleId, ParameterId},
};

mod class;
mod class_body;
mod generic_templates;
mod hierarchy;
mod interface;
mod intrinsic_registry;
mod resolver;
mod specialization;
mod static_initializer;
mod string_language_item;
mod virtuals;

use super::{
    external_links::ExternalLinkPlan,
    imports::{collect_module_bindings, collect_ordinary_bindings},
    name_lookup::{ModuleLookup, ModuleLookupProgram, TopLevelLookup},
};
use class::{collect_class, ClassWorkItem};
use class_body::resolve_class_bodies;
use generic_templates::{
    collect_class_templates, resolve_class_template_semantics, ClassTemplateWorkItem,
};
use hierarchy::build_class_hierarchy;
use interface::{collect_interface_declarations, resolve_interface_claims};
use intrinsic_registry::{intrinsic_for_declaration, validate_intrinsic_declarations};
use resolver::{
    resolve_parameter_binding_mode, resolve_parameters, resolve_result_type, resolved_visibility,
    ProgramResolver,
};
use specialization::{
    discover_specializations, generated_class_work, specialize_bodies, specialize_declarations,
    validate_specialization_requirements, SpecializationBodyInput, SpecializationDeclarationInput,
    SpecializationDiscoveryInput,
};
use static_initializer::{attach_static_field_initializers, resolve_static_field_initializers};
use string_language_item::validate_string_language_item;
use virtuals::resolve_virtual_families;

fn reject_unsupported_generic_interface_application(
    target: &syntax::NamedTypeSyntax,
    diagnostics: &mut Diagnostics,
) -> bool {
    let Some(arguments) = &target.arguments else {
        return false;
    };
    diagnostics.push(
        Diagnostic::error(
            UNSUPPORTED_GENERIC_INTERFACE,
            format!(
                "generic interface application `{}` is not yet supported",
                target.name.text
            ),
        )
        .with_primary_label(
            arguments.span,
            "generic interface syntax is preserved, but semantic resolution is not implemented",
        ),
    );
    true
}

pub(super) fn resolve_singleton(
    ast: &syntax::CompilationUnit,
    source_path: &Path,
) -> ResolveOutput {
    ProgramResolver::singleton(ast, source_path).resolve()
}

pub(super) fn resolve_graph(graph: &ModuleGraph) -> ResolveOutput {
    ProgramResolver::from_graph(graph).resolve()
}
