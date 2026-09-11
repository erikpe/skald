//! Named products passed between the resolver's orchestration stages.

use super::super::body::StringLiteralResolutionEnvironment;
use super::*;
use crate::identity::LiteralDataId;
use crate::resolve::resolver::{body::SemanticRangeRequest, ResolutionMeasurements};

pub(super) struct CollectedDeclarations {
    pub(super) module_declarations: ResolvedModuleDeclarationTable,
    pub(super) module_bindings: ResolvedModuleBindingTable,
    pub(super) ordinary_bindings: ResolvedOrdinaryBindingTable,
    pub(super) module_spans: Vec<Span>,
    pub(super) class_templates: ResolvedClassTemplateTable,
    pub(super) interface_templates: ResolvedInterfaceTemplateTable,
    pub(super) type_parameters: ResolvedTypeParameterTable,
}

/// Newly observed exact range applications from one isolated semantic probe.
pub(super) struct SemanticRangeRequestDelta {
    pub(super) requests: Vec<SemanticRangeRequest>,
    pub(super) bodies_revisited: usize,
}

/// Fixed-point specialization state and the work used to reach it.
pub(super) struct SemanticRangeCompletion {
    pub(super) discovery: GenericApplicationDiscovery,
    pub(super) measurements: ResolutionMeasurements,
}

/// Stable declaration and language-item inputs shared by every authoritative
/// body-resolution pass.
#[derive(Clone, Copy)]
pub(super) struct BodyResolutionStage<'program> {
    has_module_context: bool,
    language_items: BodyLanguageItemEnvironment<'program>,
}

impl<'program> BodyResolutionStage<'program> {
    pub(super) fn new(
        has_module_context: bool,
        string: Option<&'program ResolvedStringLanguageItem>,
        iterable: Option<&'program ResolvedIterableLanguageItem>,
        operators: Option<&'program ResolvedOperatorLanguageItem>,
        range: Option<&'program ResolvedRangeLanguageItem>,
        interface_specializations: &'program GenericInterfaceSpecializationTable,
        literal_ids: &'program HashMap<Span, LiteralDataId>,
    ) -> Self {
        Self {
            has_module_context,
            language_items: BodyLanguageItemEnvironment::new(
                StringLiteralResolutionEnvironment::new(string, literal_ids),
                iterable.map(|item| {
                    IterationResolutionEnvironment::new(item, interface_specializations)
                }),
                operators.map(|item| {
                    OperatorResolutionEnvironment::new(item, interface_specializations)
                }),
                range.map(|item| RangeResolutionEnvironment::new(item, interface_specializations)),
            ),
        }
    }

    pub(super) const fn language_items(self) -> BodyLanguageItemEnvironment<'program> {
        self.language_items
    }

    pub(super) fn environment(
        self,
        lookup: ModuleLookup<'program>,
        declarations: BodyDeclarationEnvironment<'program>,
    ) -> BodyResolutionEnvironment<'program> {
        BodyResolutionEnvironment::new(
            lookup,
            declarations,
            self.has_module_context,
            self.language_items,
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
