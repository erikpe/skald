//! Source evidence for compiler-known language-item validation.

use super::resolver::ModuleUnit;
use crate::{
    module::{CanonicalModule, CompilerDependencyKind, ModuleGraph, ProgramModuleTable},
    source::Span,
    syntax,
};

#[derive(Default)]
pub(super) struct LanguageItemRequirementOrigins {
    pub(super) string_literals: Vec<Span>,
    pub(super) iterable: Vec<Span>,
    pub(super) operators: Vec<Span>,
    pub(super) range: Vec<Span>,
}

impl LanguageItemRequirementOrigins {
    pub(super) fn collect(graph: &ModuleGraph) -> Self {
        Self {
            string_literals: compiler_dependency_spans(
                graph,
                CompilerDependencyKind::StringLiteral,
            ),
            iterable: requirement_spans(
                graph,
                CanonicalModule::Iteration,
                Some(CompilerDependencyKind::GeneralIteration),
            ),
            operators: requirement_spans(graph, CanonicalModule::Operators, None),
            range: requirement_spans(
                graph,
                CanonicalModule::Range,
                Some(CompilerDependencyKind::RangeForSource),
            ),
        }
    }
}

pub(super) struct LanguageItemDeclarationOrigins<'ast> {
    pub(super) iterable: Vec<Span>,
    pub(super) operators: Vec<(String, Span)>,
    pub(super) successor: Vec<Span>,
    pub(super) range: Vec<&'ast syntax::TopLevelDeclaration>,
}

impl<'ast> LanguageItemDeclarationOrigins<'ast> {
    pub(super) fn collect(units: &[ModuleUnit<'ast>], modules: &ProgramModuleTable) -> Self {
        let iterable = canonical_declarations(units, modules, CanonicalModule::Iteration)
            .into_iter()
            .filter(|declaration| declaration.name().text == "Iterable")
            .map(syntax::TopLevelDeclaration::span)
            .collect();
        let operators = canonical_declarations(units, modules, CanonicalModule::Operators)
            .into_iter()
            .map(|declaration| {
                (
                    declaration.name().text.to_string(),
                    syntax::TopLevelDeclaration::span(declaration),
                )
            })
            .collect();
        let range_declarations = canonical_declarations(units, modules, CanonicalModule::Range);
        let successor = range_declarations
            .iter()
            .filter(|declaration| declaration.name().text == "Successor")
            .map(|declaration| syntax::TopLevelDeclaration::span(declaration))
            .collect();
        let range = range_declarations
            .into_iter()
            .filter(|declaration| declaration.name().text == "Range")
            .collect();
        Self {
            iterable,
            operators,
            successor,
            range,
        }
    }
}

fn requirement_spans(
    graph: &ModuleGraph,
    canonical: CanonicalModule,
    compiler_dependency: Option<CompilerDependencyKind>,
) -> Vec<Span> {
    let path = canonical.path();
    let Some(target) = graph
        .find(&path)
        .map(|module| module.provenance().module_id())
    else {
        return Vec::new();
    };
    let mut spans = graph
        .modules()
        .iter()
        .flat_map(|module| {
            module
                .imports()
                .iter()
                .filter(move |edge| edge.target() == target)
                .flat_map(|edge| {
                    edge.import_spans().iter().copied().chain(
                        compiler_dependency
                            .into_iter()
                            .flat_map(|kind| edge.compiler_dependency_spans(kind).iter().copied()),
                    )
                })
        })
        .collect::<Vec<_>>();
    if spans.is_empty() && graph.entry() == target {
        spans.push(
            graph
                .module(target)
                .expect("selected canonical module must be loaded")
                .ast()
                .span,
        );
    }
    spans
}

fn compiler_dependency_spans(graph: &ModuleGraph, dependency: CompilerDependencyKind) -> Vec<Span> {
    let path = dependency.canonical_module().path();
    let Some(target) = graph
        .find(&path)
        .map(|module| module.provenance().module_id())
    else {
        return Vec::new();
    };
    graph
        .modules()
        .iter()
        .flat_map(|module| {
            module
                .imports()
                .iter()
                .filter(move |edge| edge.target() == target)
                .flat_map(|edge| edge.compiler_dependency_spans(dependency).iter().copied())
        })
        .collect()
}

fn canonical_declarations<'ast>(
    units: &[ModuleUnit<'ast>],
    modules: &ProgramModuleTable,
    canonical: CanonicalModule,
) -> Vec<&'ast syntax::TopLevelDeclaration> {
    let Some(module) = modules
        .find(&canonical.path())
        .map(|entry| entry.module_id())
    else {
        return Vec::new();
    };
    units
        .iter()
        .find(|unit| unit.module == module)
        .expect("every program module has one resolver unit")
        .ast
        .declarations
        .iter()
        .collect()
}
