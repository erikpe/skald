use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    driver::EntrySelector,
    module::{normalize_provider_roots, ProviderRootConfiguration},
    test_support::TemporaryDirectory,
};

use super::{
    diagnostic::{
        AMBIGUOUS_ENTRY_IDENTITY, AMBIGUOUS_MODULE, INVALID_ENTRY, MISSING_MODULE,
        MODULE_LOOKUP_FAILURE, MODULE_SOURCE_FAILURE, SELF_IMPORT,
    },
    dump_module_graph, load_module_graph, CompilerDependencyKind, ModuleGraph,
    ModuleGraphLoadFailure,
};

fn directory(label: &str) -> TemporaryDirectory {
    TemporaryDirectory::new(label).unwrap()
}

fn source(root: &Path, relative: &str, text: &str) -> PathBuf {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    path
}

fn roots(working_directory: &Path, paths: &[PathBuf]) -> crate::module::ProviderSet {
    let configurations = paths
        .iter()
        .cloned()
        .map(ProviderRootConfiguration::module_root)
        .collect::<Vec<_>>();
    normalize_provider_roots(working_directory, &configurations).unwrap()
}

fn load(
    entry: EntrySelector,
    working_directory: &Path,
    root_paths: &[PathBuf],
) -> Result<ModuleGraph, ModuleGraphLoadFailure> {
    let providers = roots(working_directory, root_paths);
    load_module_graph(&entry, working_directory, &providers)
}

fn codes(failure: &ModuleGraphLoadFailure) -> Vec<&'static str> {
    failure
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn module_id(graph: &ModuleGraph, path: &str) -> usize {
    graph
        .find(&path.parse().unwrap())
        .unwrap()
        .provenance()
        .module_id()
        .index()
}

#[test]
fn logical_entry_loads_only_the_reachable_import_closure() {
    let workspace = directory("graph-reachability");
    let root = workspace.join("modules");
    source(
        &root,
        "app/main.ska",
        "import dep::used;\nfn main() -> i64 { return 0; }\n",
    );
    source(
        &root,
        "dep/used.ska",
        "public fn value() -> i64 { return 1; }\n",
    );
    source(&root, "dep/unreachable.ska", "fn broken( {\n");

    let graph = load(
        EntrySelector::Module("app::main".parse().unwrap()),
        workspace.path(),
        &[root],
    )
    .unwrap();

    assert_eq!(graph.modules().len(), 2);
    assert!(graph.find(&"app::main".parse().unwrap()).is_some());
    assert!(graph.find(&"dep::used".parse().unwrap()).is_some());
    assert!(graph.find(&"dep::unreachable".parse().unwrap()).is_none());
}

#[test]
fn hostile_import_paths_fail_without_panicking_or_scanning_the_root() {
    let workspace = directory("graph-hostile-import");
    let root = workspace.join("modules");
    let hostile_path = std::iter::repeat_n("component", 2_048)
        .collect::<Vec<_>>()
        .join("::");
    source(
        &root,
        "app.ska",
        &format!("import {hostile_path};\nfn main() -> i64 {{ return 0; }}\n"),
    );
    source(&root, "unrelated.ska", "fn malformed( {\n");

    let failure = load(
        EntrySelector::Module("app".parse().unwrap()),
        workspace.path(),
        &[root],
    )
    .unwrap_err();

    assert!(!failure.diagnostics().is_empty());
    assert_eq!(failure.sources().len(), 1);
}

#[test]
fn positional_and_logical_selection_intern_the_same_rooted_module() {
    let workspace = directory("graph-rooted-entry");
    let root = workspace.join("modules");
    let main = source(
        &root,
        "app/main.ska",
        "import dep;\nfn main() -> i64 { return 0; }\n",
    );
    source(&root, "dep.ska", "fn value() -> i64 { return 1; }\n");

    let logical = load(
        EntrySelector::Module("app::main".parse().unwrap()),
        workspace.path(),
        std::slice::from_ref(&root),
    )
    .unwrap();
    let positional = load(
        EntrySelector::File(main),
        workspace.path(),
        std::slice::from_ref(&root),
    )
    .unwrap();

    assert_eq!(
        logical
            .module(logical.entry())
            .unwrap()
            .provenance()
            .module_path(),
        positional
            .module(positional.entry())
            .unwrap()
            .provenance()
            .module_path()
    );
    assert_eq!(
        module_id(&logical, "app::main"),
        module_id(&positional, "app::main")
    );
    assert_eq!(logical.modules().len(), positional.modules().len());
}

#[test]
fn positional_containment_uses_root_spellings_but_not_descendant_targets() {
    let workspace = directory("graph-lexical-containment");
    let outside = directory("graph-lexical-containment-outside");
    let real_root = workspace.join("real");
    let root_alias = workspace.join("alias");
    let rooted = source(
        &real_root,
        "app/main.ska",
        "fn main() -> i64 { return 0; }\n",
    );
    let outside_source = source(
        outside.path(),
        "shared.data",
        "fn main() -> i64 { return 0; }\n",
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&real_root, &root_alias).unwrap();
        std::os::unix::fs::symlink(&outside_source, outside.join("outside_main.ska")).unwrap();
        std::os::unix::fs::symlink(&outside_source, real_root.join("inside.ska")).unwrap();
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(&real_root, &root_alias).unwrap();
        std::os::windows::fs::symlink_file(&outside_source, outside.join("outside_main.ska"))
            .unwrap();
        std::os::windows::fs::symlink_file(&outside_source, real_root.join("inside.ska")).unwrap();
    }

    let providers = roots(workspace.path(), std::slice::from_ref(&root_alias));
    let through_alias = load_module_graph(
        &EntrySelector::File(root_alias.join("app/main.ska")),
        workspace.path(),
        &providers,
    )
    .unwrap();
    assert_eq!(
        through_alias
            .module(through_alias.entry())
            .unwrap()
            .provenance()
            .module_path()
            .to_string(),
        "app::main"
    );
    assert_eq!(
        through_alias
            .module(through_alias.entry())
            .unwrap()
            .provenance()
            .source_location()
            .canonical_io_path(),
        Some(fs::canonicalize(rooted).unwrap().as_path())
    );

    let outside_entry = load_module_graph(
        &EntrySelector::File(outside.join("outside_main.ska")),
        workspace.path(),
        &providers,
    )
    .unwrap();
    assert_eq!(
        outside_entry
            .module(outside_entry.entry())
            .unwrap()
            .provenance()
            .module_path()
            .to_string(),
        "outside_main"
    );
    assert_eq!(
        outside_entry
            .module(outside_entry.entry())
            .unwrap()
            .provenance()
            .source_location()
            .trace_source_path(),
        outside.join("outside_main.ska")
    );
    assert!(outside_entry.find(&"inside".parse().unwrap()).is_none());
}

#[test]
fn relative_positional_entry_uses_the_captured_working_directory() {
    let workspace = directory("graph-relative-entry");
    let root = workspace.join("modules");
    source(&root, "app/main.ska", "fn main() -> i64 { return 0; }\n");

    let graph = load(
        EntrySelector::File("modules/other/../app/main.ska".into()),
        workspace.path(),
        &[root],
    )
    .unwrap();

    assert_eq!(
        graph
            .module(graph.entry())
            .unwrap()
            .provenance()
            .module_path()
            .to_string(),
        "app::main"
    );
    assert_eq!(
        graph
            .module(graph.entry())
            .unwrap()
            .provenance()
            .source_location()
            .display_source_path(),
        Path::new("modules/other/../app/main.ska")
    );
    assert_eq!(
        graph
            .module(graph.entry())
            .unwrap()
            .provenance()
            .source_location()
            .trace_source_path(),
        Path::new("app/main.ska")
    );
}

#[test]
fn relative_outside_entry_retains_its_configured_trace_spelling() {
    let workspace = directory("graph-relative-outside-entry");
    source(
        workspace.path(),
        "outside/main.ska",
        "fn main() -> i64 { return 0; }\n",
    );

    let graph = load(
        EntrySelector::File("outside/other/../main.ska".into()),
        workspace.path(),
        &[],
    )
    .unwrap();

    assert_eq!(
        graph
            .module(graph.entry())
            .unwrap()
            .provenance()
            .source_location()
            .trace_source_path(),
        Path::new("outside/other/../main.ska")
    );
}

#[test]
fn overlapping_roots_make_a_positional_identity_ambiguous() {
    let workspace = directory("graph-entry-containment");
    let outer = workspace.join("outer");
    let inner = outer.join("inner");
    let main = source(&inner, "main.ska", "fn main() -> i64 { return 0; }\n");

    let failure = load(EntrySelector::File(main), workspace.path(), &[outer, inner]).unwrap_err();

    assert_eq!(codes(&failure), [AMBIGUOUS_ENTRY_IDENTITY]);
    assert_eq!(failure.diagnostics().iter().next().unwrap().notes.len(), 2);
}

#[test]
fn outside_entry_is_a_singleton_and_does_not_expose_its_parent() {
    let workspace = directory("graph-singleton");
    let outside = directory("graph-singleton-outside");
    let main = source(
        outside.path(),
        "main.ska",
        "import sibling;\nfn main() -> i64 { return 0; }\n",
    );
    source(
        outside.path(),
        "sibling.ska",
        "fn value() -> i64 { return 1; }\n",
    );

    let failure = load(EntrySelector::File(main), workspace.path(), &[]).unwrap_err();

    assert_eq!(codes(&failure), [MISSING_MODULE]);
    let diagnostic = failure.diagnostics().iter().next().unwrap();
    assert_eq!(diagnostic.labels.len(), 1);
    assert_eq!(failure.sources().len(), 1);
}

#[test]
fn singleton_participates_in_ambiguity_and_self_import_rules() {
    let workspace = directory("graph-singleton-rules");
    let root = workspace.join("modules");
    source(&root, "main.ska", "fn other() -> i64 { return 1; }\n");
    let outside = directory("graph-singleton-rules-outside");
    let main = source(
        outside.path(),
        "main.ska",
        "import main;\nfn main() -> i64 { return 0; }\n",
    );

    let ambiguous = load(
        EntrySelector::File(main.clone()),
        workspace.path(),
        std::slice::from_ref(&root),
    )
    .unwrap_err();
    assert_eq!(codes(&ambiguous), [AMBIGUOUS_MODULE]);

    let self_import = load(EntrySelector::File(main), workspace.path(), &[]).unwrap_err();
    assert_eq!(codes(&self_import), [SELF_IMPORT]);
    assert_eq!(
        self_import.diagnostics().iter().next().unwrap().message,
        "module `main` cannot import itself"
    );
}

#[test]
fn positional_entry_validates_suffix_stem_existence_and_file_kind() {
    let workspace = directory("graph-entry-validation");
    source(workspace.path(), "wrong.txt", "");
    source(workspace.path(), "bad-name.ska", "");
    fs::create_dir(workspace.join("directory.ska")).unwrap();

    for path in ["wrong.txt", "bad-name.ska", "missing.ska", "directory.ska"] {
        let failure = load(EntrySelector::File(path.into()), workspace.path(), &[]).unwrap_err();
        assert_eq!(codes(&failure), [INVALID_ENTRY], "{path}");
    }
}

#[test]
fn repeated_import_sources_share_one_edge_and_one_parsed_module() {
    let workspace = directory("graph-duplicate-edge");
    let root = workspace.join("modules");
    source(
        &root,
        "main.ska",
        "import dep;\nfrom dep import value;\nfn main() -> i64 { return 0; }\n",
    );
    source(&root, "dep.ska", "public fn value() -> i64 { return 1; }\n");

    let graph = load(
        EntrySelector::Module("main".parse().unwrap()),
        workspace.path(),
        &[root],
    )
    .unwrap();
    let main = graph.find(&"main".parse().unwrap()).unwrap();

    assert_eq!(graph.modules().len(), 2);
    assert_eq!(main.imports().len(), 1);
    assert_eq!(main.imports()[0].import_spans().len(), 2);
}

#[test]
fn string_literals_add_one_synthetic_std_str_dependency_with_all_evidence() {
    let workspace = directory("graph-string-dependency");
    let root = workspace.join("modules");
    source(
        &root,
        "app.ska",
        "fn main() -> i64 { var first: i64 = \"a\"; return \"b\"; }\n",
    );
    source(&root, "std/str.ska", "public class Str {}\n");

    let graph = load(
        EntrySelector::Module("app".parse().unwrap()),
        workspace.path(),
        &[root],
    )
    .unwrap();
    let app = graph.find(&"app".parse().unwrap()).unwrap();

    assert_eq!(graph.modules().len(), 2);
    assert!(graph.find(&"std::str".parse().unwrap()).is_some());
    assert_eq!(app.imports().len(), 1);
    assert!(app.imports()[0].import_spans().is_empty());
    assert_eq!(app.imports()[0].string_literal_spans().len(), 2);
    assert_eq!(app.imports()[0].compiler_dependencies().len(), 1);
    assert_eq!(
        app.imports()[0].compiler_dependencies()[0].kind(),
        CompilerDependencyKind::StringLiteral
    );
    assert!(app.imports()[0]
        .compiler_dependency_spans(CompilerDependencyKind::GeneralIteration)
        .is_empty());
}

#[test]
fn explicit_and_synthetic_std_str_dependencies_coalesce_without_losing_kind() {
    let workspace = directory("graph-string-coalescing");
    let root = workspace.join("modules");
    source(
        &root,
        "app.ska",
        "import std::str;\nfn main() -> i64 { return \"a\"; }\n",
    );
    source(&root, "std/str.ska", "public class Str {}\n");

    let graph = load(
        EntrySelector::Module("app".parse().unwrap()),
        workspace.path(),
        &[root],
    )
    .unwrap();
    let edge = &graph.find(&"app".parse().unwrap()).unwrap().imports()[0];

    assert_eq!(edge.import_spans().len(), 1);
    assert_eq!(edge.string_literal_spans().len(), 1);
}

#[test]
fn modules_without_literals_do_not_require_std_str() {
    let workspace = directory("graph-no-string-dependency");
    let root = workspace.join("modules");
    source(&root, "app.ska", "fn main() -> i64 { return 0; }\n");

    let graph = load(
        EntrySelector::Module("app".parse().unwrap()),
        workspace.path(),
        &[root],
    )
    .unwrap();

    assert_eq!(graph.modules().len(), 1);
    assert!(graph.find(&"std::str".parse().unwrap()).is_none());
}

#[test]
fn canonical_iteration_dependency_uses_ordinary_missing_and_ambiguity_rules() {
    let workspace = directory("graph-iteration-dependency-errors");
    let first = workspace.join("first");
    let second = workspace.join("second");
    source(
        &first,
        "app.ska",
        "import std::iter;\nfn main() -> i64 { return 0; }\n",
    );

    let missing = load(
        EntrySelector::Module("app".parse().unwrap()),
        workspace.path(),
        std::slice::from_ref(&first),
    )
    .unwrap_err();
    assert_eq!(codes(&missing), [MISSING_MODULE]);

    source(
        &first,
        "std/iter.ska",
        "public interface Iterable<Item, State> {}\n",
    );
    source(
        &second,
        "std/iter.ska",
        "public interface Iterable<Item, State> {}\n",
    );
    let ambiguous = load(
        EntrySelector::Module("app".parse().unwrap()),
        workspace.path(),
        &[first, second],
    )
    .unwrap_err();
    assert_eq!(codes(&ambiguous), [AMBIGUOUS_MODULE]);
}

#[test]
fn compiler_dependency_kinds_own_exact_canonical_module_paths() {
    assert_eq!(
        super::load::compiler_dependency_path(CompilerDependencyKind::StringLiteral),
        "std::str".parse().unwrap()
    );
    assert_eq!(
        super::load::compiler_dependency_path(CompilerDependencyKind::GeneralIteration),
        "std::iter".parse().unwrap()
    );
}

#[test]
fn canonical_error_module_reaches_string_module_through_an_ordinary_import() {
    let workspace = directory("graph-error-dependency");
    let root = workspace.join("modules");
    source(
        &root,
        "app.ska",
        "import std::error;\nfn main() -> i64 { return 0; }\n",
    );
    source(
        &root,
        "std/error.ska",
        concat!(
            "import std::str;\n",
            "public intrinsic fn panic(message: std::str::Str) -> unit;\n",
        ),
    );
    source(&root, "std/str.ska", "public class Str {}\n");

    let graph = load(
        EntrySelector::Module("app".parse().unwrap()),
        workspace.path(),
        &[root],
    )
    .unwrap();
    let error = graph.find(&"std::error".parse().unwrap()).unwrap();

    assert!(graph.find(&"std::str".parse().unwrap()).is_some());
    assert_eq!(error.imports().len(), 1);
    assert_eq!(error.imports()[0].import_spans().len(), 1);
    assert!(error.imports()[0].string_literal_spans().is_empty());
}

#[test]
fn synthetic_std_str_dependency_uses_ordinary_missing_ambiguity_and_case_rules() {
    let workspace = directory("graph-string-provider-rules");
    let first = workspace.join("first");
    let second = workspace.join("second");
    source(&first, "app.ska", "fn main() -> i64 { return \"a\"; }\n");

    let missing = load(
        EntrySelector::Module("app".parse().unwrap()),
        workspace.path(),
        std::slice::from_ref(&first),
    )
    .unwrap_err();
    assert_eq!(codes(&missing), [MISSING_MODULE]);

    source(&first, "Std/str.ska", "public class Str {}\n");
    let wrong_case = load(
        EntrySelector::Module("app".parse().unwrap()),
        workspace.path(),
        std::slice::from_ref(&first),
    )
    .unwrap_err();
    assert_eq!(codes(&wrong_case), [MODULE_LOOKUP_FAILURE]);

    source(&first, "std/str.ska", "public class Str {}\n");
    source(&second, "std/str.ska", "public class Str {}\n");
    let ambiguous = load(
        EntrySelector::Module("app".parse().unwrap()),
        workspace.path(),
        &[first, second],
    )
    .unwrap_err();
    assert_eq!(codes(&ambiguous), [AMBIGUOUS_MODULE]);
}

#[test]
fn synthetic_std_str_dependency_reports_malformed_and_non_utf8_sources() {
    let workspace = directory("graph-string-source-errors");
    let root = workspace.join("modules");
    source(
        &root,
        "app.ska",
        "fn main() -> i64 { \"requires Str\"; return 0; }\n",
    );

    source(&root, "std/str.ska", "public class Str {\n");
    let malformed = load(
        EntrySelector::Module("app".parse().unwrap()),
        workspace.path(),
        std::slice::from_ref(&root),
    )
    .unwrap_err();
    assert_eq!(codes(&malformed), ["PAR002"]);

    fs::write(root.join("std/str.ska"), [0xff, 0xfe]).unwrap();
    let non_utf8 = load(
        EntrySelector::Module("app".parse().unwrap()),
        workspace.path(),
        std::slice::from_ref(&root),
    )
    .unwrap_err();
    assert_eq!(codes(&non_utf8), [MODULE_SOURCE_FAILURE]);
}

#[test]
fn synthetic_string_dependencies_may_participate_in_cycles() {
    let workspace = directory("graph-string-cycle");
    let root = workspace.join("modules");
    source(&root, "app.ska", "fn main() -> i64 { return \"a\"; }\n");
    source(&root, "std/str.ska", "import app;\npublic class Str {}\n");

    let graph = load(
        EntrySelector::Module("app".parse().unwrap()),
        workspace.path(),
        &[root],
    )
    .unwrap();

    assert_eq!(graph.modules().len(), 2);
    let app = graph.find(&"app".parse().unwrap()).unwrap();
    let string = graph.find(&"std::str".parse().unwrap()).unwrap();
    assert_eq!(app.imports().len(), 1);
    assert!(app.imports()[0].import_spans().is_empty());
    assert_eq!(app.imports()[0].string_literal_spans().len(), 1);
    assert_eq!(app.imports()[0].target(), string.provenance().module_id());
    assert_eq!(string.imports()[0].target(), app.provenance().module_id());

    let self_workspace = directory("graph-string-self-dependency");
    let self_root = self_workspace.join("modules");
    source(
        &self_root,
        "std/str.ska",
        "fn main() -> i64 { \"self\"; return 0; }\n",
    );
    let self_graph = load(
        EntrySelector::Module("std::str".parse().unwrap()),
        self_workspace.path(),
        std::slice::from_ref(&self_root),
    )
    .unwrap();
    let string = self_graph.find(&"std::str".parse().unwrap()).unwrap();
    assert_eq!(string.imports().len(), 1);
    assert_eq!(
        string.imports()[0].target(),
        string.provenance().module_id()
    );
    assert!(string.imports()[0].import_spans().is_empty());
    assert_eq!(string.imports()[0].string_literal_spans().len(), 1);
}

#[test]
fn common_physical_source_has_distinct_logical_source_and_module_instances() {
    let workspace = directory("graph-physical-instances");
    let root = workspace.join("modules");
    source(
        &root,
        "main.ska",
        "import first;\nimport nested::second;\nfn main() -> i64 { return 0; }\n",
    );
    let shared = source(
        workspace.path(),
        "shared.ska",
        "fn value() -> i64 { return 1; }\n",
    );
    fs::create_dir_all(root.join("nested")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&shared, root.join("first.ska")).unwrap();
        std::os::unix::fs::symlink(&shared, root.join("nested/second.ska")).unwrap();
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(&shared, root.join("first.ska")).unwrap();
        std::os::windows::fs::symlink_file(&shared, root.join("nested/second.ska")).unwrap();
    }

    let graph = load(
        EntrySelector::Module("main".parse().unwrap()),
        workspace.path(),
        &[root],
    )
    .unwrap();
    let first = graph.find(&"first".parse().unwrap()).unwrap().provenance();
    let second = graph
        .find(&"nested::second".parse().unwrap())
        .unwrap()
        .provenance();

    assert_ne!(first.module_id(), second.module_id());
    assert_ne!(first.source_id(), second.source_id());
    assert_eq!(
        first.source_location().canonical_io_path(),
        second.source_location().canonical_io_path()
    );
}

#[test]
fn missing_ambiguous_and_invalid_import_candidates_are_cross_file_diagnostics() {
    let workspace = directory("graph-import-errors");
    let first = workspace.join("first");
    let second = workspace.join("second");
    source(
        &first,
        "main.ska",
        "import missing;\nimport collision;\nfn main() -> i64 { return 0; }\n",
    );
    source(&first, "collision.ska", "fn one() -> i64 { return 1; }\n");
    source(&second, "collision.ska", "fn two() -> i64 { return 2; }\n");

    let failure = load(
        EntrySelector::Module("main".parse().unwrap()),
        workspace.path(),
        &[first, second],
    )
    .unwrap_err();

    assert_eq!(codes(&failure), [AMBIGUOUS_MODULE, MISSING_MODULE]);
    assert!(failure
        .diagnostics()
        .iter()
        .all(|diagnostic| diagnostic.labels.len() == 1));
}

#[test]
fn malformed_and_non_utf8_imported_sources_stop_graph_construction() {
    let workspace = directory("graph-source-errors");
    let root = workspace.join("modules");
    source(
        &root,
        "main.ska",
        "import malformed;\nimport bytes;\nfn main() -> i64 { return 0; }\n",
    );
    source(&root, "malformed.ska", "fn broken( {\n");
    fs::write(root.join("bytes.ska"), [0xff, 0xfe]).unwrap();

    let failure = load(
        EntrySelector::Module("main".parse().unwrap()),
        workspace.path(),
        &[root],
    )
    .unwrap_err();
    let failure_codes = codes(&failure);

    assert!(failure_codes.contains(&MODULE_SOURCE_FAILURE));
    assert!(failure_codes.iter().any(|code| code.starts_with("PAR")));
    assert_eq!(failure.sources().len(), 2);
    let source_failure = failure
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == MODULE_SOURCE_FAILURE)
        .unwrap();
    assert_eq!(source_failure.labels.len(), 1);
}

#[test]
fn direct_self_import_is_rejected_but_multi_module_cycles_are_loaded() {
    let workspace = directory("graph-cycles");
    let root = workspace.join("modules");
    source(&root, "self_cycle.ska", "import self_cycle;\n");

    let self_failure = load(
        EntrySelector::Module("self_cycle".parse().unwrap()),
        workspace.path(),
        std::slice::from_ref(&root),
    )
    .unwrap_err();
    let self_diagnostic = self_failure.diagnostics().iter().next().unwrap();
    assert_eq!(self_diagnostic.code, SELF_IMPORT);
    assert_eq!(
        self_diagnostic.message,
        "module `self_cycle` cannot import itself"
    );
    assert_eq!(self_diagnostic.labels.len(), 1);
    assert_eq!(
        self_diagnostic.labels[0].message,
        "remove this redundant import"
    );
    assert_eq!(
        self_diagnostic.notes,
        ["a module's own declarations are available without importing it"]
    );

    source(&root, "pair_a.ska", "import pair_b;\n");
    source(&root, "pair_b.ska", "import pair_a;\n");
    let pair = load(
        EntrySelector::Module("pair_a".parse().unwrap()),
        workspace.path(),
        std::slice::from_ref(&root),
    )
    .unwrap();
    assert_eq!(pair.modules().len(), 2);

    source(&root, "a.ska", "import b;\n");
    source(&root, "b.ska", "import c;\n");
    source(&root, "c.ska", "import a;\n");
    let graph = load(
        EntrySelector::Module("a".parse().unwrap()),
        workspace.path(),
        std::slice::from_ref(&root),
    )
    .unwrap();
    assert_eq!(module_id(&graph, "a"), 0);
    assert_eq!(module_id(&graph, "b"), 1);
    assert_eq!(module_id(&graph, "c"), 2);

    let dump = dump_module_graph(&graph).replace(workspace.path().to_str().unwrap(), "<workspace>");
    assert_eq!(
        dump,
        concat!(
            "entry m0 a\n",
            "module m0 a source0 provider0 package0\n",
            "  relative a.ska\n",
            "  display <workspace>/modules/a.ska\n",
            "  canonical <workspace>/modules/a.ska\n",
            "  dependency m1 b imports=1 string_literals=0\n",
            "module m1 b source1 provider0 package0\n",
            "  relative b.ska\n",
            "  display <workspace>/modules/b.ska\n",
            "  canonical <workspace>/modules/b.ska\n",
            "  dependency m2 c imports=1 string_literals=0\n",
            "module m2 c source2 provider0 package0\n",
            "  relative c.ska\n",
            "  display <workspace>/modules/c.ska\n",
            "  canonical <workspace>/modules/c.ska\n",
            "  dependency m0 a imports=1 string_literals=0\n",
        )
    );
}

#[test]
fn deep_cyclic_closure_loads_without_recursive_stack_growth() {
    let workspace = directory("graph-deep-cycle");
    let root = workspace.join("modules");
    let depth = 512;
    for index in 0..depth {
        let target = if index + 1 == depth {
            depth / 2
        } else {
            index + 1
        };
        source(
            &root,
            &format!("module_{index:04}.ska"),
            &format!("import module_{target:04};\n"),
        );
    }

    let graph = load(
        EntrySelector::Module("module_0000".parse().unwrap()),
        workspace.path(),
        std::slice::from_ref(&root),
    )
    .unwrap();

    assert_eq!(graph.modules().len(), depth);
    assert_eq!(module_id(&graph, "module_0000"), 0);
    assert_eq!(module_id(&graph, "module_0511"), depth - 1);
    let last = graph.find(&"module_0511".parse().unwrap()).unwrap();
    assert_eq!(
        last.imports()[0].target().index(),
        module_id(&graph, "module_0256")
    );
}

#[test]
fn identities_and_dump_follow_canonical_module_order_not_discovery_order() {
    let workspace = directory("graph-deterministic-order");
    let root = workspace.join("modules");
    source(&root, "z.ska", "import y;\n");
    source(&root, "y.ska", "import a;\n");
    source(&root, "a.ska", "fn value() -> i64 { return 1; }\n");

    let graph = load(
        EntrySelector::Module("z".parse().unwrap()),
        workspace.path(),
        std::slice::from_ref(&root),
    )
    .unwrap();

    assert_eq!(module_id(&graph, "a"), 0);
    assert_eq!(module_id(&graph, "y"), 1);
    assert_eq!(module_id(&graph, "z"), 2);
    for module in graph.modules() {
        assert_eq!(
            module.provenance().module_id().index(),
            module.provenance().source_id().index()
        );
    }

    let dump = dump_module_graph(&graph).replace(workspace.path().to_str().unwrap(), "<workspace>");
    assert_eq!(
        dump,
        concat!(
            "entry m2 z\n",
            "module m0 a source0 provider0 package0\n",
            "  relative a.ska\n",
            "  display <workspace>/modules/a.ska\n",
            "  canonical <workspace>/modules/a.ska\n",
            "module m1 y source1 provider0 package0\n",
            "  relative y.ska\n",
            "  display <workspace>/modules/y.ska\n",
            "  canonical <workspace>/modules/y.ska\n",
            "  dependency m0 a imports=1 string_literals=0\n",
            "module m2 z source2 provider0 package0\n",
            "  relative z.ska\n",
            "  display <workspace>/modules/z.ska\n",
            "  canonical <workspace>/modules/z.ska\n",
            "  dependency m1 y imports=1 string_literals=0\n",
        )
    );
}
