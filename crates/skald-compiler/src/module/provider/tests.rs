use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::test_support::TemporaryDirectory;

use super::*;

fn directory(label: &str) -> TemporaryDirectory {
    TemporaryDirectory::new(label).unwrap()
}

fn create_source(root: &Path, relative: &str, text: &str) -> PathBuf {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    path
}

fn normalized(
    working_directory: &Path,
    configurations: &[ProviderRootConfiguration],
) -> ProviderSet {
    normalize_provider_roots(working_directory, configurations).unwrap()
}

fn unique(set: &ProviderSet, path: &str) -> ModuleCandidate {
    match set.resolve(&path.parse().unwrap()).unwrap() {
        CandidateResolution::Unique(candidate) => candidate,
        other => panic!("expected one candidate, found {other:?}"),
    }
}

#[test]
fn an_empty_provider_union_resolves_every_module_as_missing() {
    let workspace = directory("provider-empty");
    let providers = normalized(workspace.path(), &[]);

    assert!(providers.providers().is_empty());
    assert_eq!(
        providers.resolve(&"app::main".parse().unwrap()).unwrap(),
        CandidateResolution::Missing {
            module_path: "app::main".parse().unwrap(),
        }
    );
}

#[test]
fn normalizes_relative_roots_and_coalesces_equivalent_spellings_and_roles() {
    let workspace = directory("provider-normalization");
    let real = workspace.join("real");
    fs::create_dir(&real).unwrap();
    let alias = workspace.join("alias");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&real, &alias).unwrap();

    let configurations = vec![
        ProviderRootConfiguration::standard_library(alias.clone()),
        ProviderRootConfiguration::module_root(PathBuf::from("./real")),
        ProviderRootConfiguration::module_root(real.clone()),
    ];
    let providers = normalized(workspace.path(), &configurations);

    assert_eq!(providers.providers().len(), 1);
    let provider = &providers.providers()[0];
    assert_eq!(provider.id().index(), 0);
    assert_eq!(provider.package_id().index(), 0);
    assert_eq!(provider.canonical_root(), fs::canonicalize(&real).unwrap());
    assert_eq!(provider.spellings().len(), 3);
    assert!(provider
        .spellings()
        .iter()
        .any(|spelling| spelling.configuration().kind() == ProviderRootKind::StandardLibrary));

    let mut reversed = configurations;
    reversed.reverse();
    assert_eq!(providers, normalized(workspace.path(), &reversed));
}

#[test]
fn canonicalization_preserves_symlink_parent_component_semantics() {
    let workspace = directory("provider-symlink-parent");
    let outside = directory("provider-symlink-parent-target");
    let outside_child = outside.join("child");
    let outside_modules = outside.join("modules");
    fs::create_dir(&outside_child).unwrap();
    fs::create_dir(&outside_modules).unwrap();
    fs::create_dir(workspace.join("modules")).unwrap();
    let alias = workspace.join("alias");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside_child, &alias).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&outside_child, &alias).unwrap();

    let providers = normalized(
        workspace.path(),
        &[ProviderRootConfiguration::module_root(PathBuf::from(
            "alias/../modules",
        ))],
    );

    assert_eq!(
        providers.providers()[0].canonical_root(),
        fs::canonicalize(outside_modules).unwrap()
    );
    assert_eq!(
        providers.providers()[0].spellings()[0].lexical_path(),
        workspace.join("modules")
    );
}

#[test]
fn rejects_nonabsolute_working_directories_missing_roots_and_files_as_roots() {
    let workspace = directory("provider-invalid-root");
    let file = create_source(workspace.path(), "root.ska", "");

    let relative = normalize_provider_roots(
        Path::new("relative"),
        &[ProviderRootConfiguration::module_root(PathBuf::from("."))],
    )
    .unwrap_err();
    assert_eq!(
        relative[0].kind(),
        ProviderNormalizationErrorKind::WorkingDirectoryNotAbsolute
    );

    let errors = normalize_provider_roots(
        workspace.path(),
        &[
            ProviderRootConfiguration::module_root(PathBuf::from("missing")),
            ProviderRootConfiguration::standard_library(file),
        ],
    )
    .unwrap_err();
    assert_eq!(errors.len(), 2);
    assert!(matches!(
        errors[0].kind(),
        ProviderNormalizationErrorKind::Canonicalization(_)
    ));
    assert_eq!(
        errors[1].kind(),
        ProviderNormalizationErrorKind::NotDirectory
    );
}

#[test]
fn partial_logical_trees_compose_without_prefix_ownership() {
    let workspace = directory("provider-partial-tree");
    let project = workspace.join("project");
    let dependency = workspace.join("dependency");
    create_source(&project, "math/trigonometry.ska", "project");
    create_source(&dependency, "math/geometry.ska", "dependency");
    let providers = normalized(
        workspace.path(),
        &[
            ProviderRootConfiguration::module_root(dependency),
            ProviderRootConfiguration::module_root(project),
        ],
    );

    assert_eq!(
        unique(&providers, "math::trigonometry")
            .module_path()
            .to_string(),
        "math::trigonometry"
    );
    assert_eq!(
        unique(&providers, "math::geometry")
            .module_path()
            .to_string(),
        "math::geometry"
    );
    assert!(matches!(
        providers
            .resolve(&"math::missing".parse().unwrap())
            .unwrap(),
        CandidateResolution::Missing { .. }
    ));
}

#[test]
fn lookup_does_not_inspect_unrelated_files_below_a_root() {
    let workspace = directory("provider-no-scan");
    let root = workspace.join("root");
    create_source(&root, "wanted.ska", "");
    #[cfg(unix)]
    std::os::unix::fs::symlink(workspace.join("missing"), root.join("unrelated.ska")).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(workspace.join("missing"), root.join("unrelated.ska"))
        .unwrap();
    let providers = normalized(
        workspace.path(),
        &[ProviderRootConfiguration::module_root(root)],
    );

    assert_eq!(
        unique(&providers, "wanted").module_path().to_string(),
        "wanted"
    );
}

#[test]
fn exact_collisions_are_ambiguous_regardless_of_content_or_root_order() {
    let workspace = directory("provider-ambiguity");
    let first = workspace.join("first");
    let second = workspace.join("second");
    create_source(&first, "math/geometry.ska", "same");
    create_source(&second, "math/geometry.ska", "different");
    let path = "math::geometry".parse().unwrap();

    let providers = normalized(
        workspace.path(),
        &[
            ProviderRootConfiguration::module_root(second.clone()),
            ProviderRootConfiguration::module_root(first.clone()),
        ],
    );
    let CandidateResolution::Ambiguous { candidates, .. } = providers.resolve(&path).unwrap()
    else {
        panic!("expected ambiguity");
    };
    assert_eq!(candidates.len(), 2);
    assert!(candidates[0].provider_id() < candidates[1].provider_id());

    let reversed = normalized(
        workspace.path(),
        &[
            ProviderRootConfiguration::module_root(first),
            ProviderRootConfiguration::module_root(second),
        ],
    );
    assert_eq!(
        providers.resolve(&path).unwrap(),
        reversed.resolve(&path).unwrap()
    );
}

#[test]
fn hard_links_and_common_symlink_targets_do_not_deduplicate_providers() {
    let workspace = directory("provider-physical-collision");
    let shared = create_source(workspace.path(), "shared.ska", "shared");
    let first = workspace.join("first");
    let second = workspace.join("second");
    let third = workspace.join("third");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    fs::create_dir(&third).unwrap();
    fs::hard_link(&shared, first.join("same.ska")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&shared, second.join("same.ska")).unwrap();
        std::os::unix::fs::symlink(&shared, third.join("same.ska")).unwrap();
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(&shared, second.join("same.ska")).unwrap();
        std::os::windows::fs::symlink_file(&shared, third.join("same.ska")).unwrap();
    }
    let providers = normalized(
        workspace.path(),
        &[
            ProviderRootConfiguration::module_root(first),
            ProviderRootConfiguration::module_root(second),
            ProviderRootConfiguration::module_root(third),
        ],
    );

    let CandidateResolution::Ambiguous { candidates, .. } =
        providers.resolve(&"same".parse().unwrap()).unwrap()
    else {
        panic!("expected ambiguity");
    };
    assert_eq!(candidates.len(), 3);
    assert_ne!(candidates[0].provider_id(), candidates[1].provider_id());
    assert!(candidates
        .iter()
        .enumerate()
        .any(|(index, candidate)| candidates[index + 1..]
            .iter()
            .any(|other| candidate.canonical_io_path() == other.canonical_io_path())));
}

#[test]
fn symlink_files_directories_and_escapes_preserve_lexical_module_identity() {
    let workspace = directory("provider-symlink-root");
    let outside = directory("provider-symlink-outside");
    let root = workspace.join("root");
    fs::create_dir(&root).unwrap();
    let shared_file = create_source(outside.path(), "shared.data", "shared");
    let nested_source = create_source(outside.path(), "nested/source.ska", "nested");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&shared_file, root.join("first.ska")).unwrap();
        std::os::unix::fs::symlink(&shared_file, root.join("second.ska")).unwrap();
        std::os::unix::fs::symlink(outside.join("nested"), root.join("external")).unwrap();
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(&shared_file, root.join("first.ska")).unwrap();
        std::os::windows::fs::symlink_file(&shared_file, root.join("second.ska")).unwrap();
        std::os::windows::fs::symlink_dir(outside.join("nested"), root.join("external")).unwrap();
    }
    let providers = normalized(
        workspace.path(),
        &[ProviderRootConfiguration::module_root(root)],
    );

    let first = unique(&providers, "first");
    let second = unique(&providers, "second");
    let nested = unique(&providers, "external::source");
    assert_eq!(first.canonical_io_path(), second.canonical_io_path());
    assert_ne!(first.module_path(), second.module_path());
    assert_eq!(
        nested.canonical_io_path(),
        fs::canonicalize(nested_source).unwrap()
    );
    assert_eq!(
        nested.root_relative_path(),
        Path::new("external/source.ska")
    );
}

#[test]
fn broken_and_cyclic_symlinks_are_structured_lookup_errors() {
    let workspace = directory("provider-broken-links");
    let root = workspace.join("root");
    fs::create_dir(&root).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(workspace.join("missing"), root.join("broken.ska")).unwrap();
        std::os::unix::fs::symlink("cycle.ska", root.join("cycle.ska")).unwrap();
        std::os::unix::fs::symlink(workspace.join("missing-directory"), root.join("broken"))
            .unwrap();
        std::os::unix::fs::symlink("directory_cycle", root.join("directory_cycle")).unwrap();
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(workspace.join("missing"), root.join("broken.ska"))
            .unwrap();
        std::os::windows::fs::symlink_file("cycle.ska", root.join("cycle.ska")).unwrap();
        std::os::windows::fs::symlink_dir(workspace.join("missing-directory"), root.join("broken"))
            .unwrap();
        std::os::windows::fs::symlink_dir("directory_cycle", root.join("directory_cycle")).unwrap();
    }
    let providers = normalized(
        workspace.path(),
        &[ProviderRootConfiguration::module_root(root)],
    );

    for path in ["broken", "cycle", "broken::item", "directory_cycle::item"] {
        let errors = providers.resolve(&path.parse().unwrap()).unwrap_err();
        assert!(matches!(
            errors[0].kind(),
            CandidateLookupErrorKind::SymlinkResolution(_)
        ));
    }
}

#[test]
fn non_directory_components_and_non_regular_candidates_are_rejected() {
    let workspace = directory("provider-non-file");
    let root = workspace.join("root");
    create_source(&root, "blocked", "");
    fs::create_dir_all(root.join("directory.ska")).unwrap();
    let providers = normalized(
        workspace.path(),
        &[ProviderRootConfiguration::module_root(root)],
    );

    let blocked = providers
        .resolve(&"blocked::child".parse().unwrap())
        .unwrap_err();
    assert_eq!(
        blocked[0].kind(),
        CandidateLookupErrorKind::NonDirectoryComponent
    );
    let directory = providers
        .resolve(&"directory".parse().unwrap())
        .unwrap_err();
    assert_eq!(
        directory[0].kind(),
        CandidateLookupErrorKind::NonRegularFile
    );
}

#[test]
fn exact_case_is_verified_and_ambiguous_case_suggestions_are_deterministic() {
    let workspace = directory("provider-case");
    let root = workspace.join("root");
    create_source(&root, "math/Str.ska", "");
    let providers = normalized(
        workspace.path(),
        &[ProviderRootConfiguration::module_root(root)],
    );

    assert_eq!(
        unique(&providers, "math::Str").module_path().to_string(),
        "math::Str"
    );
    let mismatch = providers
        .resolve(&"Math::Str".parse().unwrap())
        .unwrap_err();
    assert_eq!(mismatch[0].kind(), CandidateLookupErrorKind::CaseMismatch);
}

#[test]
fn a_near_case_in_one_provider_does_not_poison_an_exact_candidate_in_another() {
    let workspace = directory("provider-case-union");
    let exact = workspace.join("exact");
    let near = workspace.join("near");
    create_source(&exact, "math/Str.ska", "");
    create_source(&near, "math/str.ska", "");
    let providers = normalized(
        workspace.path(),
        &[
            ProviderRootConfiguration::module_root(near),
            ProviderRootConfiguration::module_root(exact),
        ],
    );

    assert!(matches!(
        providers.resolve(&"math::Str".parse().unwrap()).unwrap(),
        CandidateResolution::Unique(_)
    ));
}

#[test]
fn case_collision_classification_is_independent_of_host_enumeration_order() {
    use std::ffi::{OsStr, OsString};

    let first = PathBuf::from("/root/Str.ska");
    let second = PathBuf::from("/root/str.ska");
    let classify = |entries: Vec<(OsString, PathBuf)>| {
        super::lookup::select_directory_component(OsStr::new("STR.ska"), entries)
    };
    let forward = classify(vec![
        (OsString::from("Str.ska"), first.clone()),
        (OsString::from("str.ska"), second.clone()),
    ]);
    let reverse = classify(vec![
        (OsString::from("str.ska"), second.clone()),
        (OsString::from("Str.ska"), first.clone()),
    ]);

    assert_eq!(forward, reverse);
    assert_eq!(
        forward,
        super::lookup::DirectoryComponentSelection::CaseCollision(vec![first, second])
    );
}

#[cfg(unix)]
#[test]
fn unreadable_files_and_directories_are_reported_when_reached() {
    use std::os::unix::fs::PermissionsExt;

    let workspace = directory("provider-unreadable");
    let root = workspace.join("root");
    let file = create_source(&root, "file.ska", "");
    let hidden = root.join("hidden");
    fs::create_dir(&hidden).unwrap();
    create_source(&hidden, "item.ska", "");
    let providers = normalized(
        workspace.path(),
        &[ProviderRootConfiguration::module_root(root)],
    );

    fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();
    let file_result = providers.resolve(&"file".parse().unwrap());
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    let file_errors = file_result.unwrap_err();
    assert!(matches!(
        file_errors[0].kind(),
        CandidateLookupErrorKind::UnreadableFile(_)
    ));

    fs::set_permissions(&hidden, fs::Permissions::from_mode(0o000)).unwrap();
    let directory_result = providers.resolve(&"hidden::item".parse().unwrap());
    fs::set_permissions(&hidden, fs::Permissions::from_mode(0o700)).unwrap();
    let directory_errors = directory_result.unwrap_err();
    assert!(matches!(
        directory_errors[0].kind(),
        CandidateLookupErrorKind::UnreadableDirectory(_)
    ));
}

#[test]
fn candidate_paths_and_ids_are_lexical_and_independent_of_physical_targets() {
    let workspace = directory("provider-candidate-provenance");
    let first = workspace.join("z-root");
    let second = workspace.join("a-root");
    create_source(&first, "app/main.ska", "");
    create_source(&second, "tool.ska", "");
    let providers = normalized(
        workspace.path(),
        &[
            ProviderRootConfiguration::module_root(first),
            ProviderRootConfiguration::module_root(second),
        ],
    );

    assert!(providers.providers()[0].canonical_root() < providers.providers()[1].canonical_root());
    let candidate = unique(&providers, "app::main");
    assert_eq!(candidate.root_relative_path(), Path::new("app/main.ska"));
    assert!(candidate.display_source_path().ends_with("app/main.ska"));
    assert_eq!(
        candidate.package_id().index(),
        candidate.provider_id().index()
    );
}
