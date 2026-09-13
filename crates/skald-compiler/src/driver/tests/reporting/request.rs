use std::path::PathBuf;

use crate::{
    backend::Target,
    driver::{
        ArtifactKind, ArtifactOptions, CompilationEnvironment, CompilationRequest, EntrySelector,
        StandardLibrarySelection,
    },
    test_support::TemporaryDirectory,
};

pub(super) fn request(
    workspace: &TemporaryDirectory,
    root: PathBuf,
    entry: EntrySelector,
) -> CompilationRequest {
    CompilationRequest::new(
        entry,
        vec![root],
        StandardLibrarySelection::Disabled,
        Target::X86_64SysV,
        ArtifactOptions::new(ArtifactKind::Assembly, None),
        CompilationEnvironment::new(workspace.path().to_owned(), workspace.join("unused-std")),
    )
}
