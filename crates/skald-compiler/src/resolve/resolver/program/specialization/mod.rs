//! Closed generic-class request discovery and deterministic identity ownership.

mod bodies;
mod bound_members;
mod closed_types;
mod coordinator;
mod declarations;
mod interface_declarations;
mod interface_validation;
mod names;
mod publication;
mod requests;
mod validation;

pub(super) use bodies::{generated_class_work, generated_work_item};
pub(super) use bound_members::close_bound_member_selections;
pub(super) use declarations::{specialize_declarations, SpecializationDeclarationInput};
pub(super) use interface_declarations::{
    materialize_interface_declarations, InterfaceMaterializationInput,
};
pub(super) use publication::{
    CandidateProgram, CandidateProgramProducts, ClassPublicationProducts,
    ExecutablePublicationProducts, InterfacePublicationProducts, OrdinaryProgramProducts,
    RetainedProgramProducts,
};
pub(super) use requests::{
    discover_specializations, extend_with_semantic_range_requests, GenericApplicationDiscovery,
    GenericTemplateDiscoveryInput, SpecializationDiscoveryInput,
};

#[cfg(test)]
mod body_tests;
#[cfg(test)]
mod declaration_tests;
#[cfg(test)]
mod function_values_tests;
#[cfg(test)]
mod interface_declaration_tests;
#[cfg(test)]
mod publication_tests;
#[cfg(test)]
mod tests;

use crate::resolve::{
    ir::{
        canonical_operator_application, canonical_successor_application,
        primitive_operator_evidence, primitive_operator_operation, primitive_successor_evidence,
        primitive_successor_operation, ClosedGenericBoundMember, ClosedGenericIterationSelection,
        ClosedGenericOperatorSelection, ClosedGenericRequirementSubject, GenericApplicationOrigin,
        GenericClassInstanceKey, GenericSpecialization, GenericSpecializationKey,
        GenericSpecializationProvenance, GenericSpecializationTransition,
        ResolvedClassTemplateSemantics, ResolvedPrimitiveBoundOperation,
        ResolvedTemplateBoundRequirement, ResolvedTemplateSelection,
        ResolvedTemplateTypeUseContext, ResolvedTypeNameContext, ResolvedTypeNameRenderer,
    },
    resolver::{
        body::{
            BodyDeclarationEnvironment, BodyLanguageItemEnvironment, BodyResolutionEnvironment,
            BodySpecializationEnvironment,
        },
        name_lookup::{ModuleLookup, TopLevelLookup},
        ClassSymbols, OrdinaryMemberSymbol, OrdinaryMemberSymbolKind, ResolvedTypeInterner,
        TopLevelSymbol, TopLevelSymbolKind, DUPLICATE_GENERIC_BOUND,
        INVALID_GENERIC_INTERFACE_REQUIREMENT, NON_TERMINATING_GENERIC_SPECIALIZATION,
        UNSATISFIED_GENERIC_REQUIREMENT,
    },
};
use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    identity::{
        CallableId, ClassId, ClassTemplateId, CopyAssignmentId, CopyConstructorId, DestructorId,
        FieldId, FunctionId, InitializerId, InterfaceId, InterfaceRequirementId,
        InterfaceTemplateId, MethodId, ModuleId, ParameterId, StaticFieldId,
    },
    module::ProgramModuleTable,
    resolve::{
        CanonicalOperatorProtocol, CanonicalOperatorProtocolShape, GenericCapability,
        GenericInterfaceApplicationOrigin, GenericInterfaceInstanceKey,
        GenericInterfaceRequirementMapping, GenericInterfaceSpecialization,
        GenericInterfaceSpecializationProvenance, GenericInterfaceSpecializationState,
        GenericInterfaceSpecializationTable, GenericInterfaceSpecializationTransition,
        GenericRequirementReason, GenericSpecializationState, GenericSpecializationTable,
        ResolvedAddressTakenCallableTable, ResolvedArrayType, ResolvedArrayTypeTable,
        ResolvedClassDeclaration, ResolvedClassDeclarationTable, ResolvedClassDefinition,
        ResolvedClassDefinitionTable, ResolvedClassHierarchy, ResolvedClassTemplateSemanticTable,
        ResolvedClassTemplateTable, ResolvedCopyAssignmentDeclaration,
        ResolvedCopyConstructorDeclaration, ResolvedCopyOperation, ResolvedDestructorDeclaration,
        ResolvedDirectBase, ResolvedFieldDeclaration, ResolvedFunctionDeclarationTable,
        ResolvedFunctionDefinitionTable, ResolvedFunctionType, ResolvedFunctionTypeParameter,
        ResolvedFunctionTypeParameterMode, ResolvedFunctionTypeTable,
        ResolvedInitializerDeclaration, ResolvedInterfaceClaim, ResolvedInterfaceDeclaration,
        ResolvedInterfaceDeclarationTable, ResolvedInterfaceParameter,
        ResolvedInterfaceRequirement, ResolvedInterfaceTemplate,
        ResolvedInterfaceTemplateSemanticTable, ResolvedInterfaceTemplateSemantics,
        ResolvedInterfaceTemplateTable, ResolvedInterfaceTemplateTypeUseContext,
        ResolvedInterfaceType, ResolvedIterableLanguageItem, ResolvedLiteralDataTable,
        ResolvedMemberVisibility, ResolvedMethodDeclaration, ResolvedMethodDispatch,
        ResolvedMethodKind, ResolvedMethodModifier, ResolvedModuleBindingTable,
        ResolvedModuleDeclarationTable, ResolvedObjectTarget, ResolvedOperatorLanguageItem,
        ResolvedOptionalBoxType, ResolvedOptionalBoxTypeTable, ResolvedOptionalType,
        ResolvedOptionalTypeTable, ResolvedOrdinaryBindingTable, ResolvedParameter,
        ResolvedParameterBindingMode, ResolvedProgram, ResolvedRangeLanguageItem,
        ResolvedReceiverAccess, ResolvedSharedTarget, ResolvedStaticFieldDeclaration,
        ResolvedStringLanguageItem, ResolvedTemplateType, ResolvedTemplateTypeKind, ResolvedType,
        ResolvedTypeKind, ResolvedTypeParameterTable, ResolvedVirtualFamilyTable,
    },
    source::Span,
    syntax,
};

use super::{
    class_body::resolve_class_bodies,
    resolver::{self, resolve_parameter_binding_mode, resolved_visibility},
    static_initializer::{self, resolve_static_field_initializers},
};
use coordinator::SpecializationCoordinator;

#[cfg(test)]
use crate::resolve::ir::ResolvedTemplateDependentSelectionKind;
#[cfg(test)]
use crate::resolve::{GenericAliasAccess, ResolvedTopLevelId};

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
    pub(super) language_items: BodyLanguageItemEnvironment<'program>,
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

pub(super) fn template_source<'unit, 'ast>(
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
