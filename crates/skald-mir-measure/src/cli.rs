//! Small command-line adapter around corpus planning and measurement.

use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
    process::ExitCode,
};

use crate::{load_corpus, measure_corpus, render_report, MeasurementOptions, ReportFormat};

const HELP: &str = r#"skald-mir-measure - local final-MIR redundancy census

Usage:
  skald-mir-measure [options]

Options:
  --manifest PATH         Corpus manifest (default: tests/measurements/local_mir_redundancy.toml)
  --repository-root PATH  Repository root (default: current directory)
  --workload ID           Measure one manifest workload; repeatable
  --format human|json     Output projection (default: human)
  --output PATH           Write below build/measurements instead of stdout
  --operational           Include nondeterministic compile-duration context
  -h, --help              Show this help
"#;

pub fn run_cli(arguments: impl IntoIterator<Item = OsString>) -> ExitCode {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    ExitCode::from(run_with_io(arguments, &mut stdout, &mut stderr))
}

fn run_with_io(
    arguments: impl IntoIterator<Item = OsString>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    let options = match Options::parse(arguments) {
        Ok(options) => options,
        Err(error) => return fail(stderr, &format!("skald-mir-measure: {error}\n\n{HELP}"), 2),
    };
    if options.help {
        return write(stdout, stderr, HELP);
    }
    let repository_root = match fs::canonicalize(&options.repository_root) {
        Ok(path) => path,
        Err(error) => {
            return fail(
                stderr,
                &format!("skald-mir-measure: invalid repository root: {error}"),
                2,
            )
        }
    };
    let mut corpus = match load_corpus(&repository_root, &options.manifest) {
        Ok(corpus) => corpus,
        Err(error) => return fail(stderr, &format!("skald-mir-measure: {error}"), 2),
    };
    if !options.workloads.is_empty() {
        if let Err(error) = corpus.retain_ids(&options.workloads) {
            return fail(stderr, &format!("skald-mir-measure: {error}"), 2);
        }
    }
    let report = match measure_corpus(
        &repository_root,
        &corpus,
        MeasurementOptions::default().with_operational_context(options.operational),
    ) {
        Ok(report) => report,
        Err(error) => return fail(stderr, &format!("skald-mir-measure: {error}"), 1),
    };
    let rendered = match render_report(&report, options.format) {
        Ok(rendered) => rendered,
        Err(error) => return fail(stderr, &format!("skald-mir-measure: {error}"), 1),
    };
    match options.output {
        Some(output) => match write_output(&repository_root, &output, &rendered) {
            Ok(()) => 0,
            Err(error) => fail(stderr, &format!("skald-mir-measure: {error}"), 1),
        },
        None => write(stdout, stderr, &rendered),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    repository_root: PathBuf,
    manifest: PathBuf,
    workloads: BTreeSet<String>,
    format: ReportFormat,
    output: Option<PathBuf>,
    operational: bool,
    help: bool,
}

impl Options {
    fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut arguments = arguments.into_iter();
        let _program = arguments.next();
        let mut options = Self {
            repository_root: PathBuf::from("."),
            manifest: PathBuf::from("tests/measurements/local_mir_redundancy.toml"),
            workloads: BTreeSet::new(),
            format: ReportFormat::Human,
            output: None,
            operational: false,
            help: false,
        };
        while let Some(argument) = arguments.next() {
            match argument.to_str() {
                Some("-h" | "--help") => options.help = true,
                Some("--operational") if !options.operational => options.operational = true,
                Some("--operational") => return Err("--operational specified twice".to_owned()),
                Some("--repository-root") => {
                    options.repository_root = value(&mut arguments, "--repository-root")?.into();
                }
                Some("--manifest") => {
                    options.manifest = value(&mut arguments, "--manifest")?.into();
                }
                Some("--workload") => {
                    let id = utf8(value(&mut arguments, "--workload")?, "workload ID")?;
                    if !options.workloads.insert(id.clone()) {
                        return Err(format!("workload {id:?} selected twice"));
                    }
                }
                Some("--output") if options.output.is_none() => {
                    options.output = Some(value(&mut arguments, "--output")?.into());
                }
                Some("--output") => return Err("--output specified twice".to_owned()),
                Some("--format") => {
                    let format = utf8(value(&mut arguments, "--format")?, "format")?;
                    options.format = match format.as_str() {
                        "human" => ReportFormat::Human,
                        "json" => ReportFormat::Json,
                        _ => {
                            return Err(format!(
                                "unknown format {format:?}; expected human or json"
                            ))
                        }
                    };
                }
                Some(value) => return Err(format!("unknown option {value:?}")),
                None => return Err("options must be valid UTF-8".to_owned()),
            }
        }
        Ok(options)
    }
}

fn value(arguments: &mut impl Iterator<Item = OsString>, option: &str) -> Result<OsString, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn utf8(value: OsString, description: &str) -> Result<String, String> {
    value
        .into_string()
        .map_err(|_| format!("{description} must be valid UTF-8"))
}

fn write_output(repository_root: &Path, output: &Path, contents: &str) -> Result<(), String> {
    if output.is_absolute()
        || output.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        || !output.starts_with("build/measurements")
    {
        return Err("--output must be a contained path below build/measurements".to_owned());
    }
    let output = repository_root.join(output);
    let parent = output
        .parent()
        .expect("validated measurement output must have a parent");
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    fs::write(&output, contents)
        .map_err(|error| format!("could not write {}: {error}", output.display()))
}

fn write(stdout: &mut impl Write, stderr: &mut impl Write, contents: &str) -> u8 {
    match stdout.write_all(contents.as_bytes()) {
        Ok(()) => 0,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => 0,
        Err(error) => fail(stderr, &format!("skald-mir-measure: {error}"), 1),
    }
}

fn fail(stderr: &mut impl Write, message: &str, status: u8) -> u8 {
    let _ = writeln!(stderr, "{message}");
    status
}

#[cfg(test)]
mod tests {
    use super::Options;
    use crate::ReportFormat;
    use std::ffi::OsString;

    fn parse(arguments: &[&str]) -> Result<Options, String> {
        Options::parse(arguments.iter().map(OsString::from))
    }

    #[test]
    fn parses_selection_format_and_operational_context() {
        let options = parse(&[
            "tool",
            "--workload",
            "focused/one",
            "--format",
            "json",
            "--operational",
            "--output",
            "build/measurements/report.json",
        ])
        .unwrap();
        assert_eq!(options.format, ReportFormat::Json);
        assert!(options.operational);
        assert!(options.workloads.contains("focused/one"));
    }

    #[test]
    fn rejects_duplicates_missing_values_and_unknown_options() {
        assert!(parse(&["tool", "--workload", "a", "--workload", "a"]).is_err());
        assert!(parse(&["tool", "--manifest"]).is_err());
        assert!(parse(&["tool", "--unknown"]).is_err());
    }
}
