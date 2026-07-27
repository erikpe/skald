use super::*;

use crate::module::ModulePath;

#[test]
fn entry_selection_requires_exactly_one_form_without_filesystem_access() {
    assert_eq!(
        EntrySelector::from_options(Some("app/main.ska".into()), None).unwrap(),
        EntrySelector::File("app/main.ska".into())
    );
    assert_eq!(
        EntrySelector::from_options(None, Some("app::main".parse().unwrap())).unwrap(),
        EntrySelector::Module("app::main".parse().unwrap())
    );
    assert_eq!(
        EntrySelector::from_options(None, None),
        Err(EntrySelectionError::Missing)
    );
    assert_eq!(
        EntrySelector::from_options(
            Some("app/main.ska".into()),
            Some("app::main".parse().unwrap())
        ),
        Err(EntrySelectionError::Conflicting)
    );
}

#[test]
fn standard_library_options_are_typed_and_mutually_exclusive() {
    assert_eq!(
        StandardLibrarySelection::from_options(None, false).unwrap(),
        StandardLibrarySelection::Default
    );
    assert_eq!(
        StandardLibrarySelection::from_options(Some("sdk/modules".into()), false).unwrap(),
        StandardLibrarySelection::Replacement("sdk/modules".into())
    );
    assert_eq!(
        StandardLibrarySelection::from_options(None, true).unwrap(),
        StandardLibrarySelection::Disabled
    );
    assert_eq!(
        StandardLibrarySelection::from_options(Some("sdk/modules".into()), true),
        Err(StandardLibrarySelectionError::Conflicting)
    );
}

#[test]
fn compilation_request_retains_explicit_process_and_artifact_inputs() {
    let entry_path: ModulePath = "app::main".parse().unwrap();
    let request = CompilationRequest::new(
        EntrySelector::Module(entry_path.clone()),
        vec!["project/modules".into(), "deps/modules".into()],
        StandardLibrarySelection::Replacement("sdk/modules".into()),
        Target::X86_64SysV,
        ArtifactOptions::new(ArtifactKind::Assembly, Some("out/main.s".into())),
        CompilationEnvironment::new("/work/project".into(), "/install/skald/std".into()),
    );

    assert_eq!(request.entry(), &EntrySelector::Module(entry_path));
    assert_eq!(
        request.module_roots(),
        [
            PathBuf::from("project/modules"),
            PathBuf::from("deps/modules")
        ]
    );
    assert_eq!(
        request.standard_library(),
        &StandardLibrarySelection::Replacement("sdk/modules".into())
    );
    assert_eq!(request.target(), Target::X86_64SysV);
    assert_eq!(request.artifact().kind(), ArtifactKind::Assembly);
    assert_eq!(request.artifact().output(), Some(Path::new("out/main.s")));
    assert_eq!(
        request.environment().working_directory(),
        Path::new("/work/project")
    );
    assert_eq!(
        request.environment().default_standard_library_root(),
        Path::new("/install/skald/std")
    );
}
