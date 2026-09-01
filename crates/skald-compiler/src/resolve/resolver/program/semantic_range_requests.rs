//! Diagnostic-isolated semantic discovery of concise range applications.

use super::*;
use crate::{
    identity::LiteralDataId,
    module::ProgramModuleTable,
    resolve::resolver::body::{SemanticRangeRequestCollector, StringLiteralResolutionEnvironment},
};

pub(super) struct SemanticRangeCompletionInput<'program, 'ast> {
    pub(super) units: &'program [resolver::ModuleUnit<'ast>],
    pub(super) modules: &'program ProgramModuleTable,
    pub(super) discovery: SpecializationDiscoveryInput<'program, 'ast>,
    pub(super) template_semantics: &'program ResolvedClassTemplateSemanticTable,
    pub(super) functions: &'program ResolvedFunctionDeclarationTable,
    pub(super) classes: &'program ResolvedClassDeclarationTable,
    pub(super) class_symbols: &'program [ClassSymbols],
    pub(super) class_work: &'program [ClassWorkItem],
    pub(super) interfaces: &'program ResolvedInterfaceDeclarationTable,
    pub(super) has_module_context: bool,
    pub(super) literal_ids: &'program HashMap<Span, LiteralDataId>,
    pub(super) range_source_spans: &'program [Span],
    pub(super) iterable: Option<&'program ResolvedIterableLanguageItem>,
    pub(super) operators: Option<&'program ResolvedOperatorLanguageItem>,
    pub(super) range: Option<&'program ResolvedRangeLanguageItem>,
}

pub(super) fn complete_semantic_range_specializations(
    input: SemanticRangeCompletionInput<'_, '_>,
    mut discovery: GenericApplicationDiscovery,
    type_interner: &mut ResolvedTypeInterner,
    diagnostics: &mut Diagnostics,
) -> GenericApplicationDiscovery {
    let mut semantic_diagnostics = Diagnostics::new();
    let provisional_specialized = specialize_declarations(
        SpecializationDeclarationInput::new(
            input.units,
            input.modules,
            input.template_semantics,
            &discovery.class_specializations,
            input.classes,
            input.interfaces,
            type_interner,
        ),
        &mut semantic_diagnostics,
    );
    let mut semantic_classes = input.classes.clone();
    let mut semantic_symbols = input.class_symbols.to_vec();
    if provisional_specialized.valid {
        semantic_classes.extend(provisional_specialized.declarations);
        semantic_symbols.extend(provisional_specialized.symbols);
    }
    let semantic_hierarchy = build_class_hierarchy(
        &semantic_classes,
        &semantic_symbols,
        &mut semantic_diagnostics,
    );

    let mut semantic_lookups = input.discovery.lookups_with_specializations(
        &discovery.class_specializations,
        &discovery.interface_specializations,
    );
    loop {
        let mut semantic_interner = type_interner.clone();
        let requests = discover_semantic_range_requests(
            SemanticRangeDiscoveryInput {
                units: input.units,
                modules: input.modules,
                lookups: semantic_lookups,
                functions: input.functions,
                classes: &semantic_classes,
                class_work: input.class_work,
                interfaces: input.interfaces,
                hierarchy: &semantic_hierarchy,
                has_module_context: input.has_module_context,
                literal_ids: input.literal_ids,
                range_source_spans: input.range_source_spans,
                iterable: input.iterable,
                operators: input.operators,
                range: input.range,
                interface_specializations: &discovery.interface_specializations,
            },
            &mut semantic_interner,
        );
        let previous_class_count = discovery.class_specializations.iter().len();
        discovery = extend_with_semantic_range_requests(
            input.discovery,
            input.range,
            discovery,
            &requests,
            type_interner,
            diagnostics,
        );
        let has_undiscovered_range = input.range_source_spans.iter().any(|span| {
            !discovery
                .class_specializations
                .iter()
                .any(|specialization| {
                    specialization
                        .provenance
                        .origins
                        .iter()
                        .any(|origin| origin.span == *span)
                })
        });
        if discovery.class_specializations.iter().len() == previous_class_count
            || !has_undiscovered_range
        {
            return discovery;
        }
        semantic_lookups = input.discovery.lookups_with_specializations(
            &discovery.class_specializations,
            &discovery.interface_specializations,
        );
    }
}

pub(super) struct SemanticRangeDiscoveryInput<'program, 'ast> {
    pub(super) units: &'program [resolver::ModuleUnit<'ast>],
    pub(super) modules: &'program ProgramModuleTable,
    pub(super) lookups: resolver::ProgramLookupTables<'program>,
    pub(super) functions: &'program ResolvedFunctionDeclarationTable,
    pub(super) classes: &'program ResolvedClassDeclarationTable,
    pub(super) class_work: &'program [ClassWorkItem],
    pub(super) interfaces: &'program ResolvedInterfaceDeclarationTable,
    pub(super) hierarchy: &'program ResolvedClassHierarchy,
    pub(super) has_module_context: bool,
    pub(super) literal_ids: &'program HashMap<Span, LiteralDataId>,
    pub(super) range_source_spans: &'program [Span],
    pub(super) iterable: Option<&'program ResolvedIterableLanguageItem>,
    pub(super) operators: Option<&'program ResolvedOperatorLanguageItem>,
    pub(super) range: Option<&'program ResolvedRangeLanguageItem>,
    pub(super) interface_specializations: &'program GenericInterfaceSpecializationTable,
}

pub(super) fn discover_semantic_range_requests(
    input: SemanticRangeDiscoveryInput<'_, '_>,
    type_interner: &mut ResolvedTypeInterner,
) -> Vec<crate::resolve::resolver::body::SemanticRangeRequest> {
    let collector = SemanticRangeRequestCollector::default();
    let mut diagnostics = Diagnostics::new();
    let mut address_taken = ResolvedAddressTakenCallableTable::default();
    let declarations = BodyDeclarationEnvironment::new(
        input.functions,
        input.classes,
        input.interfaces,
        input.hierarchy,
    );

    for unit in input.units {
        let environment = BodyResolutionEnvironment::new(
            input.lookups.for_unit(unit, input.modules),
            declarations,
            input.has_module_context,
            BodyLanguageItemEnvironment::new(
                StringLiteralResolutionEnvironment::new(None, input.literal_ids),
                input.iterable.map(|item| {
                    IterationResolutionEnvironment::new(item, input.interface_specializations)
                }),
                input.operators.map(|item| {
                    OperatorResolutionEnvironment::new(item, input.interface_specializations)
                }),
                input.range.map(|item| {
                    RangeResolutionEnvironment::new(item, input.interface_specializations)
                }),
            ),
        )
        .with_range_request_collector(&collector);

        for work in &unit.function_work {
            let syntax::TopLevelDeclaration::Function(function) =
                &unit.ast.declarations[work.ast_index]
            else {
                continue;
            };
            if !contains_range(function.span, input.range_source_spans) {
                continue;
            }
            let declaration = input
                .functions
                .get(work.id)
                .expect("function work retains declaration metadata");
            let _ = resolve_callable_body(
                CallableResolutionContext::function(work.id.into()),
                &declaration.parameters,
                &function.body,
                environment,
                type_interner,
                &mut address_taken,
                &mut diagnostics,
            );
        }

        let class_work = input
            .class_work
            .iter()
            .filter(|work| {
                if work.module != unit.module {
                    return false;
                }
                let syntax::TopLevelDeclaration::Class(class) =
                    &unit.ast.declarations[work.ast_index]
                else {
                    return false;
                };
                contains_range(class.span, input.range_source_spans)
            })
            .cloned()
            .collect::<Vec<_>>();
        let _ = resolve_static_field_initializers(
            unit.ast,
            &class_work,
            input.classes,
            environment,
            type_interner,
            &mut address_taken,
            &mut diagnostics,
        );
        let _ = resolve_class_bodies(
            unit.ast,
            &class_work,
            input.classes,
            environment,
            type_interner,
            &mut address_taken,
            &mut diagnostics,
        );
    }

    collector.into_requests()
}

fn contains_range(container: Span, ranges: &[Span]) -> bool {
    ranges.iter().any(|range| {
        range.source_id() == container.source_id()
            && container.range().start() <= range.range().start()
            && range.range().end() <= container.range().end()
    })
}
