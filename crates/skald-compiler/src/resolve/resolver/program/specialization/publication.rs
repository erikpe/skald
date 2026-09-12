//! Ordered selection between validated specialization candidates and ordinary products.

use super::*;
use crate::external::ExternalLinkTable;

/// Request, declaration, type, and language-item evidence retained by every
/// publication outcome, including diagnostic-only error output.
pub(in crate::resolve::resolver::program) struct RetainedProgramProducts {
    pub(in crate::resolve::resolver::program) modules: ProgramModuleTable,
    pub(in crate::resolve::resolver::program) external_links: ExternalLinkTable,
    pub(in crate::resolve::resolver::program) module_bindings: ResolvedModuleBindingTable,
    pub(in crate::resolve::resolver::program) ordinary_bindings: ResolvedOrdinaryBindingTable,
    pub(in crate::resolve::resolver::program) module_declarations: ResolvedModuleDeclarationTable,
    pub(in crate::resolve::resolver::program) class_templates: ResolvedClassTemplateTable,
    pub(in crate::resolve::resolver::program) interface_templates: ResolvedInterfaceTemplateTable,
    pub(in crate::resolve::resolver::program) interface_template_semantics:
        ResolvedInterfaceTemplateSemanticTable,
    pub(in crate::resolve::resolver::program) type_parameters: ResolvedTypeParameterTable,
    pub(in crate::resolve::resolver::program) template_semantics:
        ResolvedClassTemplateSemanticTable,
    pub(in crate::resolve::resolver::program) function_types: ResolvedFunctionTypeTable,
    pub(in crate::resolve::resolver::program) address_taken_callables:
        ResolvedAddressTakenCallableTable,
    pub(in crate::resolve::resolver::program) array_types: ResolvedArrayTypeTable,
    pub(in crate::resolve::resolver::program) optional_types: ResolvedOptionalTypeTable,
    pub(in crate::resolve::resolver::program) optional_box_types: ResolvedOptionalBoxTypeTable,
    pub(in crate::resolve::resolver::program) iterable_language_item:
        Option<ResolvedIterableLanguageItem>,
    pub(in crate::resolve::resolver::program) operator_language_item:
        Option<ResolvedOperatorLanguageItem>,
    pub(in crate::resolve::resolver::program) range_language_item:
        Option<ResolvedRangeLanguageItem>,
    pub(in crate::resolve::resolver::program) string_language_item:
        Option<ResolvedStringLanguageItem>,
    pub(in crate::resolve::resolver::program) literal_data: ResolvedLiteralDataTable,
    pub(in crate::resolve::resolver::program) declarations: ResolvedFunctionDeclarationTable,
    pub(in crate::resolve::resolver::program) entry_function: Option<FunctionId>,
    pub(in crate::resolve::resolver::program) span: Span,
}

/// Candidate class identities, declarations, and their derived hierarchy.
pub(in crate::resolve::resolver::program) struct ClassPublicationProducts {
    pub(in crate::resolve::resolver::program) specializations: GenericSpecializationTable,
    pub(in crate::resolve::resolver::program) declarations: ResolvedClassDeclarationTable,
    pub(in crate::resolve::resolver::program) hierarchy: ResolvedClassHierarchy,
}

/// Candidate interface identities and declarations selected as one family.
pub(in crate::resolve::resolver::program) struct InterfacePublicationProducts {
    pub(in crate::resolve::resolver::program) specializations: GenericInterfaceSpecializationTable,
    pub(in crate::resolve::resolver::program) declarations: ResolvedInterfaceDeclarationTable,
}

/// Executable products derived from the candidate declaration graph.
pub(in crate::resolve::resolver::program) struct ExecutablePublicationProducts {
    pub(in crate::resolve::resolver::program) functions: ResolvedFunctionDefinitionTable,
    pub(in crate::resolve::resolver::program) virtual_families: ResolvedVirtualFamilyTable,
    pub(in crate::resolve::resolver::program) classes: ResolvedClassDefinitionTable,
}

/// Complete candidate inputs grouped by their publication disposition.
pub(in crate::resolve::resolver::program) struct CandidateProgramProducts {
    pub(in crate::resolve::resolver::program) retained: RetainedProgramProducts,
    pub(in crate::resolve::resolver::program) classes: ClassPublicationProducts,
    pub(in crate::resolve::resolver::program) interfaces: InterfacePublicationProducts,
    pub(in crate::resolve::resolver::program) executable: ExecutablePublicationProducts,
}

impl CandidateProgramProducts {
    fn into_program(self) -> ResolvedProgram {
        let RetainedProgramProducts {
            modules,
            external_links,
            module_bindings,
            ordinary_bindings,
            module_declarations,
            class_templates,
            interface_templates,
            interface_template_semantics,
            type_parameters,
            template_semantics,
            function_types,
            address_taken_callables,
            array_types,
            optional_types,
            optional_box_types,
            iterable_language_item,
            operator_language_item,
            range_language_item,
            string_language_item,
            literal_data,
            declarations,
            entry_function,
            span,
        } = self.retained;
        let ClassPublicationProducts {
            specializations: generic_specializations,
            declarations: classes,
            hierarchy,
        } = self.classes;
        let InterfacePublicationProducts {
            specializations: generic_interface_specializations,
            declarations: interfaces,
        } = self.interfaces;
        let ExecutablePublicationProducts {
            functions: definitions,
            virtual_families,
            classes: class_definitions,
        } = self.executable;
        ResolvedProgram {
            modules,
            external_links,
            module_bindings,
            ordinary_bindings,
            module_declarations,
            class_templates,
            interface_templates,
            interface_template_semantics,
            type_parameters,
            template_semantics,
            generic_specializations,
            generic_interface_specializations,
            function_types,
            address_taken_callables,
            array_types,
            optional_types,
            optional_box_types,
            iterable_language_item,
            operator_language_item,
            range_language_item,
            string_language_item,
            literal_data,
            declarations,
            definitions,
            classes,
            interfaces,
            hierarchy,
            virtual_families,
            class_definitions,
            entry_function,
            span,
        }
    }
}

pub(in crate::resolve::resolver::program) struct OrdinaryProgramProducts {
    classes: ResolvedClassDeclarationTable,
    interfaces: ResolvedInterfaceDeclarationTable,
    hierarchy: ResolvedClassHierarchy,
}

impl OrdinaryProgramProducts {
    pub(in crate::resolve::resolver::program) fn new(
        classes: ResolvedClassDeclarationTable,
        interfaces: ResolvedInterfaceDeclarationTable,
        hierarchy: ResolvedClassHierarchy,
    ) -> Self {
        Self {
            classes,
            interfaces,
            hierarchy,
        }
    }
}

pub(in crate::resolve::resolver::program) struct CandidateProgram {
    program: ResolvedProgram,
}

/// Immutable candidate view used by publication validators.
///
/// Keeping this private view distinct from the mutation owner makes validator
/// access explicit. P02 can change product selection without giving validators
/// authority to alter candidate state.
struct PublicationValidationView<'candidate> {
    program: &'candidate ResolvedProgram,
}

impl<'candidate> PublicationValidationView<'candidate> {
    const fn new(program: &'candidate ResolvedProgram) -> Self {
        Self { program }
    }

    fn class_requirements_are_valid(self, diagnostics: &mut Diagnostics) -> bool {
        super::validation::validate_specialization_requirements(self.program, diagnostics)
    }

    fn interface_requirements_are_valid(self, diagnostics: &mut Diagnostics) -> bool {
        super::interface_validation::validate_interface_specializations(self.program, diagnostics)
    }
}

impl CandidateProgram {
    pub(in crate::resolve::resolver::program) fn new(products: CandidateProgramProducts) -> Self {
        Self {
            program: products.into_program(),
        }
    }

    pub(in crate::resolve::resolver::program) fn validate_and_publish(
        mut self,
        diagnostics: &mut Diagnostics,
        ordinary: OrdinaryProgramProducts,
    ) -> ResolvedProgram {
        if !PublicationValidationView::new(&self.program).class_requirements_are_valid(diagnostics)
        {
            self.reject_class_candidates(&ordinary);
        }
        if !PublicationValidationView::new(&self.program)
            .interface_requirements_are_valid(diagnostics)
        {
            self.reject_interface_candidates(&ordinary);
        }
        self.program
    }

    fn reject_class_candidates(&mut self, ordinary: &OrdinaryProgramProducts) {
        let generated = self
            .program
            .generic_specializations
            .iter()
            .filter_map(GenericSpecialization::class)
            .collect::<Vec<_>>();
        for class in generated {
            self.program.generic_specializations.fail_class(class);
        }
        self.program.classes = ordinary.classes.clone();
        self.program.hierarchy = ordinary.hierarchy.clone();
        self.program.class_definitions = ResolvedClassDefinitionTable::default();
        self.program.definitions = ResolvedFunctionDefinitionTable::default();
        self.program.virtual_families = ResolvedVirtualFamilyTable::default();
    }

    fn reject_interface_candidates(&mut self, ordinary: &OrdinaryProgramProducts) {
        self.program.generic_interface_specializations.fail_all();
        self.program.interfaces = ordinary.interfaces.clone();
    }
}
