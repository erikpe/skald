use super::*;

#[test]
fn help_and_version_are_available_without_compilation() {
    let (exit_code, stdout, stderr) = run(&["skac", "--help"]);
    assert_eq!(exit_code, 0);
    assert_eq!(stdout, format!("{HELP}\n"));
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run(&["skac", "--version"]);
    assert_eq!(exit_code, 0);
    assert_eq!(stdout, format!("skac {}\n", env!("CARGO_PKG_VERSION")));
    assert!(stderr.is_empty());
}

#[test]
fn invalid_arguments_are_usage_errors() {
    let (exit_code, stdout, stderr) = run(&["skac"]);
    assert_eq!(exit_code, EXIT_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.starts_with("skac: no input file was provided\n"));

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
