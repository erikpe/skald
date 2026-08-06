use skald_golden::{
    build_plan, select, PlannedLeafKind, ResolvedArgs, ResolvedByteSource,
    ResolvedWorkingDirectory, SelectionOptions, TestPlan,
};
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    base: PathBuf,
    root: PathBuf,
    artifacts: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "skald-golden-planning-{}-{sequence}",
            std::process::id()
        ));
        let root = base.join("golden");
        let artifacts = base.join("artifacts");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.toml"), "schema = 1\n").unwrap();
        Self {
            base,
            root,
            artifacts,
        }
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn bytes(&self, relative: &str, contents: &[u8]) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn directory(&self, relative: &str) {
        fs::create_dir_all(self.root.join(relative)).unwrap();
    }

    fn configure(&self, contents: &str) {
        fs::write(self.root.join("config.toml"), contents).unwrap();
    }

    fn plan(&self) -> TestPlan {
        self.plan_with_args(&[])
    }

    fn plan_with_args(&self, arguments: &[OsString]) -> TestPlan {
        build_plan(&self.root, &self.artifacts, arguments).expect("fixture should plan")
    }

    fn canonical(&self, relative: &str) -> PathBuf {
        fs::canonicalize(self.root.join(relative)).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.base).unwrap();
    }
}

fn simple_run(name: &str, source: &str, run: &str) -> String {
    format!(
        "schema=1\n[[test]]\nname={name:?}\nmode='run'\nsource={source:?}\n[[test.run]]\nname={run:?}\n"
    )
}

#[test]
fn discovers_nested_specs_in_stable_identity_order() {
    let fixture = Fixture::new();
    fixture.write("z.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write("z.golden.toml", &simple_run("zeta", "z.ska", "last"));
    fixture.write("a/source.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write(
        "a/nested/first.golden.toml",
        &simple_run("alpha", "../source.ska", "first"),
    );

    let plan = fixture.plan();
    assert_eq!(
        plan.specs()
            .iter()
            .map(|spec| spec.id())
            .collect::<Vec<_>>(),
        ["a/nested/first", "z"]
    );
    assert_eq!(
        plan.leaves()
            .iter()
            .map(|leaf| leaf.id())
            .collect::<Vec<_>>(),
        [
            "a/nested/first::alpha::default::first",
            "z::zeta::default::last"
        ]
    );
    assert_eq!(
        plan.specs()[0].relative_path(),
        "a/nested/first.golden.toml"
    );
    assert!(!fixture.artifacts.exists());

    let repeated = fixture.plan();
    assert_eq!(plan, repeated);
}

#[test]
fn expands_variants_runs_and_compiler_arguments_in_contract_order() {
    let fixture = Fixture::new();
    fixture.configure(
        r#"
schema = 1
[variant.default]
compiler_args = ["--target", "x86_64-sysv"]
[variant.checked]
compiler_args = ["--module-root", "variant-root", "--variant-flag"]
"#,
    );
    fixture.write("feature/program.ska", "fn main() -> i64 { return 0; }\n");
    fixture.directory("feature/base-root");
    fixture.directory("feature/variant-root");
    fixture.directory("feature/cli-sdk");
    fixture.write(
        "feature/matrix.golden.toml",
        r#"
schema = 1
[[test]]
name = "matrix"
mode = "run"
source = "program.ska"
compiler_args = ["--module-root", "base-root", "--unknown-base"]
variants = ["checked", "default"]
[[test.run]]
name = "small"
[[test.run]]
name = "large"
"#,
    );

    let cli = [
        OsString::from("--stdlib-root"),
        OsString::from("cli-sdk"),
        OsString::from("--unknown-cli"),
    ];
    let plan = fixture.plan_with_args(&cli);
    assert_eq!(plan.tests().len(), 1);
    assert_eq!(plan.builds().len(), 2);
    assert_eq!(plan.leaves().len(), 4);

    let checked = plan
        .build("feature/matrix::matrix::checked")
        .expect("checked build should exist");
    let expected = vec![
        fixture.canonical("feature/program.ska").into_os_string(),
        "--module-root".into(),
        fixture.canonical("feature/base-root").into_os_string(),
        "--unknown-base".into(),
        "--module-root".into(),
        fixture.canonical("feature/variant-root").into_os_string(),
        "--variant-flag".into(),
        "--stdlib-root".into(),
        fixture.canonical("feature/cli-sdk").into_os_string(),
        "--unknown-cli".into(),
    ];
    assert_eq!(checked.compiler_args(), expected);
    assert_eq!(
        checked.leaf_ids(),
        [
            "feature/matrix::matrix::checked::large",
            "feature/matrix::matrix::checked::small"
        ]
    );
    assert_eq!(
        plan.test("feature/matrix::matrix").unwrap().build_ids(),
        [
            "feature/matrix::matrix::checked",
            "feature/matrix::matrix::default"
        ]
    );
}

#[test]
fn resolves_logical_entries_without_rewriting_unknown_arguments() {
    let fixture = Fixture::new();
    fixture.directory("modules/root");
    fixture.write(
        "modules/logical.golden.toml",
        r#"
schema = 1
[[test]]
name = "logical"
mode = "compile-fail"
compiler_args = ["--entry", "app::main", "--module-root", "root", "--future-flag", "literal"]
[test.expect.stderr]
inline = "error"
"#,
    );

    let plan = fixture.plan();
    let test = plan.test("modules/logical::logical").unwrap();
    assert!(test.source().is_none());
    assert!(test.source_relative().is_none());
    let build = plan.build("modules/logical::logical::default").unwrap();
    assert_eq!(
        build.compiler_args(),
        [
            OsString::from("--entry"),
            OsString::from("app::main"),
            OsString::from("--module-root"),
            fixture.canonical("modules/root").into_os_string(),
            OsString::from("--future-flag"),
            OsString::from("literal"),
        ]
    );
}

#[test]
fn canonicalizes_every_path_bearing_fixture_field() {
    let fixture = Fixture::new();
    fixture.write("all/program.ska", "fn main() -> i64 { return 0; }\n");
    fixture.bytes("all/data/args.argv", b"one\0");
    fixture.bytes("all/data/stdin.bin", b"input");
    fixture.bytes("all/data/input.bin", b"payload");
    fixture.bytes("all/data/stdout.bin", b"output");
    fixture.bytes("all/data/stderr.bin", b"warning");
    fixture.bytes("all/data/output.bin", b"result");
    fixture.directory("all/fixture-cwd");
    fixture.write(
        "all/all.golden.toml",
        r#"
schema = 1
[[test]]
name = "all"
mode = "run"
source = "program.ska"
[[test.run]]
name = "paths"
argv_file = "data/args.argv"
stdin = { file = "data/stdin.bin" }
input_files = [{ name = "input", contents = { file = "data/input.bin" } }]
cwd = { fixture = "fixture-cwd" }
expect = { stdout = { file = "data/stdout.bin" }, stderr = { match = "contains", file = "data/stderr.bin" }, output_files = [{ name = "output", contents = { file = "data/output.bin" } }] }
"#,
    );

    let plan = fixture.plan();
    let PlannedLeafKind::Run(run) = plan.leaves()[0].kind() else {
        panic!("expected run leaf");
    };
    assert_eq!(
        run.args(),
        &ResolvedArgs::File(fixture.canonical("all/data/args.argv"))
    );
    assert_eq!(
        run.stdin(),
        &ResolvedByteSource::File(fixture.canonical("all/data/stdin.bin"))
    );
    assert_eq!(
        run.input_files()[0].contents(),
        &ResolvedByteSource::File(fixture.canonical("all/data/input.bin"))
    );
    assert_eq!(
        run.cwd(),
        &ResolvedWorkingDirectory::Fixture(fixture.canonical("all/fixture-cwd"))
    );
    assert_eq!(
        run.expectation().stdout().expected(),
        Some(&ResolvedByteSource::File(
            fixture.canonical("all/data/stdout.bin")
        ))
    );
    assert_eq!(
        run.expectation().stderr().expected(),
        Some(&ResolvedByteSource::File(
            fixture.canonical("all/data/stderr.bin")
        ))
    );
    assert_eq!(
        run.expectation().output_files()[0].contents(),
        &ResolvedByteSource::File(fixture.canonical("all/data/output.bin"))
    );
}

#[test]
fn rejects_missing_lexically_escaping_and_wrong_kind_paths() {
    let cases = [
        ("source='missing.ska'", "source"),
        ("source='../../outside.ska'", "source"),
        ("source='directory'", "source"),
    ];
    for (source, field) in cases {
        let fixture = Fixture::new();
        fixture.directory("case/directory");
        fixture.write(
            "case/invalid.golden.toml",
            &format!(
                "schema=1\n[[test]]\nname='invalid'\nmode='run'\n{source}\n[[test.run]]\nname='run'\n"
            ),
        );
        let error = build_plan(&fixture.root, &fixture.artifacts, &[]).unwrap_err();
        assert!(error.field().unwrap().contains(field), "{error}");
    }

    let fixture = Fixture::new();
    fixture.write("case/program.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write(
        "case/invalid.golden.toml",
        &format!(
            "schema=1\n[[test]]\nname='invalid'\nmode='run'\nsource={:?}\n[[test.run]]\nname='run'\n",
            fixture.canonical("case/program.ska").display().to_string()
        ),
    );
    let error = build_plan(&fixture.root, &fixture.artifacts, &[]).unwrap_err();
    assert!(error
        .message_text()
        .contains("relative to the spec directory"));
}

#[cfg(unix)]
#[test]
fn rejects_symlink_escapes_but_accepts_contained_symlinks() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    let outside = fixture.base.join("outside.ska");
    fs::write(&outside, "fn main() -> i64 { return 0; }\n").unwrap();
    fixture.write("case/inside.ska", "fn main() -> i64 { return 0; }\n");
    symlink(&outside, fixture.root.join("case/escape.ska")).unwrap();
    symlink("inside.ska", fixture.root.join("case/alias.ska")).unwrap();
    fixture.write(
        "case/escape.golden.toml",
        &simple_run("escape", "escape.ska", "run"),
    );
    let error = build_plan(&fixture.root, &fixture.artifacts, &[]).unwrap_err();
    assert!(error.message_text().contains("outside golden root"));

    fs::remove_file(fixture.root.join("case/escape.golden.toml")).unwrap();
    fixture.write(
        "case/inside.golden.toml",
        &simple_run("inside", "alias.ska", "run"),
    );
    let plan = fixture.plan();
    assert_eq!(
        plan.tests()[0].source(),
        Some(fixture.canonical("case/inside.ska").as_path())
    );
}

#[test]
fn rejects_empty_external_partial_and_compile_expectations() {
    let fixtures = [
        (
            "run",
            "[[test.run]]\nname='r'\nexpect={stderr={match='contains',file='empty'}}",
        ),
        ("compile-fail", "[test.expect.stderr]\nfile='empty'"),
    ];
    for (mode, tail) in fixtures {
        let fixture = Fixture::new();
        fixture.write("case/program.ska", "fn main() -> i64 { return 0; }\n");
        fixture.bytes("case/empty", b"");
        fixture.write(
            "case/empty.golden.toml",
            &format!(
                "schema=1\n[[test]]\nname='empty'\nmode={mode:?}\nsource='program.ska'\n{tail}\n"
            ),
        );
        let error = build_plan(&fixture.root, &fixture.artifacts, &[]).unwrap_err();
        assert!(error.message_text().contains("must not be empty"));
    }
}

#[test]
fn detects_ambiguous_ids_and_keeps_flattened_artifact_names_distinct() {
    let fixture = Fixture::new();
    fixture.configure("schema=1\n[variant.c]\n[variant.'b::c']\n");
    fixture.write("program.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write(
        "duplicate.golden.toml",
        r#"
schema=1
[[test]]
name='a'
mode='run'
source='program.ska'
variants=['b::c']
[[test.run]]
name='r'
[[test]]
name='a::b'
mode='run'
source='program.ska'
variants=['c']
[[test.run]]
name='r'
"#,
    );
    let error = build_plan(&fixture.root, &fixture.artifacts, &[]).unwrap_err();
    assert!(error.message_text().contains("duplicate build ID"));

    fs::remove_file(fixture.root.join("duplicate.golden.toml")).unwrap();
    fixture.write(
        "collision.golden.toml",
        r#"
schema=1
[[test]]
name='a/b'
mode='run'
source='program.ska'
[[test.run]]
name='r'
[[test]]
name='a_b'
mode='run'
source='program.ska'
[[test.run]]
name='r'
"#,
    );
    let plan = fixture.plan();
    let paths = plan
        .builds()
        .iter()
        .map(|build| build.artifact_directory())
        .collect::<Vec<_>>();
    assert_ne!(paths[0], paths[1]);
    let prefixes = paths
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy())
        .map(|name| name.rsplit_once('-').unwrap().0.to_owned())
        .collect::<Vec<_>>();
    assert_eq!(prefixes[0], prefixes[1]);
}

fn selection_fixture() -> Fixture {
    let fixture = Fixture::new();
    fixture.configure("schema=1\n[variant.default]\n[variant.optimized]\n");
    fixture.write(
        "language/numbers/program.ska",
        "fn main() -> i64 { return 0; }\n",
    );
    fixture.write(
        "language/numbers/basic.golden.toml",
        r#"
schema=1
[[test]]
name='calculate'
mode='run'
source='program.ska'
variants=['default','optimized']
[[test.run]]
name='small'
[[test.run]]
name='large'
[[test]]
name='invalid'
mode='compile-fail'
source='program.ska'
[test.expect.stderr]
inline='error'
"#,
    );
    fixture.write("modules/program.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write(
        "modules/imports.golden.toml",
        &simple_run("imports", "program.ska", "default"),
    );
    fixture
}

#[test]
fn selects_with_component_and_recursive_globs_exact_ids_and_variants() {
    let fixture = selection_fixture();
    let plan = fixture.plan();
    assert_eq!(
        select(&plan, &SelectionOptions::default())
            .unwrap()
            .leaves()
            .len(),
        6
    );
    assert_eq!(
        select(&plan, &SelectionOptions::default().include("language/**"))
            .unwrap()
            .leaves()
            .len(),
        5
    );
    assert_eq!(
        select(
            &plan,
            &SelectionOptions::default()
                .include("language/**")
                .include("modules/**")
                .exclude("**::optimized::*")
        )
        .unwrap()
        .leaves()
        .len(),
        4
    );
    assert_eq!(
        select(
            &plan,
            &SelectionOptions::default().include("language/numbers/*.ska")
        )
        .unwrap()
        .leaves()
        .len(),
        5
    );
    assert_eq!(
        select(
            &plan,
            &SelectionOptions::default().include("language/numbers/*.golden.toml")
        )
        .unwrap()
        .leaves()
        .len(),
        5
    );
    assert!(select(&plan, &SelectionOptions::default().include("language/*")).is_err());
    assert_eq!(
        select(&plan, &SelectionOptions::default().variant("optimized"))
            .unwrap()
            .leaves()
            .len(),
        2
    );
    let exact = "language/numbers/basic::calculate::default::small";
    assert_eq!(
        select(&plan, &SelectionOptions::default().exact(exact))
            .unwrap()
            .leaves()[0]
            .id(),
        exact
    );
}

#[test]
fn rejects_empty_selection_unless_it_is_explicitly_allowed() {
    let fixture = selection_fixture();
    let plan = fixture.plan();
    let empty = SelectionOptions::default().include("absent/**");
    assert!(select(&plan, &empty)
        .unwrap_err()
        .message()
        .contains("no golden-test leaves"));
    assert!(select(&plan, &empty.clone().allow_empty(true))
        .unwrap()
        .leaves()
        .is_empty());
    assert!(
        select(&plan, &SelectionOptions::default().exact("missing::leaf"))
            .unwrap_err()
            .message()
            .contains("unknown exact")
    );
    assert!(select(
        &plan,
        &SelectionOptions::default()
            .exact("language/numbers/basic::calculate::default::small")
            .include("language/**")
    )
    .is_err());
}

#[test]
fn list_and_list_tests_are_stable_and_explain_is_complete() {
    let fixture = selection_fixture();
    let plan = fixture.plan();
    let exact = "modules/imports::imports::default::default";
    let selected = select(&plan, &SelectionOptions::default().exact(exact)).unwrap();
    assert_eq!(selected.list(), format!("{exact}\n"));
    assert_eq!(
        selected.list_tests(),
        "test  modules/imports::imports\nbuild modules/imports::imports::default\n"
    );

    let explanation = selected.explain(exact).unwrap();
    let source = fixture.canonical("modules/program.ska");
    let artifact = plan
        .build("modules/imports::imports::default")
        .unwrap()
        .artifact_directory();
    let expected = format!(
        "id = {exact}\n\
spec = modules/imports.golden.toml\n\
test = modules/imports::imports\n\
build = modules/imports::imports::default\n\
variant = default\n\
source = {}\n\
artifact-directory = {}\n\
base-args = [\"{}\"]\n\
variant-args = []\n\
command-line-args = []\n\
compile-timeout = None\n\
compile-serial = false\n\
compile-resources = []\n\
kind = run\n\
run = default\n\
args = []\n\
stdin = inline \"\"\n\
cwd = <private>\n\
env = {{}}\n\
run-timeout = None\n\
run-serial = false\n\
run-resources = []\n\
exit = Code(0)\n\
stdout = Exact inline \"\"\n\
stderr = Exact inline \"\"\n\
dependencies = [\"modules/imports::imports::default\"]\n",
        source.display(),
        artifact.display(),
        escaped_os(source.as_os_str())
    );
    assert_eq!(explanation, expected);
    assert!(!fixture.artifacts.exists());
}

#[test]
fn malformed_unselected_specs_fail_before_filtering() {
    let fixture = Fixture::new();
    fixture.write("good.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write("good.golden.toml", &simple_run("good", "good.ska", "run"));
    fixture.write(
        "bad.golden.toml",
        "schema=1\n[[test]]\nname='bad'\nmode='run'\nsource='missing.ska'\n[[test.run]]\nname='run'\n",
    );

    let error = build_plan(&fixture.root, &fixture.artifacts, &[]).unwrap_err();
    assert!(error.path().unwrap().ends_with("bad.golden.toml"));
    assert!(error.field().unwrap().contains("source"));
}

#[test]
fn missing_configuration_fails_before_discovery_can_succeed() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.root.join("config.toml")).unwrap();
    let error = build_plan(&fixture.root, &fixture.artifacts, &[]).unwrap_err();
    assert!(error.path().unwrap().ends_with("config.toml"));
}

fn escaped_os(value: &OsStr) -> String {
    let mut output = String::new();
    for byte in value.as_encoded_bytes() {
        for escaped in std::ascii::escape_default(*byte) {
            output.push(char::from(escaped));
        }
    }
    output
}
