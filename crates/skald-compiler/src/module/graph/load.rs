use std::{collections::BTreeMap, fs, path::Path};

use crate::{
    diagnostics::Diagnostics,
    driver::EntrySelector,
    identity::ModuleId,
    lexer::lex,
    source::{SourceDatabase, SourceId, Span, TextRange},
    syntax::{parse, CompilationUnit, ImportDeclaration},
};

use super::{
    cycle::find_cycle,
    diagnostic::{append_pending_diagnostics, cycle_diagnostic, entry_failure, PendingLoadError},
    entry::{select_entry, LoaderProviders},
    model::{LoadedModule, ModuleGraph, ModuleGraphLoadFailure, ModuleImportEdge},
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
        let discovered = discover_imports(candidate.display_source_path(), &text);
        let imports = discovered
            .as_ref()
            .map(import_occurrences)
            .unwrap_or_default();
        staged.insert(module_path.clone(), StagedModule { candidate, text });

        for (target, occurrences) in imports {
            match lookup.resolve(&target) {
                Ok(candidate) => {
                    if !staged.contains_key(&target) {
                        let imported_from = Some((module_path.clone(), occurrences[0]));
                        pending_modules
                            .entry(target)
                            .and_modify(|pending: &mut PendingModule| {
                                let should_replace = pending.imported_from.as_ref().is_none_or(
                                    |(current_module, current_range)| {
                                        (&module_path, occurrences[0].start())
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
                    import_range: occurrences[0],
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

fn discover_imports(path: &Path, text: &str) -> Option<CompilationUnit> {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add(path, text);
    parse_source(&sources, source_id).0
}

fn parse_source(
    sources: &SourceDatabase,
    source_id: SourceId,
) -> (Option<CompilationUnit>, Diagnostics) {
    let source = sources
        .get(source_id)
        .expect("the loader parses an inserted source");
    let lexed = lex(source);
    let mut diagnostics = lexed.diagnostics;
    if diagnostics.has_errors() {
        return (None, diagnostics);
    }
    let parsed = parse(source, &lexed.tokens);
    diagnostics.append(parsed.diagnostics);
    if diagnostics.has_errors() {
        (None, diagnostics)
    } else {
        (Some(parsed.ast), diagnostics)
    }
}

fn import_occurrences(ast: &CompilationUnit) -> BTreeMap<ModulePath, Vec<TextRange>> {
    let mut imports = BTreeMap::<ModulePath, Vec<TextRange>>::new();
    for import in &ast.imports {
        let name = match import {
            ImportDeclaration::Module(import) => &import.module,
            ImportDeclaration::Selective(import) => &import.module,
        };
        let path = ModulePath::from_components(
            name.components().map(|component| component.text.to_owned()),
        )
        .expect("parsed import components are valid source identifiers");
        imports.entry(path).or_default().push(name.span.range());
    }
    imports
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
        let (ast, source_diagnostics) = parse_source(&sources, source_id);
        diagnostics.append(source_diagnostics);
        if let Some(ast) = ast {
            finalized.push(FinalizedModule {
                path,
                candidate: module.candidate,
                source_id,
                ast,
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
        .map(|module| finalized_imports(&module.ast, &ids))
        .collect::<Vec<_>>();
    if let Some(cycle) = find_cycle(&imports) {
        let paths = finalized
            .iter()
            .map(|module| module.path.clone())
            .collect::<Vec<_>>();
        diagnostics.push(cycle_diagnostic(&cycle, &paths));
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
                ),
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
}

fn finalized_imports(
    ast: &CompilationUnit,
    ids: &BTreeMap<ModulePath, ModuleId>,
) -> Vec<ModuleImportEdge> {
    import_occurrences(ast)
        .into_iter()
        .map(|(path, ranges)| {
            let target = ids[&path];
            let source_id = ast.span.source_id();
            let spans = ranges
                .into_iter()
                .map(|range| Span::new(source_id, range))
                .collect();
            ModuleImportEdge::new(target, spans)
        })
        .collect()
}
