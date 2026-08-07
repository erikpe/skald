use skald_golden::{
    build_plan, compare_stream, decode_arguments, load_bytes, ExitExpectation, PlannedLeafKind,
    ResolvedStreamExpectation, ResolvedWorkingDirectory,
};
use std::{
    collections::HashSet,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

#[test]
fn adapts_single_file_sidecars_without_changing_bytes_or_missing_file_meaning() {
    let fixture = Fixture::new();
    fixture.write("run/io.ska", b"source");
    fixture.write("run/io.exit", b"failure\n");
    fixture.write("run/io.argv", b"space arg\0\xff\0");
    fixture.write("run/io.stdin", b"in\0\xff\n");
    fixture.write("run/io.stdout", b"out\r\n\xff");

    let plan = fixture.plan(&[OsString::from("--extra")]);

    assert_eq!(plan.specs()[0].id(), "run/io");
    assert_eq!(plan.specs()[0].relative_path(), "run/io.ska");
    let build = plan.build("run/io::default").unwrap();
    assert_eq!(
        build.compiler_args(),
        ["tests/golden/run/io.ska", "--extra"]
    );
    assert_eq!(
        build.compiler_working_directory(),
        Some(fixture.repository.as_path())
    );
    let leaf = plan.leaf("run/io::default::<run>").unwrap();
    let PlannedLeafKind::Run(run) = leaf.kind() else {
        panic!("expected run")
    };
    assert_eq!(decode_arguments(run.args()).unwrap().len(), 2);
    assert_eq!(load_bytes(run.stdin()).unwrap(), b"in\0\xff\n");
    assert_eq!(run.expectation().exit(), ExitExpectation::Failure);
    assert_exact_bytes(run.expectation().stdout(), b"out\r\n\xff");
    assert_exact_bytes(run.expectation().stderr(), b"");
    assert_eq!(
        run.cwd(),
        &ResolvedWorkingDirectory::Fixture(fixture.repository.clone())
    );
    assert_eq!(run.resources(), ["legacy-working-directory"]);
}

#[test]
fn adapts_case_args_as_one_case_and_strips_only_its_absolute_diagnostic_prefix() {
    let fixture = Fixture::new();
    fixture.write(
        "compile_fail/modules/case.args",
        b"# comment\n--entry\napp::main\n",
    );
    fixture.write("compile_fail/modules/main.ska", b"support");
    fixture.write("compile_fail/modules/nested/hidden.ska", b"support");
    fixture.write(
        "compile_fail/modules/case.stderr",
        b"error\n --> main.ska:1:1\n",
    );

    let plan = fixture.plan(&[]);

    assert_eq!(plan.leaves().len(), 1);
    let build = plan.build("compile_fail/modules/case::default").unwrap();
    assert_eq!(build.base_args(), ["--entry", "app::main"]);
    assert_eq!(
        build.compiler_working_directory(),
        Some(fixture.golden.join("compile_fail/modules").as_path())
    );
    let leaf = plan
        .leaf("compile_fail/modules/case::default::<compile>")
        .unwrap();
    let PlannedLeafKind::Compile(expectation) = leaf.kind() else {
        panic!("expected compile fail")
    };
    assert_exact_bytes(expectation.stderr(), b"error\n --> main.ska:1:1\n");
    let expected_prefix = format!("{}/", fixture.golden.join("compile_fail/modules").display());
    assert_eq!(
        expectation.stderr_prefix_to_strip(),
        Some(expected_prefix.as_bytes())
    );
}

#[test]
fn rejects_malformed_required_exit_and_exact_byte_argument_sidecars_during_planning() {
    let fixture = Fixture::new();
    fixture.write("run/bad.ska", b"source");
    fixture.write("run/bad.exit", b"256");
    let error = build_plan(&fixture.golden, &fixture.artifacts, &[]).unwrap_err();
    assert!(error.to_string().contains("outside 0..=255"));

    fixture.write("run/bad.exit", b"0");
    fixture.write("run/bad.argv", b"not-nul-terminated");
    let error = build_plan(&fixture.golden, &fixture.artifacts, &[]).unwrap_err();
    assert!(error.to_string().contains("must end with NUL"));
}

#[test]
fn repository_corpus_preserves_migrations_and_remaining_legacy_ownership() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let plan = build_plan(
        repository.join("tests/golden"),
        repository.join("build/golden-rust"),
        &[],
    )
    .unwrap();
    let legacy = plan
        .leaves()
        .iter()
        .filter(|leaf| {
            leaf.spec_relative_path().starts_with("run/")
                || leaf.spec_relative_path().starts_with("compile_fail/")
        })
        .collect::<Vec<_>>();
    let native = legacy
        .iter()
        .filter(|leaf| matches!(leaf.kind(), PlannedLeafKind::Run(_)))
        .count();
    let compile_fail = legacy.len() - native;

    assert_eq!(native, 104, "unexpected remaining legacy native count");
    assert_eq!(
        compile_fail, 124,
        "unexpected remaining legacy compile-fail count"
    );
    assert_eq!(legacy.len(), 228);

    let mappings = [
        ("process-runtime.tsv", 38),
        ("modules-standard-library.tsv", 22),
    ];
    let mut old_ids = HashSet::new();
    let mut new_ids = HashSet::new();
    for (name, expected_count) in mappings {
        let mapping =
            fs::read_to_string(repository.join("tests/golden/migrations").join(name)).unwrap();
        let pairs = mapping
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                line.split_once('\t')
                    .expect("migration rows must contain one tab")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            pairs.len(),
            expected_count,
            "incomplete migration map {name}"
        );
        for (old, new) in pairs {
            assert!(
                old_ids.insert(old.to_owned()),
                "legacy leaf mapped twice: {old}"
            );
            assert!(
                new_ids.insert(new.to_owned()),
                "spec leaf owns two legacy observations: {new}"
            );
            assert!(
                plan.leaf(old).is_none(),
                "legacy leaf remains discovered: {old}"
            );
            assert!(plan.leaf(new).is_some(), "migrated leaf is missing: {new}");
        }
    }

    assert_eq!(old_ids.len(), 60);
    assert_eq!(new_ids.len(), 60);
    assert!(
        plan.specs().iter().all(|spec| {
            !spec.relative_path().starts_with("modules/cases/")
                && !spec.relative_path().starts_with("standard_library/cases/")
                && !spec.relative_path().starts_with("static_fields/cases/")
                && !spec
                    .relative_path()
                    .starts_with("private_initializers/cases/")
        }),
        "module provider sources must not be discovered as specs"
    );
    assert_eq!(
        plan.leaves().len(),
        290,
        "migration must preserve the complete observation count"
    );
}

#[test]
fn repository_module_provider_roots_are_canonical_and_contained() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let golden = fs::canonicalize(repository.join("tests/golden")).unwrap();
    let plan = build_plan(&golden, repository.join("build/golden-rust"), &[]).unwrap();
    let migrated_prefixes = [
        "modules/resolution::",
        "standard_library/replacement::",
        "static_fields/modules::",
        "private_initializers/modules::",
    ];
    let mut provider_roots = 0;

    for build in plan.builds().iter().filter(|build| {
        migrated_prefixes
            .iter()
            .any(|prefix| build.id().starts_with(prefix))
    }) {
        for pair in build.compiler_args().windows(2) {
            let option = pair[0].to_str();
            if !matches!(option, Some("--module-root" | "--stdlib-root")) {
                continue;
            }
            provider_roots += 1;
            let path = Path::new(&pair[1]);
            assert!(
                path.is_absolute(),
                "provider root is not absolute: {path:?}"
            );
            assert!(
                path.starts_with(&golden),
                "provider root escaped golden tree: {path:?}"
            );
            assert_eq!(fs::canonicalize(path).unwrap(), path);
        }
    }

    assert_eq!(
        provider_roots, 28,
        "unexpected migrated provider-root count"
    );
}

fn assert_exact_bytes(expectation: &ResolvedStreamExpectation, expected: &[u8]) {
    let ResolvedStreamExpectation::Match {
        expected: source, ..
    } = expectation
    else {
        panic!("legacy streams must compare exactly")
    };
    assert_eq!(load_bytes(source).unwrap(), expected);
    assert!(compare_stream(expectation, expected).unwrap().is_ok());
}

struct Fixture {
    repository: PathBuf,
    golden: PathBuf,
    artifacts: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let repository = std::env::temp_dir().join(format!(
            "skald-golden-legacy-plan-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        let golden = repository.join("tests/golden");
        let artifacts = repository.join("build/golden");
        fs::create_dir_all(&golden).unwrap();
        fs::write(golden.join("config.toml"), "schema = 1\n").unwrap();
        Self {
            repository,
            golden,
            artifacts,
        }
    }

    fn write(&self, relative: &str, contents: &[u8]) {
        let path = self.golden.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn plan(&self, arguments: &[OsString]) -> skald_golden::TestPlan {
        build_plan(&self.golden, &self.artifacts, arguments).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.repository);
    }
}
