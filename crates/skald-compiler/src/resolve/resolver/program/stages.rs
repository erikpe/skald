//! Named products passed between the resolver's orchestration stages.

use super::super::body::StringLiteralResolutionEnvironment;
use super::*;
use crate::identity::LiteralDataId;

pub(super) struct CollectedDeclarations {
    pub(super) module_declarations: ResolvedModuleDeclarationTable,
    pub(super) module_bindings: ResolvedModuleBindingTable,
    pub(super) ordinary_bindings: ResolvedOrdinaryBindingTable,
    pub(super) module_spans: Vec<Span>,
    pub(super) class_templates: ResolvedClassTemplateTable,
    pub(super) interface_templates: ResolvedInterfaceTemplateTable,
    pub(super) type_parameters: ResolvedTypeParameterTable,
}

/// Stable declaration and language-item inputs shared by every authoritative
/// body-resolution pass.
#[derive(Clone, Copy)]
pub(super) struct BodyResolutionStage<'program> {
    has_module_context: bool,
    string: Option<&'program ResolvedStringLanguageItem>,
    iterable: Option<&'program ResolvedIterableLanguageItem>,
    operators: Option<&'program ResolvedOperatorLanguageItem>,
    range: Option<&'program ResolvedRangeLanguageItem>,
    interface_specializations: &'program GenericInterfaceSpecializationTable,
}

impl<'program> BodyResolutionStage<'program> {
    pub(super) const fn new(
        has_module_context: bool,
        string: Option<&'program ResolvedStringLanguageItem>,
        iterable: Option<&'program ResolvedIterableLanguageItem>,
        operators: Option<&'program ResolvedOperatorLanguageItem>,
        range: Option<&'program ResolvedRangeLanguageItem>,
        interface_specializations: &'program GenericInterfaceSpecializationTable,
    ) -> Self {
        Self {
            has_module_context,
            string,
            iterable,
            operators,
            range,
            interface_specializations,
        }
    }

    pub(super) fn language_items(
        self,
        literal_ids: &'program HashMap<Span, LiteralDataId>,
    ) -> BodyLanguageItemEnvironment<'program> {
        BodyLanguageItemEnvironment::new(
            StringLiteralResolutionEnvironment::new(self.string, literal_ids),
            self.iterable.map(|item| {
                IterationResolutionEnvironment::new(item, self.interface_specializations)
            }),
            self.operators.map(|item| {
                OperatorResolutionEnvironment::new(item, self.interface_specializations)
            }),
            self.range
                .map(|item| RangeResolutionEnvironment::new(item, self.interface_specializations)),
        )
    }

    pub(super) fn environment(
        self,
        lookup: ModuleLookup<'program>,
        declarations: BodyDeclarationEnvironment<'program>,
        literal_ids: &'program HashMap<Span, LiteralDataId>,
    ) -> BodyResolutionEnvironment<'program> {
        BodyResolutionEnvironment::new(
            lookup,
            declarations,
            self.has_module_context,
            self.language_items(literal_ids),
        )
    }
}

/// Completed ordinary and generated function and class bodies awaiting publication.
pub(super) struct ResolvedBodies {
    pub(super) functions: ResolvedFunctionDefinitionTable,
    pub(super) classes: ResolvedClassDefinitionTable,
}

impl ResolvedBodies {
    pub(super) fn new(
        functions: Vec<Option<ResolvedFunctionDefinition>>,
        classes: Vec<ResolvedClassDefinition>,
    ) -> Self {
        Self {
            functions: ResolvedFunctionDefinitionTable::new(functions),
            classes: ResolvedClassDefinitionTable::new(classes),
        }
    }
}
