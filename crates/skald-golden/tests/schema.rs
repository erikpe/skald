use skald_golden::{
    parse_config, parse_spec, ArgSource, ByteSource, ExitExpectation, MatchMode, SchemaVersion,
    StreamExpectation, TestKind, WorkingDirectory,
};
use std::path::Path;

const SPEC_PATH: &str = "tests/golden/language/example.golden.toml";
const CONFIG_PATH: &str = "tests/golden/config.toml";

fn parse(contents: &str) -> skald_golden::Spec {
    parse_spec(SPEC_PATH, contents).expect("schema fixture should be valid")
}

fn assert_rejected(contents: &str, expected_field: &str) {
    let error = parse_spec(SPEC_PATH, contents).expect_err("schema fixture should be rejected");
    assert_eq!(error.spec_path(), Path::new(SPEC_PATH));
    assert!(
        error.field_path().contains(expected_field),
        "expected field path containing {expected_field:?}, got {error:?}"
    );
    assert!(error.to_string().starts_with(SPEC_PATH));
}

#[test]
fn parses_every_toml_example_from_the_frozen_design() {
    let config = parse_config(
        CONFIG_PATH,
        r#"
schema = 1

[variant.default]
compiler_args = []

[variant.optimized]
compiler_args = ["--optimize"]
"#,
    )
    .expect("variant example should parse");
    assert_eq!(config.schema(), SchemaVersion::V1);
    assert_eq!(config.variants().len(), 2);

    parse(
        r#"
schema = 1

[[test]]
name = "replacement_standard_library"
mode = "run"
source = "application/main.ska"
compiler_args = ["--stdlib-root", "replacement-sdk"]
variants = ["default"]

[[test.run]]
name = "default"
"#,
    );

    parse(
        r#"
schema = 1
[[test]]
name = "inline_stdin"
mode = "run"
source = "inline.ska"
[[test.run]]
name = "sample"
stdin = { inline = "sample input\n" }
"#,
    );

    parse(
        r#"
schema = 1
[[test]]
name = "file_stdin"
mode = "run"
source = "file.ska"
[[test.run]]
name = "sample"
stdin = { file = "data/full-input.bin" }
"#,
    );

    parse(
        r#"
schema = 1

[[test]]
name = "integer_division"
mode = "run"
source = "integer_division.ska"
variants = ["default", "optimized"]

[[test.run]]
name = "positive_values"
args = ["12", "3"]
expect = { exit = 0, stdout = { inline = "4\n" } }

[[test.run]]
name = "corpus"
stdin = { file = "data/division.stdin" }
expect = { exit = 0, stdout = { file = "data/division.stdout" } }
"#,
    );

    parse(
        r#"
schema = 1
[[test]]
name = "bounds"
mode = "run"
source = "bounds.ska"
[[test.run]]
name = "out_of_bounds"
[test.run.expect]
exit = "failure"
[test.run.expect.stderr]
match = "contains"
inline = "array index is out of bounds"
"#,
    );

    parse(
        r#"
schema = 1
[[test]]
name = "alias_mismatch"
mode = "compile-fail"
source = "alias_exact_type_mismatch.ska"
[test.expect.stderr]
match = "starts-with"
inline = """error[TYP005]: alias argument has type `Right`, expected `Left`
 --> tests/golden/aliases/alias_exact_type_mismatch.ska:8:13"""
"#,
    );

    parse(
        r#"
schema = 1
[[test]]
name = "round_trip"
mode = "run"
source = "round_trip.ska"
[[test.run]]
name = "file_round_trip"
args = ["{tmp:input}", "{tmp:output}"]
input_files = [{ name = "input", contents = { file = "data/payload.bin" } }]
expect = { exit = 0, output_files = [{ name = "output", contents = { file = "data/payload.bin" } }] }
"#,
    );
}

#[test]
fn represents_the_complete_frozen_schema_as_typed_data() {
    let spec = parse(
        r#"
schema = 1

[[test]]
name = "native"
mode = "run"
source = "program.ska"
compiler_args = ["--stdlib-root", "sdk"]
variants = ["default", "instrumented"]
timeout = 20
serial = true
resources = ["compiler-cache"]

[[test.run]]
name = "ordinary"
args = ["one", "two"]
stdin = { inline = "input\n" }
input_files = [{ name = "input", contents = { file = "data/input.bin" } }]
cwd = { fixture = "fixtures" }
env = { SKALD_CASE = "ordinary" }
timeout = 10
serial = true
resources = ["terminal"]
expect = { exit = "failure", stdout = { match = "exact", file = "expected.stdout" }, stderr = { match = "contains", inline = "panic" }, output_files = [{ name = "result", contents = { inline = "done" } }] }

[[test.run]]
name = "exact_arguments"
argv_file = "data/arguments.argv"
expect = { stdout = { ignore = true }, stderr = { match = "starts-with", file = "expected-prefix.stderr" } }

[[test]]
name = "rejected"
mode = "compile-fail"
compiler_args = ["--entry", "app::main", "--module-root", "."]
variants = ["default"]
timeout = 30
resources = ["compiler"]
[test.expect.stderr]
match = "starts-with"
file = "expected.stderr"
"#,
    );

    assert_eq!(spec.schema(), SchemaVersion::V1);
    let native = &spec.tests()[0];
    assert_eq!(native.name(), "native");
    assert_eq!(native.source().unwrap().to_str(), Some("program.ska"));
    assert_eq!(native.timeout_seconds(), Some(20));
    assert!(native.serial());
    assert_eq!(native.resources(), ["compiler-cache"]);

    let TestKind::Run(native) = native.kind() else {
        panic!("native test should have run data");
    };
    let ordinary = &native.runs()[0];
    assert_eq!(
        ordinary.args(),
        &ArgSource::Utf8(vec!["one".into(), "two".into()])
    );
    assert_eq!(ordinary.stdin(), &ByteSource::Inline("input\n".into()));
    assert_eq!(
        ordinary.cwd(),
        &WorkingDirectory::Fixture("fixtures".into())
    );
    assert_eq!(ordinary.timeout_seconds(), Some(10));
    assert_eq!(ordinary.expectation().exit(), ExitExpectation::Failure);
    assert_eq!(
        ordinary.expectation().stderr().matchers().unwrap()[0].mode(),
        MatchMode::Contains
    );
    assert_eq!(
        native.runs()[1].args(),
        &ArgSource::File("data/arguments.argv".into())
    );
    assert_eq!(
        native.runs()[1].expectation().stdout(),
        &StreamExpectation::Ignore
    );

    let TestKind::CompileFail(rejected) = spec.tests()[1].kind() else {
        panic!("rejected test should have compile-fail data");
    };
    assert_eq!(
        rejected.expectation().stderr().matchers().unwrap()[0].mode(),
        MatchMode::StartsWith
    );
}

#[test]
fn defaults_are_strict_and_explicit_in_the_typed_model() {
    let spec = parse(
        r#"
schema = 1
[[test]]
name = "defaults"
mode = "run"
source = "program.ska"
[[test.run]]
name = "default"
"#,
    );
    let test = &spec.tests()[0];
    assert_eq!(test.variants(), ["default"]);
    let TestKind::Run(run_test) = test.kind() else {
        panic!("expected run test");
    };
    let run = &run_test.runs()[0];
    assert_eq!(run.args(), &ArgSource::Utf8(Vec::new()));
    assert_eq!(run.stdin(), &ByteSource::Inline(String::new()));
    assert_eq!(run.cwd(), &WorkingDirectory::Private);
    assert_eq!(run.expectation().exit(), ExitExpectation::Code(0));
    assert_eq!(
        run.expectation().stdout(),
        &StreamExpectation::exact_empty()
    );
    assert_eq!(
        run.expectation().stderr(),
        &StreamExpectation::exact_empty()
    );
}

#[test]
fn accepts_every_match_mode_with_inline_or_file_data() {
    let mut runs = String::new();
    for (index, mode) in ["exact", "starts-with", "contains"].iter().enumerate() {
        runs.push_str(&format!(
            r#"
[[test.run]]
name = "inline_{index}"
expect = {{ stdout = {{ match = "{mode}", inline = "value" }} }}
[[test.run]]
name = "file_{index}"
expect = {{ stderr = {{ match = "{mode}", file = "expected-{index}.stderr" }} }}
"#
        ));
    }
    parse(&format!(
        r#"
schema = 1
[[test]]
name = "matrix"
mode = "run"
source = "matrix.ska"
{runs}
"#
    ));

    for mode in ["exact", "starts-with", "contains"] {
        parse(&format!(
            r#"
schema = 1
[[test]]
name = "compile_{mode}"
mode = "compile-fail"
source = "invalid.ska"
[test.expect.stderr]
match = "{mode}"
file = "expected.stderr"
"#
        ));
    }
}

#[test]
fn schema_two_accepts_singular_shorthand_and_matcher_lists_for_every_stream() {
    let spec = parse(
        r#"
schema = 2

[[test]]
name = "native"
mode = "run"
source = "native.ska"

[[test.run]]
name = "streams"
[test.run.expect.stdout]
matches = [
  { name = "header", match = "starts-with", inline = "start" },
  { match = "contains", file = "expected/result.stdout" },
]
[[test.run.expect.stderr.matches]]
name = "complete stderr"
file = "expected/native.stderr"

[[test]]
name = "rejected"
mode = "compile-fail"
source = "rejected.ska"
[test.expect.stdout]
matches = [
  { name = "progress", match = "contains", inline = "checking" },
  { match = "exact", file = "expected/compiler.stdout" },
]
[[test.expect.stderr.matches]]
name = "first diagnostic"
match = "contains"
inline = "error[ONE]"
[[test.expect.stderr.matches]]
name = "diagnostic prefix"
match = "starts-with"
file = "expected/compiler-prefix.stderr"
"#,
    );

    assert_eq!(spec.schema(), SchemaVersion::V2);
    let TestKind::Run(native) = spec.tests()[0].kind() else {
        panic!("expected a native test");
    };
    let stdout = native.runs()[0].expectation().stdout().matchers().unwrap();
    assert_eq!(stdout.len(), 2);
    assert_eq!(stdout[0].name(), Some("header"));
    assert_eq!(stdout[0].mode(), MatchMode::StartsWith);
    assert_eq!(stdout[1].name(), None);
    assert_eq!(stdout[1].mode(), MatchMode::Contains);
    let stderr = native.runs()[0].expectation().stderr().matchers().unwrap();
    assert_eq!(stderr.len(), 1);
    assert_eq!(stderr[0].mode(), MatchMode::Exact);

    let TestKind::CompileFail(rejected) = spec.tests()[1].kind() else {
        panic!("expected a compile-fail test");
    };
    assert_eq!(rejected.expectation().stdout().matchers().unwrap().len(), 2);
    assert_eq!(rejected.expectation().stderr().matchers().unwrap().len(), 2);
}

#[test]
fn schema_two_compile_fail_stdout_defaults_to_exact_empty() {
    let spec = parse(
        r#"
schema = 2
[[test]]
name = "rejected"
mode = "compile-fail"
source = "rejected.ska"
expect = { stderr = { inline = "error" } }
"#,
    );
    let TestKind::CompileFail(rejected) = spec.tests()[0].kind() else {
        panic!("expected a compile-fail test");
    };
    assert_eq!(
        rejected.expectation().stdout(),
        &StreamExpectation::exact_empty()
    );
}

#[test]
fn schema_one_rejects_matcher_lists_and_compile_fail_stdout() {
    let cases = [
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\nexpect={stdout={matches=[{inline='x'}]}}",
            "stdout.matches",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='compile-fail'\nsource='x'\nexpect={stdout={inline='progress'},stderr={inline='error'}}",
            "expect.stdout",
        ),
    ];
    for (contents, field) in cases {
        assert_rejected(contents, field);
    }

    let error = parse_config(CONFIG_PATH, "schema=2")
        .expect_err("repository configuration must remain schema version 1");
    assert_eq!(error.field_path(), "schema");
}

#[test]
fn rejects_invalid_matcher_list_shapes_and_names() {
    let cases = [
        ("expect={stdout={matches=[]}}", "stdout.matches"),
        (
            "expect={stdout={inline='x',matches=[{inline='x'}]}}",
            "stdout",
        ),
        ("expect={stdout={matches=[{}]}}", "matches[0]"),
        (
            "expect={stdout={matches=[{inline='x',file='x'}]}}",
            "matches[0]",
        ),
        (
            "expect={stdout={matches=[{name='',inline='x'}]}}",
            "matches[0].name",
        ),
        (
            "expect={stdout={matches=[{name='same',inline='x'},{name='same',inline='y'}]}}",
            "matches[1].name",
        ),
        (
            "expect={stdout={matches=[{match='contains',inline=''}]}}",
            "matches[0].inline",
        ),
    ];
    for (tail, field) in cases {
        assert_rejected(
            &format!(
                "schema=2\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\n{tail}"
            ),
            field,
        );
    }
}

#[test]
fn rejects_unknown_keys_at_every_schema_level() {
    let cases = [
        ("schema = 1\nunknown = true", "unknown"),
        (
            "schema = 1\n[[test]]\nname='x'\nmode='run'\nsource='x.ska'\nunknown=true",
            "test[0]",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x.ska'\n[[test.run]]\nname='r'\nunknown=true",
            "test[0].run[0]",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x.ska'\n[[test.run]]\nname='r'\nstdin={inline='x',unknown=true}",
            "stdin",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x.ska'\n[[test.run]]\nname='r'\ninput_files=[{name='f',contents={inline='x'},unknown=true}]",
            "input_files",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x.ska'\n[[test.run]]\nname='r'\ncwd={fixture='.',unknown=true}",
            "cwd",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x.ska'\n[[test.run]]\nname='r'\nexpect={unknown=true}",
            "expect",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x.ska'\n[[test.run]]\nname='r'\nexpect={stdout={inline='x',unknown=true}}",
            "stdout",
        ),
        (
            "schema=2\n[[test]]\nname='x'\nmode='run'\nsource='x.ska'\n[[test.run]]\nname='r'\nexpect={stdout={matches=[{inline='x',unknown=true}]}}",
            "stdout.matches[0]",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x.ska'\n[[test.run]]\nname='r'\nexpect={output_files=[{name='f',contents={inline='x'},unknown=true}]}",
            "output_files",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='compile-fail'\nsource='x.ska'\nexpect={stderr={inline='x'},unknown=true}",
            "expect",
        ),
    ];

    for (contents, field) in cases {
        assert_rejected(contents, field);
    }

    let error = parse_config(
        CONFIG_PATH,
        "schema=1\n[variant.default]\ncompiler_args=[]\nunknown=true",
    )
    .expect_err("unknown variant field should fail");
    assert!(error.field_path().contains("variant.default"));

    let error = parse_config(CONFIG_PATH, "schema=1\nunknown=true")
        .expect_err("unknown config field should fail");
    assert!(error.field_path().contains("unknown"));
}

#[test]
fn rejects_versions_duplicates_and_empty_collections() {
    let cases = [
        ("schema=3\n[[test]]\nname='x'\nmode='run'\nsource='x'", "schema"),
        ("schema=1", "test"),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\n[[test]]\nname='x'\nmode='run'\nsource='y'\n[[test.run]]\nname='r'",
            "test[1].name",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'",
            "test[0].run",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\nvariants=[]",
            "variants",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\nvariants=['default','default']",
            "variants[1]",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\nresources=[]",
            "resources",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\ninput_files=[]",
            "input_files",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\nexpect={output_files=[]}",
            "output_files",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\nresources=['shared','shared']",
            "resources[1]",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\nexpect={output_files=[{name='same',contents={inline='x'}},{name='same',contents={inline='y'}}]}",
            "output_files[1].name",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\n[[test.run]]\nname='r'",
            "run[1].name",
        ),
    ];
    for (contents, field) in cases {
        assert_rejected(contents, field);
    }

    let duplicate_variant = parse_config(CONFIG_PATH, "schema=1\n[variant.same]\n[variant.same]")
        .expect_err("duplicate variants should fail TOML decoding");
    assert_eq!(duplicate_variant.field_path(), "<document>");
}

#[test]
fn rejects_mode_incompatible_and_mutually_exclusive_fields() {
    let cases = [
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\nexpect={stderr={inline='x'}}\n[[test.run]]\nname='r'",
            "test[0].expect",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='compile-fail'\nsource='x'\nexpect={stderr={inline='x'}}\n[[test.run]]\nname='r'",
            "test[0].run",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='compile-fail'\nsource='x'",
            "test[0].expect",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='compile-fail'\nsource='x'\nexpect={}",
            "expect.stderr",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\ncompiler_args=['--entry','app']\n[[test.run]]\nname='r'",
            "source",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\n[[test.run]]\nname='r'",
            "source",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\ncompiler_args=['--entry']\n[[test.run]]\nname='r'",
            "compiler_args[1]",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\nargs=[]\nargv_file='args.bin'",
            "args",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\nstdin={inline='x',file='x.bin'}",
            "stdin",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\nstdin={}",
            "stdin",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\ninput_files=[{name='input',contents={}}]",
            "input_files[0].contents",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\nexpect={output_files=[{name='output',contents={inline='x',file='x'}}]}",
            "output_files[0].contents",
        ),
    ];
    for (contents, field) in cases {
        assert_rejected(contents, field);
    }
}

#[test]
fn rejects_missing_empty_or_ignored_expected_data() {
    let cases = [
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\nexpect={stdout={}}",
            "stdout",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\nexpect={stdout={match='starts-with',inline=''}}",
            "stdout.inline",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\nexpect={stderr={match='contains',inline=''}}",
            "stderr.inline",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\nexpect={stderr={ignore=false}}",
            "stderr.ignore",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\nexpect={stderr={ignore=true,inline='x'}}",
            "stderr",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='compile-fail'\nsource='x'\nexpect={stderr={ignore=true}}",
            "stderr.ignore",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='compile-fail'\nsource='x'\nexpect={stderr={inline=''}}",
            "stderr.inline",
        ),
    ];
    for (contents, field) in cases {
        assert_rejected(contents, field);
    }
}

#[test]
fn rejects_invalid_names_timeouts_and_process_values() {
    let cases = [
        (
            "schema=1\n[[test]]\nname=''\nmode='run'\nsource='x'",
            "test[0].name",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\ntimeout=0",
            "test[0].timeout",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\ntimeout=0",
            "run[0].timeout",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\nresources=['']\n[[test.run]]\nname='r'",
            "resources[0]",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\ninput_files=[{name='../bad',contents={inline='x'}}]",
            "input_files[0].name",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\ninput_files=[{name='same',contents={inline='x'}},{name='same',contents={inline='y'}}]",
            "input_files[1].name",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\nargv_file=''",
            "argv_file",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\ncwd={fixture=''}",
            "cwd.fixture",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\nstdin={file=''}",
            "stdin.file",
        ),
        (
            "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname='r'\nexpect={exit='success'}",
            "expect.exit",
        ),
    ];
    for (contents, field) in cases {
        assert_rejected(contents, field);
    }
}

#[test]
fn reports_toml_syntax_and_type_errors_with_a_spec_and_field_path() {
    assert_rejected("schema = [", "<document>");
    assert_rejected(
        "schema=1\n[[test]]\nname='x'\nmode='run'\nsource='x'\n[[test.run]]\nname=1",
        "test[0].run[0].name",
    );
}
