//! Closed generic-class request discovery and deterministic identity ownership.

mod bodies;
mod bound_members;
mod closed_types;
mod coordinator;
mod declarations;
mod interface_declarations;
mod interface_validation;
mod names;
mod requests;
mod validation;

pub(super) use bodies::generated_class_work;
pub(super) use bound_members::close_bound_member_selections;
pub(super) use declarations::{specialize_declarations, SpecializationDeclarationInput};
pub(super) use interface_declarations::{
    materialize_interface_declarations, InterfaceMaterializationInput,
};
pub(super) use interface_validation::validate_interface_specializations;
pub(super) use requests::{
    discover_specializations, GenericTemplateDiscoveryInput, SpecializationDiscoveryInput,
};
pub(super) use validation::validate_specialization_requirements;

#[cfg(test)]
mod body_tests;
#[cfg(test)]
mod declaration_tests;
#[cfg(test)]
mod function_values_tests;
#[cfg(test)]
mod interface_declaration_tests;
#[cfg(test)]
mod tests;

use super::*;
use crate::module::ProgramModuleTable;
use crate::resolve::resolver::body::StringLiteralResolutionEnvironment;
use coordinator::SpecializationCoordinator;

pub(super) struct SpecializationBodyInput<'program, 'ast> {
    pub(super) units: &'program [resolver::ModuleUnit<'ast>],
    pub(super) modules: &'program ProgramModuleTable,
    pub(super) lookups: resolver::ProgramLookupTables<'program>,
    pub(super) semantics: &'program ResolvedClassTemplateSemanticTable,
    pub(super) specializations: &'program GenericSpecializationTable,
    pub(super) functions: &'program ResolvedFunctionDeclarationTable,
    pub(super) classes: &'program ResolvedClassDeclarationTable,
    pub(super) interfaces: &'program ResolvedInterfaceDeclarationTable,
    pub(super) hierarchy: &'program ResolvedClassHierarchy,
    pub(super) has_module_context: bool,
    pub(super) string_literals: StringLiteralResolutionEnvironment<'program>,
}

pub(super) struct SpecializedBodies {
    pub(super) definitions: Vec<ResolvedClassDefinition>,
    pub(super) static_initializers: Vec<static_initializer::ResolvedStaticInitializerUpdate>,
    pub(super) valid: bool,
}

pub(super) fn specialize_bodies(
    input: SpecializationBodyInput<'_, '_>,
    type_interner: &mut ResolvedTypeInterner,
    address_taken_callables: &mut ResolvedAddressTakenCallableTable,
    diagnostics: &mut Diagnostics,
) -> SpecializedBodies {
    bodies::specialize_bodies(input, type_interner, address_taken_callables, diagnostics)
}

fn template_source<'unit, 'ast>(
    units: &'unit [resolver::ModuleUnit<'ast>],
    template: ClassTemplateId,
) -> Option<(
    &'unit resolver::ModuleUnit<'ast>,
    &'ast syntax::ClassDecl,
    usize,
)> {
    units.iter().find_map(|unit| {
        unit.template_work.iter().find_map(|work| {
            (work.id == template).then(|| {
                let syntax::TopLevelDeclaration::Class(class) =
                    &unit.ast.declarations[work.ast_index]
                else {
                    unreachable!("template work references a class declaration")
                };
                (unit, class, work.ast_index)
            })
        })
    })
}

fn class_source<'unit, 'ast>(
    units: &'unit [resolver::ModuleUnit<'ast>],
    class: ClassId,
) -> Option<(&'unit resolver::ModuleUnit<'ast>, &'ast syntax::ClassDecl)> {
    units.iter().find_map(|unit| {
        unit.class_work.iter().find_map(|(id, ast_index)| {
            (*id == class).then(|| {
                let syntax::TopLevelDeclaration::Class(class) = &unit.ast.declarations[*ast_index]
                else {
                    unreachable!("class work references a class declaration")
                };
                (unit, class)
            })
        })
    })
}
