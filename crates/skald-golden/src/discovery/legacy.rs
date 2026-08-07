use crate::PlanError;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

const CASE_ARGUMENTS_FILE: &str = "case.args";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LegacyKind {
    Run,
    CompileFail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LegacyCase {
    pub(crate) kind: LegacyKind,
    pub(crate) expectation_stem: PathBuf,
    pub(crate) relative_stem: PathBuf,
    pub(crate) compiler_working_directory: PathBuf,
    pub(crate) compiler_args: Vec<OsString>,
    pub(crate) diagnostic_path_prefix: Option<Vec<u8>>,
}

pub(super) fn discover(golden_root: &Path) -> Result<Vec<LegacyCase>, PlanError> {
    let repository = golden_root.parent().and_then(Path::parent).ok_or_else(|| {
        PlanError::at_path(
            golden_root,
            "legacy fixtures require a repository/tests/golden layout",
        )
    })?;
    let mut cases = Vec::new();
    for (directory, kind) in [
        ("run", LegacyKind::Run),
        ("compile_fail", LegacyKind::CompileFail),
    ] {
        let root = golden_root.join(directory);
        if root.try_exists().map_err(|error| {
            PlanError::at_path(&root, format!("could not inspect legacy root: {error}"))
        })? {
            if !root.is_dir() {
                return Err(PlanError::at_path(&root, "legacy root is not a directory"));
            }
            discover_directory(golden_root, repository, &root, kind, &mut cases)?;
        }
    }
    cases.sort_by(|left, right| left.relative_stem.cmp(&right.relative_stem));
    Ok(cases)
}

fn discover_directory(
    golden_root: &Path,
    repository: &Path,
    directory: &Path,
    kind: LegacyKind,
    cases: &mut Vec<LegacyCase>,
) -> Result<(), PlanError> {
    let arguments_path = directory.join(CASE_ARGUMENTS_FILE);
    if arguments_path.is_file() {
        cases.push(load_multi_file_case(
            golden_root,
            directory,
            &arguments_path,
            kind,
        )?);
        return Ok(());
    }

    let mut entries = fs::read_dir(directory)
        .map_err(|error| {
            PlanError::at_path(
                directory,
                format!("could not read legacy directory: {error}"),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            PlanError::at_path(
                directory,
                format!("could not read legacy directory entry: {error}"),
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            PlanError::at_path(&path, format!("could not inspect legacy entry: {error}"))
        })?;
        if file_type.is_dir() {
            discover_directory(golden_root, repository, &path, kind, cases)?;
        } else if file_type.is_file()
            && path.extension().is_some_and(|extension| extension == "ska")
        {
            let relative_stem = path
                .strip_prefix(golden_root)
                .expect("legacy source is below golden root")
                .to_owned();
            let repository_relative = path.strip_prefix(repository).map_err(|_| {
                PlanError::at_path(
                    &path,
                    "legacy source is not below the inferred repository root",
                )
            })?;
            let compiler_arg = repository_relative.as_os_str().to_owned();
            cases.push(LegacyCase {
                kind,
                expectation_stem: path,
                relative_stem,
                compiler_working_directory: repository.to_owned(),
                compiler_args: vec![compiler_arg],
                diagnostic_path_prefix: None,
            });
        }
    }
    Ok(())
}

fn load_multi_file_case(
    golden_root: &Path,
    directory: &Path,
    arguments_path: &Path,
    kind: LegacyKind,
) -> Result<LegacyCase, PlanError> {
    let text = fs::read_to_string(arguments_path).map_err(|error| {
        PlanError::at_path(
            arguments_path,
            format!("could not read legacy compiler arguments as UTF-8: {error}"),
        )
    })?;
    let compiler_args = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(OsString::from)
        .collect::<Vec<_>>();
    if compiler_args.is_empty() {
        return Err(PlanError::at_path(
            arguments_path,
            "legacy case.args contains no arguments",
        ));
    }
    let canonical_directory = fs::canonicalize(directory).map_err(|error| {
        PlanError::at_path(
            directory,
            format!("could not canonicalize legacy case directory: {error}"),
        )
    })?;
    Ok(LegacyCase {
        kind,
        expectation_stem: arguments_path.to_owned(),
        relative_stem: arguments_path
            .strip_prefix(golden_root)
            .expect("legacy case is below golden root")
            .to_owned(),
        compiler_working_directory: canonical_directory.clone(),
        compiler_args,
        diagnostic_path_prefix: Some(format!("{}/", canonical_directory.display()).into_bytes()),
    })
}

#[cfg(test)]
mod tests {
    use super::{discover, LegacyKind};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn case_args_stops_recursion_and_preserves_one_argument_per_line() {
        let fixture = Fixture::new();
        let case = fixture.golden.join("run/modules");
        fs::create_dir_all(case.join("nested")).unwrap();
        fs::write(case.join("case.args"), "# comment\n --entry \napp::main\n").unwrap();
        fs::write(case.join("support.ska"), "support").unwrap();
        fs::write(case.join("nested/hidden.ska"), "hidden").unwrap();

        let cases = discover(&fixture.golden).unwrap();

        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].kind, LegacyKind::Run);
        assert_eq!(
            cases[0].relative_stem,
            PathBuf::from("run/modules/case.args")
        );
        assert_eq!(cases[0].compiler_args, ["--entry", "app::main"]);
        assert!(cases[0].diagnostic_path_prefix.is_some());
    }

    #[test]
    fn ordinary_sources_are_sorted_and_use_repository_relative_arguments() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.golden.join("compile_fail/z")).unwrap();
        fs::create_dir_all(fixture.golden.join("run")).unwrap();
        fs::write(fixture.golden.join("compile_fail/z/b.ska"), "b").unwrap();
        fs::write(fixture.golden.join("run/a.ska"), "a").unwrap();

        let cases = discover(&fixture.golden).unwrap();

        assert_eq!(
            cases
                .iter()
                .map(|case| case.relative_stem.as_path())
                .collect::<Vec<_>>(),
            [
                PathBuf::from("compile_fail/z/b.ska"),
                PathBuf::from("run/a.ska")
            ]
        );
        assert_eq!(cases[1].compiler_args, ["tests/golden/run/a.ska"]);
        assert_eq!(cases[1].compiler_working_directory, fixture.repository);
    }

    struct Fixture {
        repository: PathBuf,
        golden: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let repository = std::env::temp_dir().join(format!(
                "skald-golden-legacy-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let golden = repository.join("tests/golden");
            fs::create_dir_all(&golden).unwrap();
            Self { repository, golden }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.repository);
        }
    }
}
