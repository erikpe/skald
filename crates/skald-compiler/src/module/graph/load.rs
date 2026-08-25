use std::{collections::BTreeMap, fs, path::Path};

use crate::{
    diagnostics::Diagnostics,
    driver::EntrySelector,
    identity::ModuleId,
    lexer::{lex, TokenKind},
    source::{SourceDatabase, SourceId, Span, TextRange},
    syntax::{parse, CompilationUnit, ImportDeclaration},
};

use super::{
    diagnostic::{
        append_pending_diagnostics, entry_failure, self_import_diagnostic, PendingLoadError,
    },
    entry::{select_entry, LoaderProviders},
    model::{
        CompilerDependencyEvidence, CompilerDependencyKind, LoadedModule, ModuleGraph,
        ModuleGraphLoadFailure, ModuleImportEdge,
    },
};
use crate::module::{
    ModuleCandidate, ModulePath, ModuleProvenance, ModuleSourceLocation, ProviderSet,
};

/// Selects an entry and loads exactly its reachable parsed module graph.
///
/// Provider normalization is intentionally a separate operation. This
/// boundary receives all process-dependent filesystem context explicitly and
/// performs no semantic declaration or import-binding resolution.
pub fn load_module_graph(
    entry: &EntrySelector,
    working_directory: &Path,
    providers: &ProviderSet,
) -> Result<ModuleGraph, ModuleGraphLoadFailure> {
    let selected = match select_entry(entry, working_directory, providers) {
        Ok(selected) => selected,
        Err(error) => return Err(entry_failure(error)),
    };
    let entry_path = selected.candidate.module_path().clone();
    let lookup = LoaderProviders::new(providers, selected.singleton);

    let mut pending_modules = BTreeMap::new();
    pending_modules.insert(
        entry_path.clone(),
        PendingModule {
            candidate: selected.candidate,
            imported_from: None,
        },
    );
    let mut staged = BTreeMap::new();
    let mut pending_errors = Vec::new();

    while let Some((module_path, pending)) = pending_modules.pop_first() {
        if staged.contains_key(&module_path) {
            continue;
        }
        let candidate = pending.candidate;
        let text = match fs::read_to_string(candidate.canonical_io_path()) {
            Ok(text) => text,
            Err(error) => {
                pending_errors.push(PendingLoadError::Source {
                    module_path,
                    candidate,
                    imported_from: pending.imported_from,
                    kind: error.kind(),
                });
                continue;
            }
        };
        let discovered = discover_dependencies(candidate.display_source_path(), &text);
        let dependencies = discovered
            .as_ref()
            .map(|parsed| dependency_occurrences(&parsed.ast, &parsed.compiler_dependency_ranges))
            .unwrap_or_default();
        staged.insert(module_path.clone(), StagedModule { candidate, text });

        for (target, occurrences) in dependencies {
            let first_range = occurrences.first_range();
            match lookup.resolve(&target) {
                Ok(candidate) => {
                    if !staged.contains_key(&target) {
                        let imported_from = Some((module_path.clone(), first_range));
                        pending_modules
                            .entry(target)
                            .and_modify(|pending: &mut PendingModule| {
                                let should_replace = pending.imported_from.as_ref().is_none_or(
                                    |(current_module, current_range)| {
                                        (&module_path, first_range.start())
                                            < (current_module, current_range.start())
                                    },
                                );
                                if should_replace {
                                    pending.imported_from = imported_from.clone();
                                }
                            })
                            .or_insert(PendingModule {
                                candidate,
                                imported_from,
                            });
                    }
                }
                Err(error) => pending_errors.push(PendingLoadError::Resolution {
                    importing_module: module_path.clone(),
                    import_range: first_range,
                    target,
                    error,
                }),
            }
        }
    }

    finalize_graph(entry_path, staged, pending_errors)
}

struct StagedModule {
    candidate: ModuleCandidate,
    text: String,
}

struct PendingModule {
    candidate: ModuleCandidate,
    imported_from: Option<(ModulePath, TextRange)>,
}

fn discover_dependencies(path: &Path, text: &str) -> Option<ParsedModule> {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add(path, text);
    parse_source(&sources, source_id).0
}

fn parse_source(
    sources: &SourceDatabase,
    source_id: SourceId,
) -> (Option<ParsedModule>, Diagnostics) {
    let source = sources
        .get(source_id)
        .expect("the loader parses an inserted source");
    let lexed = lex(source);
    let mut diagnostics = lexed.diagnostics;
    if diagnostics.has_errors() {
        return (None, diagnostics);
    }
    let parsed = parse(source, &lexed.tokens);
    let mut compiler_dependency_ranges = BTreeMap::<CompilerDependencyKind, Vec<TextRange>>::new();
    for token in &lexed.tokens {
        let kind = match token.kind {
            TokenKind::StringLiteral => CompilerDependencyKind::StringLiteral,
            TokenKind::For => CompilerDependencyKind::GeneralIteration,
            _ => continue,
        };
        compiler_dependency_ranges
            .entry(kind)
            .or_default()
            .push(token.span.range());
    }
    diagnostics.append(parsed.diagnostics);
    if diagnostics.has_errors() {
        (None, diagnostics)
    } else {
        (
            Some(ParsedModule {
                ast: parsed.ast,
                compiler_dependency_ranges,
            }),
            diagnostics,
        )
    }
}

struct ParsedModule {
    ast: CompilationUnit,
    compiler_dependency_ranges: BTreeMap<CompilerDependencyKind, Vec<TextRange>>,
}

#[derive(Default)]
struct DependencyOccurrences {
    import_ranges: Vec<TextRange>,
    compiler_dependency_ranges: BTreeMap<CompilerDependencyKind, Vec<TextRange>>,
}

impl DependencyOccurrences {
    fn first_range(&self) -> TextRange {
        self.import_ranges
            .first()
            .or_else(|| {
                self.compiler_dependency_ranges
                    .values()
                    .find_map(|ranges| ranges.first())
            })
            .copied()
            .expect("a discovered dependency has source evidence")
    }
}

fn dependency_occurrences(
    ast: &CompilationUnit,
    compiler_dependency_ranges: &BTreeMap<CompilerDependencyKind, Vec<TextRange>>,
) -> BTreeMap<ModulePath, DependencyOccurrences> {
    let mut dependencies = BTreeMap::<ModulePath, DependencyOccurrences>::new();
    for import in &ast.imports {
        let name = match import {
            ImportDeclaration::Module(import) => &import.module,
            ImportDeclaration::Selective(import) => &import.module,
        };
        let path = ModulePath::from_components(
            name.components().map(|component| component.text.to_owned()),
        )
        .expect("parsed import components are valid source identifiers");
        dependencies
            .entry(path)
            .or_default()
            .import_ranges
            .push(name.span.range());
    }
    for (&kind, ranges) in compiler_dependency_ranges {
        record_compiler_dependency(&mut dependencies, kind, ranges);
    }
    dependencies
}

fn record_compiler_dependency(
    dependencies: &mut BTreeMap<ModulePath, DependencyOccurrences>,
    kind: CompilerDependencyKind,
    ranges: &[TextRange],
) {
    if ranges.is_empty() {
        return;
    }
    let path = compiler_dependency_path(kind);
    dependencies
        .entry(path)
        .or_default()
        .compiler_dependency_ranges
        .entry(kind)
        .or_default()
        .extend(ranges.iter().copied());
}

pub(super) fn compiler_dependency_path(kind: CompilerDependencyKind) -> ModulePath {
    let path = match kind {
        CompilerDependencyKind::StringLiteral => "std::str",
        CompilerDependencyKind::GeneralIteration => "std::iter",
    };
    ModulePath::try_from(path).expect("compiler dependency path must be valid")
}

fn finalize_graph(
    entry_path: ModulePath,
    staged: BTreeMap<ModulePath, StagedModule>,
    pending_errors: Vec<PendingLoadError>,
) -> Result<ModuleGraph, ModuleGraphLoadFailure> {
    let mut sources = SourceDatabase::new();
    let mut source_ids = BTreeMap::new();
    for (path, module) in &staged {
        let source_id = sources.add(module.candidate.display_source_path(), &module.text);
        source_ids.insert(path.clone(), source_id);
    }

    let mut diagnostics = Diagnostics::new();
    let mut finalized = Vec::new();
    for (path, module) in staged {
        let source_id = source_ids[&path];
        let (parsed, source_diagnostics) = parse_source(&sources, source_id);
        diagnostics.append(source_diagnostics);
        if let Some(parsed) = parsed {
            finalized.push(FinalizedModule {
                path,
                candidate: module.candidate,
                source_id,
                ast: parsed.ast,
                compiler_dependency_ranges: parsed.compiler_dependency_ranges,
            });
        }
    }

    append_pending_diagnostics(&mut diagnostics, pending_errors, &source_ids);
    if diagnostics.has_errors() {
        return Err(ModuleGraphLoadFailure::new(sources, diagnostics));
    }

    let ids = finalized
        .iter()
        .enumerate()
        .map(|(index, module)| (module.path.clone(), ModuleId::new(index)))
        .collect::<BTreeMap<_, _>>();
    let imports = finalized
        .iter()
        .map(|module| finalized_dependencies(module, &ids))
        .collect::<Vec<_>>();
    for (index, (module, dependencies)) in finalized.iter().zip(&imports).enumerate() {
        if let Some(span) = dependencies
            .iter()
            .filter(|dependency| dependency.target().index() == index)
            .flat_map(|dependency| dependency.import_spans())
            .next()
        {
            diagnostics.push(self_import_diagnostic(&module.path, *span));
        }
    }
    if diagnostics.has_errors() {
        return Err(ModuleGraphLoadFailure::new(sources, diagnostics));
    }

    let entry = ids[&entry_path];
    let modules = finalized
        .into_iter()
        .zip(imports)
        .enumerate()
        .map(|(index, (module, imports))| {
            debug_assert_eq!(module.source_id.index(), index);
            let provenance = ModuleProvenance::new(
                ModuleId::new(index),
                module.path,
                module.source_id,
                module.candidate.provider_id(),
                module.candidate.package_id(),
                ModuleSourceLocation::new(
                    module.candidate.root_relative_path().to_owned(),
                    module.candidate.display_source_path().to_owned(),
                    Some(module.candidate.canonical_io_path().to_owned()),
                )
                .with_trace_source_path(module.candidate.trace_source_path().to_owned()),
            );
            LoadedModule::new(provenance, module.ast, imports)
        })
        .collect();
    Ok(ModuleGraph::new(sources, entry, modules))
}

struct FinalizedModule {
    path: ModulePath,
    candidate: ModuleCandidate,
    source_id: SourceId,
    ast: CompilationUnit,
    compiler_dependency_ranges: BTreeMap<CompilerDependencyKind, Vec<TextRange>>,
}

fn finalized_dependencies(
    module: &FinalizedModule,
    ids: &BTreeMap<ModulePath, ModuleId>,
) -> Vec<ModuleImportEdge> {
    dependency_occurrences(&module.ast, &module.compiler_dependency_ranges)
        .into_iter()
        .map(|(path, occurrences)| {
            let target = ids[&path];
            let source_id = module.ast.span.source_id();
            let import_spans = occurrences
                .import_ranges
                .into_iter()
                .map(|range| Span::new(source_id, range))
                .collect();
            let compiler_dependencies = occurrences
                .compiler_dependency_ranges
                .into_iter()
                .map(|(kind, ranges)| {
                    CompilerDependencyEvidence::new(
                        kind,
                        ranges
                            .into_iter()
                            .map(|range| Span::new(source_id, range))
                            .collect(),
                    )
                })
                .collect();
            ModuleImportEdge::new(target, import_spans, compiler_dependencies)
        })
        .collect()
}
