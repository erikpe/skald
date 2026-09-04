use super::*;
use std::os::unix::ffi::OsStringExt;

#[test]
fn help_version_and_mir_pass_listing_are_available_without_compilation() {
    let (exit_code, stdout, stderr) = run(&["skac", "--help"]);
    assert_eq!(exit_code, 0);
    assert_eq!(stdout, format!("{HELP}\n"));
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run(&["skac", "--version"]);
    assert_eq!(exit_code, 0);
    assert_eq!(stdout, format!("skac {}\n", env!("CARGO_PKG_VERSION")));
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run(&["skac", "--list-mir-passes"]);
    assert_eq!(exit_code, 0);
    assert_eq!(
        stdout,
        "Available final-MIR passes:\n  checked-integer-constant-folding\n      Folds exact successful checked-integer constant protocols.\n  conservative-cfg-cleanup\n      Folds ordinary branches and removes unprotected unreachable MIR blocks.\n  dead-pure-definition-elimination\n      Removes unused non-failing scalar MIR definitions.\n  primitive-algebraic-simplification\n      Simplifies exact primitive MIR algebraic identities.\n  primitive-constant-folding\n      Folds exact block-local primitive MIR constants.\n  whole-world-reachability\n      Removes unreachable executable MIR definitions.\n"
    );
    assert!(stderr.is_empty());
}

#[test]
fn invalid_arguments_are_usage_errors() {
    let (exit_code, stdout, stderr) = run(&["skac"]);
    assert_eq!(exit_code, EXIT_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.starts_with("skac: exactly one file or logical module entry is required\n"));

    let (exit_code, _, stderr) = run(&["skac", "test.ska", "--emit", "object"]);
    assert_eq!(exit_code, EXIT_USAGE);
    assert!(stderr.contains("unsupported emission kind `object`; expected `asm`"));

    let (exit_code, _, stderr) = run(&["skac", "test.txt"]);
    assert_eq!(exit_code, EXIT_USAGE);
    assert!(stderr.contains("input must use the canonical `.ska` suffix"));

    let (exit_code, _, stderr) = run(&["skac", "test.ska", "--target", "unknown"]);
    assert_eq!(exit_code, EXIT_USAGE);
    assert!(stderr.contains("unsupported target `unknown`"));
}

#[test]
fn mir_optimization_cli_selection_is_typed_and_precedes_source_io() {
    let (exit_code, stdout, stderr) = run(&["skac", "missing.ska", "--mir-optimization", "none"]);
    assert_eq!(exit_code, EXIT_COMPILE_ERROR);
    assert!(stdout.is_empty());
    assert!(stderr.contains("error[MOD001]: invalid entry"));

    let cases = [
        (
            vec!["skac", "missing.ska", "--disable-mir-pass", "unknown-pass"],
            "unknown MIR pass name: `unknown-pass`; known MIR passes: `checked-integer-constant-folding`, `conservative-cfg-cleanup`, `dead-pure-definition-elimination`, `primitive-algebraic-simplification`, `primitive-constant-folding`, `whole-world-reachability`",
        ),
        (
            vec!["skac", "--disable-mir-pass", "unknown-pass", "missing.ska"],
            "unknown MIR pass name: `unknown-pass`; known MIR passes: `checked-integer-constant-folding`, `conservative-cfg-cleanup`, `dead-pure-definition-elimination`, `primitive-algebraic-simplification`, `primitive-constant-folding`, `whole-world-reachability`",
        ),
        (
            vec!["skac", "missing.ska", "--mir-optimization", "fast"],
            "invalid MIR optimization profile `fast`",
        ),
        (
            vec!["skac", "missing.ska", "--mir-optimization"],
            "expected `none` or `default` after `--mir-optimization`",
        ),
        (
            vec!["skac", "missing.ska", "--disable-mir-pass"],
            "expected a registered MIR pass name after `--disable-mir-pass`",
        ),
    ];

    for (arguments, expected) in cases {
        let (exit_code, stdout, stderr) = run(&arguments);
        assert_eq!(exit_code, EXIT_USAGE, "{arguments:?}: {stderr}");
        assert!(stdout.is_empty());
        assert!(stderr.contains(expected), "{arguments:?}: {stderr}");
        assert!(!stderr.contains("error[MOD001]"), "{arguments:?}: {stderr}");
    }
}

#[test]
fn module_arguments_are_order_independent_and_selector_conflicts_are_usage_errors() {
    let cases = [
        (
            vec!["skac", "main.ska", "--entry", "app::main"],
            "file and logical module entries are mutually exclusive",
        ),
        (
            vec!["skac", "--entry", "app::main", "--entry", "other::main"],
            "entry option specified more than once",
        ),
        (
            vec!["skac", "--entry", "app..main"],
            "invalid entry module path",
        ),
        (
            vec!["skac", "one.ska", "two.ska"],
            "more than one positional input file",
        ),
        (
            vec!["skac", "main.ska", "--stdlib-root", "std", "--no-stdlib"],
            "replacement standard-library root and disabled standard library are mutually exclusive",
        ),
        (
            vec![
                "skac",
                "main.ska",
                "--stdlib-root",
                "one",
                "--stdlib-root",
                "two",
            ],
            "standard-library root specified more than once",
        ),
        (
            vec!["skac", "main.ska", "--no-stdlib", "--no-stdlib"],
            "no-stdlib option specified more than once",
        ),
        (
            vec![
                "skac",
                "main.ska",
                "--omit-runtime-trace",
                "--omit-runtime-trace",
            ],
            "omit-runtime-trace option specified more than once",
        ),
    ];

    for (arguments, expected) in cases {
        let (exit_code, stdout, stderr) = run(&arguments);
        assert_eq!(exit_code, EXIT_USAGE, "{arguments:?}: {stderr}");
        assert!(stdout.is_empty());
        assert!(stderr.contains(expected), "{arguments:?}: {stderr}");
    }
}

#[test]
fn output_defaults_follow_the_selected_entry_form_and_artifact_kind() {
    let positional = EntrySelector::File("app/main.ska".into());
    assert_eq!(
        default_output_path(&positional, ArtifactKind::Executable),
        PathBuf::from("app/main")
    );
    assert_eq!(
        default_output_path(&positional, ArtifactKind::Assembly),
        PathBuf::from("app/main.s")
    );

    let logical = EntrySelector::Module("tools::formatter".parse().unwrap());
    assert_eq!(
        default_output_path(&logical, ArtifactKind::Executable),
        PathBuf::from("formatter")
    );
    assert_eq!(
        default_output_path(&logical, ArtifactKind::Assembly),
        PathBuf::from("formatter.s")
    );
}

#[test]
fn logical_entry_requires_utf8_while_filesystem_options_retain_os_strings() {
    let args = [
        OsString::from("skac"),
        OsString::from("--entry"),
        OsString::from_vec(b"app::\xff".to_vec()),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let exit_code = run_cli_with_context(
        args,
        &mut stdout,
        &mut stderr,
        &Toolchain::new("false", "missing-runtime.a"),
    )
    .unwrap();

    assert_eq!(exit_code, EXIT_USAGE);
    assert!(stdout.is_empty());
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("value after `--entry` must be valid UTF-8"));
}
