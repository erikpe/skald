use std::path::Path;

use crate::{
    identity::{ModuleId, PackageId, ProviderId},
    source::SourceDatabase,
};

use super::*;

#[test]
fn logical_paths_parse_render_and_order_without_case_normalization() {
    let path: ModulePath = "std::Str".parse().unwrap();

    assert_eq!(path.len(), 2);
    assert_eq!(path.components().collect::<Vec<_>>(), ["std", "Str"]);
    assert_eq!(path.final_component(), "Str");
    assert_eq!(path.to_string(), "std::Str");
    assert_ne!(path, "std::str".parse().unwrap());
    assert!("app::main".parse::<ModulePath>().unwrap() < path);
}

#[test]
fn components_reuse_source_identifier_policy() {
    for valid in ["main", "_private", "app::_2", "public::from"] {
        assert!(valid.parse::<ModulePath>().is_ok(), "rejected `{valid}`");
    }

    for invalid in [
        "2main",
        "app::two-words",
        "std::Str!",
        "std::Str.ska",
        "näme",
    ] {
        let error = invalid.parse::<ModulePath>().unwrap_err();
        assert_eq!(error.kind(), ModulePathErrorKind::InvalidComponent);
    }
}

#[test]
fn invalid_paths_report_the_exact_component() {
    let empty = "".parse::<ModulePath>().unwrap_err();
    assert_eq!(empty.kind(), ModulePathErrorKind::EmptyPath);
    assert_eq!(empty.component_index(), None);
    assert_eq!(empty.component(), None);
    assert_eq!(
        empty.to_string(),
        "module path must contain at least one component"
    );

    let missing = "app::::main".parse::<ModulePath>().unwrap_err();
    assert_eq!(missing.kind(), ModulePathErrorKind::EmptyComponent);
    assert_eq!(missing.component_index(), Some(1));
    assert_eq!(missing.component(), Some(""));
    assert_eq!(missing.to_string(), "module path component 2 is empty");

    let invalid = ModulePath::from_components(["app", "bad-name"]).unwrap_err();
    assert_eq!(invalid.kind(), ModulePathErrorKind::InvalidComponent);
    assert_eq!(invalid.component_index(), Some(1));
    assert_eq!(invalid.component(), Some("bad-name"));
    assert_eq!(
        invalid.to_string(),
        "module path component 2 `bad-name` is not a Skald identifier"
    );

    assert_eq!(
        ModulePath::from_components(Vec::<String>::new())
            .unwrap_err()
            .kind(),
        ModulePathErrorKind::EmptyPath
    );
}

#[test]
fn provenance_keeps_logical_and_physical_roles_separate() {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("/display/math/geometry.ska", "fn value() -> i64;");
    let provenance = ModuleProvenance::new(
        ModuleId::new(4),
        "math::geometry".parse().unwrap(),
        source_id,
        ProviderId::new(2),
        PackageId::new(1),
        ModuleSourceLocation::new(
            "math/geometry.ska".into(),
            "/display/math/geometry.ska".into(),
            Some("/shared/geometry.ska".into()),
        ),
    );

    assert_eq!(provenance.module_id().index(), 4);
    assert_eq!(provenance.module_path().to_string(), "math::geometry");
    assert_eq!(provenance.source_id(), source_id);
    assert_eq!(provenance.provider_id().index(), 2);
    assert_eq!(provenance.package_id().index(), 1);
    assert_eq!(
        provenance.source_location().root_relative_path(),
        Path::new("math/geometry.ska")
    );
    assert_eq!(
        provenance.source_location().display_source_path(),
        Path::new("/display/math/geometry.ska")
    );
    assert_eq!(
        provenance.source_location().canonical_io_path(),
        Some(Path::new("/shared/geometry.ska"))
    );
}
