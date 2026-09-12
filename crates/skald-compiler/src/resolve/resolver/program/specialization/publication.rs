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

impl ClassPublicationProducts {
    fn select_ordinary(
        mut self,
        declarations: ResolvedClassDeclarationTable,
        hierarchy: ResolvedClassHierarchy,
    ) -> Self {
        let generated = self
            .specializations
            .iter()
            .filter_map(GenericSpecialization::class)
            .collect::<Vec<_>>();
        for class in generated {
            self.specializations.fail_class(class);
        }
        Self {
            specializations: self.specializations,
            declarations,
            hierarchy,
        }
    }
}

/// Candidate interface identities and declarations selected as one family.
pub(in crate::resolve::resolver::program) struct InterfacePublicationProducts {
    pub(in crate::resolve::resolver::program) specializations: GenericInterfaceSpecializationTable,
    pub(in crate::resolve::resolver::program) declarations: ResolvedInterfaceDeclarationTable,
}

impl InterfacePublicationProducts {
    fn select_ordinary(mut self, declarations: ResolvedInterfaceDeclarationTable) -> Self {
        self.specializations.fail_all();
        Self {
            specializations: self.specializations,
            declarations,
        }
    }
}

/// Executable products derived from the candidate declaration graph.
pub(in crate::resolve::resolver::program) struct ExecutablePublicationProducts {
    pub(in crate::resolve::resolver::program) functions: ResolvedFunctionDefinitionTable,
    pub(in crate::resolve::resolver::program) virtual_families: ResolvedVirtualFamilyTable,
    pub(in crate::resolve::resolver::program) classes: ResolvedClassDefinitionTable,
}

impl ExecutablePublicationProducts {
    fn empty() -> Self {
        Self {
            functions: ResolvedFunctionDefinitionTable::default(),
            virtual_families: ResolvedVirtualFamilyTable::default(),
            classes: ResolvedClassDefinitionTable::default(),
        }
    }
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
    products: CandidateProgramProducts,
}

/// Immutable candidate view used by publication validators.
///
/// Keeping this private view distinct from the mutation owner makes validator
/// access explicit while withholding authority to alter candidate state.
pub(super) struct PublicationValidationView<'candidate> {
    retained: &'candidate RetainedProgramProducts,
    classes: &'candidate ClassPublicationProducts,
    interfaces: &'candidate InterfacePublicationProducts,
}

impl<'candidate> PublicationValidationView<'candidate> {
    const fn new(products: &'candidate CandidateProgramProducts) -> Self {
        Self {
            retained: &products.retained,
            classes: &products.classes,
            interfaces: &products.interfaces,
        }
    }

    fn class_requirements_are_valid(&self, diagnostics: &mut Diagnostics) -> bool {
        super::validation::validate_specialization_requirements(self, diagnostics)
    }

    fn interface_requirements_are_valid(&self, diagnostics: &mut Diagnostics) -> bool {
        super::interface_validation::validate_interface_specializations(self, diagnostics)
    }

    pub(super) fn modules(&self) -> &ProgramModuleTable {
        &self.retained.modules
    }

    pub(super) fn class_templates(&self) -> &ResolvedClassTemplateTable {
        &self.retained.class_templates
    }

    pub(super) fn interface_templates(&self) -> &ResolvedInterfaceTemplateTable {
        &self.retained.interface_templates
    }

    pub(super) fn type_parameters(&self) -> &ResolvedTypeParameterTable {
        &self.retained.type_parameters
    }

    pub(super) fn function_types(&self) -> &ResolvedFunctionTypeTable {
        &self.retained.function_types
    }

    pub(super) fn operator_language_item(&self) -> Option<&ResolvedOperatorLanguageItem> {
        self.retained.operator_language_item.as_ref()
    }

    pub(super) fn range_language_item(&self) -> Option<&ResolvedRangeLanguageItem> {
        self.retained.range_language_item.as_ref()
    }

    pub(super) fn hierarchy(&self) -> &ResolvedClassHierarchy {
        &self.classes.hierarchy
    }

    pub(super) fn interface(&self, id: InterfaceId) -> Option<&ResolvedInterfaceDeclaration> {
        self.interfaces.declarations.get(id)
    }

    pub(super) fn field(&self, id: FieldId) -> Option<&ResolvedFieldDeclaration> {
        self.classes.declarations.get(id.class())?.field(id)
    }
}

impl crate::type_capabilities::ResolvedCapabilityView for PublicationValidationView<'_> {
    fn classes(&self) -> &ResolvedClassDeclarationTable {
        &self.classes.declarations
    }

    fn generic_specializations(&self) -> &GenericSpecializationTable {
        &self.classes.specializations
    }

    fn generic_interface_specializations(&self) -> &GenericInterfaceSpecializationTable {
        &self.interfaces.specializations
    }

    fn template_semantics(&self) -> &ResolvedClassTemplateSemanticTable {
        &self.retained.template_semantics
    }

    fn interface_template_semantics(&self) -> &ResolvedInterfaceTemplateSemanticTable {
        &self.retained.interface_template_semantics
    }

    fn array_types(&self) -> &ResolvedArrayTypeTable {
        &self.retained.array_types
    }

    fn optional_types(&self) -> &ResolvedOptionalTypeTable {
        &self.retained.optional_types
    }

    fn optional_box_types(&self) -> &ResolvedOptionalBoxTypeTable {
        &self.retained.optional_box_types
    }
}

impl CandidateProgram {
    pub(in crate::resolve::resolver::program) fn new(products: CandidateProgramProducts) -> Self {
        Self { products }
    }

    pub(in crate::resolve::resolver::program) fn validate_and_publish(
        mut self,
        diagnostics: &mut Diagnostics,
        ordinary: OrdinaryProgramProducts,
    ) -> ResolvedProgram {
        let OrdinaryProgramProducts {
            classes,
            interfaces,
            hierarchy,
        } = ordinary;
        if !PublicationValidationView::new(&self.products).class_requirements_are_valid(diagnostics)
        {
            self.products.classes = self.products.classes.select_ordinary(classes, hierarchy);
            self.products.executable = ExecutablePublicationProducts::empty();
        }
        if !PublicationValidationView::new(&self.products)
            .interface_requirements_are_valid(diagnostics)
        {
            self.products.interfaces = self.products.interfaces.select_ordinary(interfaces);
        }
        self.products.into_program()
    }
}
