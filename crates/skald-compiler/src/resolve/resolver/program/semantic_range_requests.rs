//! Diagnostic-isolated semantic discovery of concise range applications.

use std::collections::HashMap;

use crate::resolve::resolver::{
    body::{
        resolve_callable_body, BodyDeclarationEnvironment, BodySpecializationEnvironment,
        CallableResolutionContext, SemanticRangeRequestCollector,
    },
    ClassSymbols, ResolutionMeasurements, ResolvedTypeInterner,
};
use crate::{
    diagnostics::Diagnostics,
    identity::LiteralDataId,
    module::ProgramModuleTable,
    resolve::{
        GenericInterfaceSpecializationTable, GenericSpecializationState,
        GenericSpecializationTable, ResolvedAddressTakenCallableTable,
        ResolvedClassDeclarationTable, ResolvedClassHierarchy, ResolvedClassTemplateSemanticTable,
        ResolvedFunctionDeclarationTable, ResolvedInterfaceDeclarationTable,
        ResolvedIterableLanguageItem, ResolvedOperatorLanguageItem, ResolvedRangeLanguageItem,
    },
    source::Span,
    syntax,
};

use super::{
    class::ClassWorkItem,
    class_body::resolve_class_bodies,
    hierarchy::build_class_hierarchy,
    resolver,
    specialization::{
        self, extend_with_semantic_range_requests, specialize_declarations,
        GenericApplicationDiscovery, SpecializationDeclarationInput, SpecializationDiscoveryInput,
    },
    stages::{BodyResolutionStage, SemanticRangeCompletion, SemanticRangeRequestDelta},
    static_initializer::resolve_static_field_initializers,
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
    pub(super) iterable: Option<&'program ResolvedIterableLanguageItem>,
    pub(super) operators: Option<&'program ResolvedOperatorLanguageItem>,
    pub(super) range: Option<&'program ResolvedRangeLanguageItem>,
}

pub(super) fn complete_semantic_range_specializations(
    input: SemanticRangeCompletionInput<'_, '_>,
    mut discovery: GenericApplicationDiscovery,
    type_interner: &mut ResolvedTypeInterner,
    diagnostics: &mut Diagnostics,
) -> SemanticRangeCompletion {
    let mut measurements = ResolutionMeasurements::default();
    if input.range.is_none() || !program_contains_range(input.units) {
        return SemanticRangeCompletion {
            discovery,
            measurements,
        };
    }

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
    // Materialization rejects the whole generated declaration family on failure.
    // Its reserved identities can still occur in ordinary signatures and bodies,
    // so probing against only the ordinary classes would be an inconsistent view.
    // Authoritative declaration and requirement validation own failure diagnostics.
    if !provisional_specialized.valid {
        return SemanticRangeCompletion {
            discovery,
            measurements,
        };
    }
    let mut semantic_classes = input.classes.clone();
    let mut semantic_symbols = input.class_symbols.to_vec();
    semantic_classes.extend(provisional_specialized.declarations);
    semantic_symbols.extend(provisional_specialized.symbols);
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
        let delta = discover_semantic_range_requests(
            SemanticRangeDiscoveryInput {
                units: input.units,
                modules: input.modules,
                lookups: semantic_lookups,
                functions: input.functions,
                classes: &semantic_classes,
                class_work: input.class_work,
                template_semantics: input.template_semantics,
                class_specializations: &discovery.class_specializations,
                interfaces: input.interfaces,
                hierarchy: &semantic_hierarchy,
                has_module_context: input.has_module_context,
                literal_ids: input.literal_ids,
                iterable: input.iterable,
                operators: input.operators,
                range: input.range,
                interface_specializations: &discovery.interface_specializations,
            },
            &mut semantic_interner,
        );
        measurements.record_semantic_range_round(delta.bodies_revisited);
        let previous_class_count = discovery.class_specializations.iter().len();
        discovery = extend_with_semantic_range_requests(
            input.discovery,
            input.range,
            discovery,
            &delta.requests,
            type_interner,
            diagnostics,
        );
        let class_count = discovery.class_specializations.iter().len();
        if class_count == previous_class_count {
            return SemanticRangeCompletion {
                discovery,
                measurements,
            };
        }
        assert!(
            class_count > previous_class_count,
            "semantic range discovery may only extend specialization work"
        );
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
    pub(super) template_semantics: &'program ResolvedClassTemplateSemanticTable,
    pub(super) class_specializations: &'program GenericSpecializationTable,
    pub(super) interfaces: &'program ResolvedInterfaceDeclarationTable,
    pub(super) hierarchy: &'program ResolvedClassHierarchy,
    pub(super) has_module_context: bool,
    pub(super) literal_ids: &'program HashMap<Span, LiteralDataId>,
    pub(super) iterable: Option<&'program ResolvedIterableLanguageItem>,
    pub(super) operators: Option<&'program ResolvedOperatorLanguageItem>,
    pub(super) range: Option<&'program ResolvedRangeLanguageItem>,
    pub(super) interface_specializations: &'program GenericInterfaceSpecializationTable,
}

pub(super) fn discover_semantic_range_requests(
    input: SemanticRangeDiscoveryInput<'_, '_>,
    type_interner: &mut ResolvedTypeInterner,
) -> SemanticRangeRequestDelta {
    let collector = SemanticRangeRequestCollector::default();
    let mut bodies_revisited = 0usize;
    let mut diagnostics = Diagnostics::new();
    let mut address_taken = ResolvedAddressTakenCallableTable::default();
    let declarations = BodyDeclarationEnvironment::new(
        input.functions,
        input.classes,
        input.interfaces,
        input.hierarchy,
    );
    let body_stage = BodyResolutionStage::new(
        input.has_module_context,
        None,
        input.iterable,
        input.operators,
        input.range,
        input.interface_specializations,
        input.literal_ids,
    );

    for unit in input.units {
        let environment = body_stage
            .environment(input.lookups.for_unit(unit, input.modules), declarations)
            .with_range_request_collector(&collector);

        for work in &unit.function_work {
            let syntax::TopLevelDeclaration::Function(function) =
                &unit.ast.declarations[work.ast_index]
            else {
                continue;
            };
            if !block_contains_range(&function.body) {
                continue;
            }
            bodies_revisited = bodies_revisited.saturating_add(1);
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
                class_contains_range(class)
            })
            .cloned()
            .collect::<Vec<_>>();
        bodies_revisited = bodies_revisited.saturating_add(
            class_work
                .iter()
                .map(class_work_body_count)
                .fold(0usize, usize::saturating_add),
        );
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

    for specialization in input.class_specializations.iter() {
        let GenericSpecializationState::Complete(class_id) = specialization.state else {
            continue;
        };
        let Some((unit, source, ast_index)) =
            specialization::template_source(input.units, specialization.key.template)
        else {
            continue;
        };
        if !class_contains_range(source) {
            continue;
        }
        let Some(declaration) = input.classes.get(class_id) else {
            continue;
        };
        let semantics = input
            .template_semantics
            .get(specialization.key.template)
            .expect("specialization keys reference template semantics");
        let work = specialization::generated_work_item(declaration, source, unit.module, ast_index);
        bodies_revisited = bodies_revisited.saturating_add(class_work_body_count(&work));
        let environment = body_stage
            .environment(input.lookups.for_unit(unit, input.modules), declarations)
            .with_specialization(BodySpecializationEnvironment::new(
                semantics,
                specialization,
            ))
            .with_range_request_collector(&collector);
        let _ = resolve_class_bodies(
            unit.ast,
            std::slice::from_ref(&work),
            input.classes,
            environment,
            type_interner,
            &mut address_taken,
            &mut diagnostics,
        );
    }

    SemanticRangeRequestDelta {
        requests: collector.into_requests(),
        bodies_revisited,
    }
}

fn program_contains_range(units: &[resolver::ModuleUnit<'_>]) -> bool {
    units.iter().any(|unit| {
        unit.ast
            .declarations
            .iter()
            .any(|declaration| match declaration {
                syntax::TopLevelDeclaration::Function(function) => {
                    block_contains_range(&function.body)
                }
                syntax::TopLevelDeclaration::Class(class) => class_contains_range(class),
                syntax::TopLevelDeclaration::ExternalFunction(_)
                | syntax::TopLevelDeclaration::IntrinsicFunction(_)
                | syntax::TopLevelDeclaration::Interface(_) => false,
            })
    })
}

fn class_work_body_count(work: &ClassWorkItem) -> usize {
    work.static_initializer_members
        .len()
        .saturating_add(work.initializer_members.len())
        .saturating_add(usize::from(work.copy_constructor_member.is_some()))
        .saturating_add(usize::from(work.copy_assignment_member.is_some()))
        .saturating_add(usize::from(work.destructor_member.is_some()))
        .saturating_add(work.method_members.len())
}

fn class_contains_range(class: &syntax::ClassDecl) -> bool {
    class.members.iter().any(|member| match member {
        syntax::ClassMember::Initializer(member) => block_contains_range(&member.body),
        syntax::ClassMember::CopyConstructor(member) => block_contains_range(&member.body),
        syntax::ClassMember::CopyAssignment(member) => block_contains_range(&member.body),
        syntax::ClassMember::Destructor(member) => block_contains_range(&member.body),
        syntax::ClassMember::Method(member) => block_contains_range(&member.body),
        syntax::ClassMember::Field(_) | syntax::ClassMember::StaticField(_) => false,
    })
}

fn block_contains_range(block: &syntax::Block) -> bool {
    block.statements.iter().any(|statement| match statement {
        syntax::Statement::ForIn(statement) => {
            matches!(statement.source, syntax::ForInSource::Range(_))
                || block_contains_range(&statement.body)
        }
        syntax::Statement::Conditional(statement) => {
            block_contains_range(&statement.if_arm.body)
                || statement
                    .elif_arms
                    .iter()
                    .any(|arm| block_contains_range(&arm.body))
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_contains_range)
        }
        syntax::Statement::While(statement) => block_contains_range(&statement.body),
        syntax::Statement::Block(block) => block_contains_range(block),
        syntax::Statement::BaseInitialization(_)
        | syntax::Statement::Local(_)
        | syntax::Statement::Return(_)
        | syntax::Statement::Break(_)
        | syntax::Statement::Continue(_)
        | syntax::Statement::Expression(_)
        | syntax::Statement::FieldAssignment(_)
        | syntax::Statement::ObjectAssignment(_) => false,
    })
}
