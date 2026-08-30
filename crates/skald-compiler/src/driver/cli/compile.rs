//! Compilation-command execution, operational observation, and presentation.

use std::{
    ffi::OsStr,
    fs,
    io::{self, Write},
    os::unix::fs::MetadataExt,
    path::Path,
};

use crate::{
    backend::target_by_name,
    diagnostics::{render_diagnostic, Diagnostics, Severity},
    reporting::{
        ReportArtifactKind, ReportDetail, ReportEvent, ReportObserver, ReportOutcome, ReportPhase,
        ReportScope, TextObserver,
    },
};

use super::{
    super::{
        artifact::PendingArtifact,
        compile_request_to_assembly_observed,
        observation::{observe_phase, observe_run},
        request::{ArtifactKind, CompilationEnvironment, CompilationRequest, EntrySelector},
        CompilationError, CompilationReport, Toolchain, ToolchainError,
    },
    default_output_path,
    parse::{CompileOptions, DiagnosticLevel},
    EXIT_COMPILE_ERROR, EXIT_IO_ERROR, EXIT_USAGE,
};

pub(super) fn compile<Stderr: Write>(
    options: CompileOptions,
    stderr: &mut Stderr,
    toolchain: &Toolchain,
) -> io::Result<i32> {
    if let Some(status) = validate_entry(&options, stderr)? {
        return Ok(status);
    }
    let target = match target_by_name(&options.target) {
        Ok(target) => target,
        Err(error) => {
            writeln!(stderr, "skac: {error}")?;
            return Ok(EXIT_USAGE);
        }
    };
    let working_directory = match std::env::current_dir() {
        Ok(path) => path,
        Err(error) => {
            writeln!(
                stderr,
                "skac: could not determine the working directory: {error}"
            )?;
            return Ok(EXIT_IO_ERROR);
        }
    };

    if let Some(status) = validate_output_alias(&options, stderr)? {
        return Ok(status);
    }

    let output = options
        .artifact
        .output()
        .map(Path::to_owned)
        .unwrap_or_else(|| default_output_path(&options.entry, options.artifact.kind()));
    let request = CompilationRequest::new(
        options.entry.clone(),
        options.module_roots,
        options.standard_library,
        target,
        options.artifact.clone(),
        CompilationEnvironment::new(
            working_directory,
            toolchain.standard_library_root().to_owned(),
        ),
    )
    .with_mir_optimization(options.mir_optimization);

    let mut observer = TextObserver::new(&mut *stderr, options.report_detail);
    let result = observe_run(
        &mut observer,
        ReportScope::Driver,
        |observer| execute_driver(&request, &output, toolchain, observer),
        result_outcome,
    );
    let (_, report_error) = observer.into_parts();

    let presentation = present_result(result, options.diagnostic_level, stderr);
    if let Some(error) = report_error {
        return Err(error);
    }
    presentation
}

fn validate_entry<Stderr: Write>(
    options: &CompileOptions,
    stderr: &mut Stderr,
) -> io::Result<Option<i32>> {
    if let EntrySelector::File(input) = &options.entry {
        if input.extension() != Some(OsStr::new("ska")) {
            writeln!(
                stderr,
                "skac: input must use the canonical `.ska` suffix: {}",
                input.display()
            )?;
            return Ok(Some(EXIT_USAGE));
        }
    }
    Ok(None)
}

fn validate_output_alias<Stderr: Write>(
    options: &CompileOptions,
    stderr: &mut Stderr,
) -> io::Result<Option<i32>> {
    let EntrySelector::File(input) = &options.entry else {
        return Ok(None);
    };
    let Some(output) = options.artifact.output() else {
        return Ok(None);
    };
    match paths_refer_to_same_file(input, output) {
        Ok(true) => {
            writeln!(
                stderr,
                "skac: output path must not refer to the input source: {}",
                output.display()
            )?;
            Ok(Some(EXIT_USAGE))
        }
        Ok(false) => Ok(None),
        Err(error) => {
            writeln!(stderr, "skac: could not resolve the output path: {error}")?;
            Ok(Some(EXIT_IO_ERROR))
        }
    }
}

fn execute_driver(
    request: &CompilationRequest,
    output: &Path,
    toolchain: &Toolchain,
    observer: &mut dyn ReportObserver,
) -> Result<CompilationReport, CommandError> {
    let artifact = compile_request_to_assembly_observed(request, observer)
        .map_err(CommandError::Compilation)?;

    match request.artifact().kind() {
        ArtifactKind::Assembly => {
            observe_phase(
                observer,
                ReportPhase::ArtifactPublication,
                || publish_assembly(&artifact.assembly, output),
                result_outcome,
            )?;
        }
        ArtifactKind::Executable => {
            let pending = observe_phase(
                observer,
                ReportPhase::HostLinking,
                || toolchain.link_assembly_pending(&artifact.assembly, output),
                result_outcome,
            )
            .map_err(DriverError::Toolchain)?;
            observe_phase(
                observer,
                ReportPhase::ArtifactPublication,
                || {
                    pending.publish().map_err(|source| {
                        DriverError::Toolchain(ToolchainError::Publish { source })
                    })
                },
                result_outcome,
            )?;
        }
    }

    observe_artifact_publication(observer, request.artifact().kind(), output);
    Ok(artifact.report)
}

fn observe_artifact_publication(
    observer: &mut dyn ReportObserver,
    kind: ArtifactKind,
    output: &Path,
) {
    if !observer.enabled(ReportDetail::Phases) {
        return;
    }
    observer.observe(ReportEvent::ArtifactPublished {
        kind: match kind {
            ArtifactKind::Assembly => ReportArtifactKind::Assembly,
            ArtifactKind::Executable => ReportArtifactKind::Executable,
        },
        path: output.to_owned(),
    });
}

fn publish_assembly(assembly: &str, output: &Path) -> Result<(), DriverError> {
    let pending = PendingArtifact::new(output).map_err(DriverError::PrepareOutput)?;
    pending
        .write(assembly.as_bytes())
        .map_err(DriverError::WriteOutput)?;
    pending.publish().map_err(DriverError::PublishOutput)
}

fn present_result<Stderr: Write>(
    result: Result<CompilationReport, CommandError>,
    diagnostic_level: DiagnosticLevel,
    stderr: &mut Stderr,
) -> io::Result<i32> {
    match result {
        Ok(report) => {
            write_selected_diagnostics(stderr, &report, diagnostic_level)?;
            Ok(0)
        }
        Err(CommandError::Compilation(CompilationError::MirOptimizationConfiguration(error))) => {
            writeln!(stderr, "skac: {error}")?;
            Ok(EXIT_USAGE)
        }
        Err(CommandError::Compilation(CompilationError::ProviderConfiguration(errors))) => {
            for error in errors {
                writeln!(stderr, "skac: {error}")?;
            }
            Ok(EXIT_COMPILE_ERROR)
        }
        Err(CommandError::Compilation(CompilationError::Diagnostics(report))) => {
            write_selected_diagnostics(stderr, &report, diagnostic_level)?;
            Ok(EXIT_COMPILE_ERROR)
        }
        Err(CommandError::Compilation(CompilationError::Backend(error))) => {
            writeln!(stderr, "skac: internal {error}")?;
            Ok(EXIT_COMPILE_ERROR)
        }
        Err(CommandError::Compilation(CompilationError::MirVerification(errors))) => {
            writeln!(stderr, "skac: internal MIR verification failed:\n{errors}")?;
            Ok(EXIT_COMPILE_ERROR)
        }
        Err(CommandError::Driver(error)) => {
            writeln!(stderr, "skac: {error}")?;
            Ok(error.exit_code())
        }
    }
}

fn write_selected_diagnostics(
    stderr: &mut impl Write,
    report: &CompilationReport,
    diagnostic_level: DiagnosticLevel,
) -> io::Result<()> {
    let rendered =
        render_selected_diagnostics(&report.sources, &report.diagnostics, diagnostic_level);
    if !rendered.is_empty() {
        write!(stderr, "{rendered}")?;
    }
    Ok(())
}

fn render_selected_diagnostics(
    sources: &crate::source::SourceDatabase,
    diagnostics: &Diagnostics,
    diagnostic_level: DiagnosticLevel,
) -> String {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.severity == Severity::Error || diagnostic_level == DiagnosticLevel::Warning
        })
        .map(|diagnostic| render_diagnostic(sources, diagnostic))
        .collect::<Vec<_>>()
        .join("\n")
}

fn paths_refer_to_same_file(input: &Path, output: &Path) -> io::Result<bool> {
    let input = match fs::metadata(input) {
        Ok(input) => input,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    match fs::metadata(output) {
        Ok(output) => Ok(input.dev() == output.dev() && input.ino() == output.ino()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn result_outcome<T, E>(result: &Result<T, E>) -> ReportOutcome {
    if result.is_ok() {
        ReportOutcome::Completed
    } else {
        ReportOutcome::Failed
    }
}

enum CommandError {
    Compilation(CompilationError),
    Driver(DriverError),
}

impl From<DriverError> for CommandError {
    fn from(error: DriverError) -> Self {
        Self::Driver(error)
    }
}

enum DriverError {
    PrepareOutput(io::Error),
    WriteOutput(io::Error),
    PublishOutput(io::Error),
    Toolchain(ToolchainError),
}

impl DriverError {
    const fn exit_code(&self) -> i32 {
        match self {
            Self::PrepareOutput(_) | Self::WriteOutput(_) | Self::PublishOutput(_) => EXIT_IO_ERROR,
            Self::Toolchain(_) => EXIT_COMPILE_ERROR,
        }
    }
}

impl std::fmt::Display for DriverError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrepareOutput(error) => {
                write!(formatter, "could not prepare assembly output: {error}")
            }
            Self::WriteOutput(error) => {
                write!(formatter, "could not write assembly output: {error}")
            }
            Self::PublishOutput(error) => {
                write!(formatter, "could not publish assembly output: {error}")
            }
            Self::Toolchain(error) => error.fmt(formatter),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        backend::{BackendError, Target},
        diagnostics::{Diagnostic, Diagnostics},
        source::SourceDatabase,
    };

    use super::*;

    #[test]
    fn diagnostic_level_filters_only_warnings_at_the_presentation_boundary() {
        let mut sources = SourceDatabase::new();
        let source = sources.add("filter.ska", "fn main() -> i64 { return 0; }");
        let span = sources.get(source).unwrap().span(0, 2).unwrap();
        let diagnostics: Diagnostics = [
            Diagnostic::warning("TEST001", "warning text").with_primary_label(span, "warning"),
            Diagnostic::error("TEST002", "error text").with_primary_label(span, "error"),
        ]
        .into_iter()
        .collect();

        let warning = render_selected_diagnostics(&sources, &diagnostics, DiagnosticLevel::Warning);
        assert!(warning.contains("warning[TEST001]"));
        assert!(warning.contains("error[TEST002]"));
        let error = render_selected_diagnostics(&sources, &diagnostics, DiagnosticLevel::Error);
        assert!(!error.contains("warning[TEST001]"));
        assert!(error.contains("error[TEST002]"));
        assert_eq!(diagnostics.len(), 2, "filtering must not mutate the report");
    }

    #[test]
    fn default_warning_presentation_is_byte_stable_without_operational_reports() {
        let mut sources = SourceDatabase::new();
        let source = sources.add("warning.ska", "let\n");
        let span = sources.get(source).unwrap().span(0, 3).unwrap();
        let diagnostics = [
            Diagnostic::warning("TEST001", "example warning").with_primary_label(span, "primary")
        ]
        .into_iter()
        .collect();
        let report = CompilationReport {
            sources,
            diagnostics,
        };
        let mut stderr = Vec::new();

        let status = present_result(Ok(report), DiagnosticLevel::Warning, &mut stderr).unwrap();

        assert_eq!(status, 0);
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            concat!(
                "warning[TEST001]: example warning\n",
                " --> warning.ska:1:1\n",
                "   |\n",
                "1 | let\n",
                "   | ^^^ primary\n",
            )
        );
    }

    #[test]
    fn backend_failure_default_presentation_is_byte_stable_and_not_duplicated() {
        let result = Err(CommandError::Compilation(CompilationError::Backend(
            BackendError::new(Target::X86_64SysV, None, "injected backend failure"),
        )));
        let mut stderr = Vec::new();

        let status = present_result(result, DiagnosticLevel::Warning, &mut stderr).unwrap();

        assert_eq!(status, EXIT_COMPILE_ERROR);
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            "skac: internal x86_64-sysv backend error: injected backend failure\n"
        );
    }

    #[test]
    fn disabled_artifact_observation_does_not_construct_an_owned_path_event() {
        observe_artifact_publication(
            &mut RejectingDisabledObserver,
            ArtifactKind::Assembly,
            Path::new("not-cloned.s"),
        );
    }

    struct RejectingDisabledObserver;

    impl ReportObserver for RejectingDisabledObserver {
        fn enabled(&self, _detail: ReportDetail) -> bool {
            false
        }

        fn observe(&mut self, _event: ReportEvent) {
            panic!("disabled artifact observation must not emit an event");
        }
    }
}
