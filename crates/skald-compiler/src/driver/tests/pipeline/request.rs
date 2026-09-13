use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    backend::Target,
    diagnostics::render_diagnostics,
    driver::{
        compile_request_to_assembly, compile_request_to_assembly_observed, ArtifactKind,
        ArtifactOptions, CompilationEnvironment, CompilationError, CompilationRequest,
        EntrySelector, MirOptimizationOptions, MirOptimizationProfile, StandardLibrarySelection,
    },
    reporting::{MetricValue, RecordingObserver, ReportDetail, ReportEvent, ReportPhase},
    test_support::{canonical_standard_library_sources, TemporaryDirectory},
};

fn write_canonical_standard_library(root: &Path) {
    for (relative, source) in canonical_standard_library_sources(&[]) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }
}

fn module_request(
    directory: &TemporaryDirectory,
    entry: EntrySelector,
    roots: Vec<PathBuf>,
) -> CompilationRequest {
    CompilationRequest::new(
        entry,
        roots,
        StandardLibrarySelection::Disabled,
        Target::X86_64SysV,
        ArtifactOptions::new(ArtifactKind::Assembly, None),
        CompilationEnvironment::new(directory.path().to_owned(), directory.join("unused-std")),
    )
}

fn mir_pipeline_pass_executions(events: &[ReportEvent]) -> Option<MetricValue> {
    mir_pipeline_metric(events, "pass executions")
}

fn mir_pipeline_metric(events: &[ReportEvent], name: &str) -> Option<MetricValue> {
    events.iter().find_map(|event| match event {
        ReportEvent::PhaseFinished {
            phase: ReportPhase::MirPipeline,
            metrics,
            ..
        } => metrics
            .iter()
            .find(|metric| metric.name() == name)
            .map(|metric| metric.value()),
        _ => None,
    })
}

#[test]
fn request_pipeline_compiles_the_reachable_multi_module_program() {
    let directory = TemporaryDirectory::new("request-pipeline").unwrap();
    let root = directory.join("modules");
    fs::create_dir_all(root.join("app")).unwrap();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("app/main.ska"),
        concat!(
            "import lib::answer;\n",
            "fn main() -> i64 { return lib::answer::value(); }\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("lib/answer.ska"),
        "public fn value() -> i64 { return 42; }\n",
    )
    .unwrap();
    let request = module_request(
        &directory,
        EntrySelector::Module("app::main".parse().unwrap()),
        vec![root],
    );

    let artifact = compile_request_to_assembly(&request).unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert_eq!(artifact.report.sources.len(), 2);
    assert!(artifact
        .assembly
        .contains("call .Lska.fn.lib.answer.value.f1"));
    assert!(artifact.assembly.contains(".globl main"));
}

#[test]
fn request_selection_matrix_reaches_quiet_and_observed_pipelines() {
    let directory = TemporaryDirectory::new("request-optimization-profile").unwrap();
    let root = directory.join("modules");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("app.ska"),
        "fn dead() -> i64 { return 9; }\nfn main() -> i64 { return 6 * 7; }\n",
    )
    .unwrap();
    let base = module_request(
        &directory,
        EntrySelector::Module("app".parse().unwrap()),
        vec![root],
    );

    let quiet_default = compile_request_to_assembly(&base).unwrap();
    assert!(quiet_default.report.diagnostics.is_empty());

    let cases = [
        (MirOptimizationOptions::default(), 17, 1),
        (
            MirOptimizationOptions::new(MirOptimizationProfile::None),
            0,
            2,
        ),
        (
            MirOptimizationOptions::default().with_disabled_pass("whole-world-reachability"),
            16,
            2,
        ),
        (
            MirOptimizationOptions::default()
                .with_disabled_pass("checked-integer-constant-folding"),
            16,
            1,
        ),
        (
            MirOptimizationOptions::default()
                .with_disabled_pass("checked-f64-to-integer-constant-folding")
                .with_disabled_pass("checked-integer-constant-folding")
                .with_disabled_pass("conservative-cfg-cleanup")
                .with_disabled_pass("constant-short-circuit-folding")
                .with_disabled_pass("dead-normalized-path-activation-cleanup")
                .with_disabled_pass("dead-pure-definition-elimination")
                .with_disabled_pass("integer-cast-chain-canonicalization")
                .with_disabled_pass("post-proof-basic-block-merging")
                .with_disabled_pass("post-proof-empty-block-forwarding")
                .with_disabled_pass("post-proof-unreachable-block-elimination")
                .with_disabled_pass("primitive-algebraic-simplification")
                .with_disabled_pass("primitive-constant-folding")
                .with_disabled_pass("whole-world-reachability"),
            0,
            2,
        ),
    ];
    for (options, expected_executions, expected_definitions) in cases {
        let request = base.clone().with_mir_optimization(options);
        let mut observer = RecordingObserver::new(ReportDetail::Details);
        let artifact = compile_request_to_assembly_observed(&request, &mut observer).unwrap();
        let repeated = compile_request_to_assembly(&request).unwrap();

        assert_eq!(artifact.assembly, repeated.assembly);
        assert!(artifact.assembly.contains(".globl main"));
        assert!(!artifact.assembly.contains(".fn.app.dead.f0"));
        assert!(artifact.report.diagnostics.is_empty());
        assert_eq!(
            mir_pipeline_pass_executions(observer.events()),
            Some(MetricValue::Count(expected_executions))
        );
        assert_eq!(
            mir_pipeline_metric(observer.events(), "definitions"),
            Some(MetricValue::Count(expected_definitions))
        );
    }
}

#[test]
fn invalid_request_optimization_is_rejected_before_provider_or_source_io() {
    let directory = TemporaryDirectory::new("invalid-request-optimization").unwrap();
    let request = module_request(
        &directory,
        EntrySelector::File(directory.join("missing.ska")),
        vec![directory.join("missing-root")],
    )
    .with_mir_optimization(MirOptimizationOptions::default().with_disabled_pass("unknown-pass"));
    let mut observer = RecordingObserver::new(ReportDetail::Trace);

    let error = compile_request_to_assembly_observed(&request, &mut observer).unwrap_err();

    let CompilationError::MirOptimizationConfiguration(error) = error else {
        panic!("invalid optimization selection must retain its failure category")
    };
    assert_eq!(error.names(), ["unknown-pass"]);
    assert!(observer.events().is_empty());
}

#[test]
fn request_pipeline_emits_closed_generic_classes_across_modules() {
    let directory = TemporaryDirectory::new("request-generic-pipeline").unwrap();
    let root = directory.join("modules");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("app.ska"),
        concat!(
            "from model import Box, Item;\n",
            "fn main() -> i64 { var box: Box<Item> = Box<Item>(Item(42)); var item: Item = box.get(); return item.value; }\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("model.ska"),
        concat!(
            "public class Item { value: i64; init(value: i64) { self.value = value; } }\n",
            "public class Box<T> { value: T; init(value: T) { self.value = value; } fn get() -> T { return self.value; } }\n",
        ),
    )
    .unwrap();
    let request = module_request(
        &directory,
        EntrySelector::Module("app".parse().unwrap()),
        vec![root],
    );

    let artifact = compile_request_to_assembly(&request).unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact
        .assembly
        .contains("class.model_x3a__x3a_Box_x3c_model_x3a__x3a_Item_x3e_"));
    assert!(!artifact
        .assembly
        .contains("model_x3a__x3a_model_x3a__x3a_Box"));
    assert!(artifact.assembly.contains("call ska_rt_abi_v9"));
    assert!(!artifact.assembly.contains("ClassTemplate"));
}

#[test]
fn request_pipeline_ignores_malformed_sources_outside_the_reachable_closure() {
    let directory = TemporaryDirectory::new("request-reachability").unwrap();
    let root = directory.join("modules");
    fs::create_dir_all(root.join("app")).unwrap();
    fs::create_dir_all(root.join("unused")).unwrap();
    fs::write(
        root.join("app/main.ska"),
        "fn main() -> i64 { return 42; }\n",
    )
    .unwrap();
    fs::write(root.join("unused/malformed.ska"), "fn broken( {\n").unwrap();
    let request = module_request(
        &directory,
        EntrySelector::Module("app::main".parse().unwrap()),
        vec![root],
    );

    let artifact = compile_request_to_assembly(&request).unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert_eq!(artifact.report.sources.len(), 1);
    assert!(artifact.assembly.contains("mov rax, 42"));
    assert!(artifact.assembly.contains(".Lska.trace."));
}

#[test]
fn literal_program_reaches_target_emission() {
    let directory = TemporaryDirectory::new("request-string-target-emission").unwrap();
    let root = directory.join("modules");
    fs::create_dir_all(root.join("std")).unwrap();
    fs::write(
        root.join("app.ska"),
        concat!(
            "from std::str import Str;\n",
            "fn main() -> i64 { var value: Str = \"typed\"; return 0; }\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("std/str.ska"),
        concat!(
            "public class Str {\n",
            "  private _storage: shared u8[];\n",
            "  private _start: i64;\n",
            "  private _length: u64;\n",
            "  private cell _hash_code: u64?;\n",
            "  init() { self._storage = new u8[](); self._start = 0; self._length = 0u; self._hash_code = none; }\n",
            "}\n",
        ),
    )
    .unwrap();
    let request = module_request(
        &directory,
        EntrySelector::Module("app".parse().unwrap()),
        vec![root],
    );

    let artifact = compile_request_to_assembly(&request).unwrap();
    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains(".Lska_literal_0_backing:"));
    assert!(artifact.assembly.contains(".quad 0xffffffffffffffff"));
}

#[test]
fn request_pipeline_accepts_a_positional_entry_outside_all_roots() {
    let directory = TemporaryDirectory::new("request-singleton").unwrap();
    let spaced_directory = directory.join("directory with spaces");
    fs::create_dir(&spaced_directory).unwrap();
    let input = spaced_directory.join("outside_main.ska");
    fs::write(&input, "fn main() -> i64 { return 42; }\n").unwrap();
    let request = module_request(&directory, EntrySelector::File(input), Vec::new());

    let artifact = compile_request_to_assembly(&request).unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert_eq!(artifact.report.sources.len(), 1);
    assert!(artifact.assembly.contains("mov rax, 42"));
}

#[test]
fn replacement_standard_library_validates_the_canonical_panic_intrinsic() {
    let directory = TemporaryDirectory::new("request-panic-intrinsic").unwrap();
    let root = directory.join("modules");
    let standard_library = directory.join("replacement-std");
    fs::create_dir_all(&root).unwrap();
    write_canonical_standard_library(&standard_library);
    fs::write(
        root.join("app.ska"),
        "import std::error;\nfn main() -> i64 { return 0; }\n",
    )
    .unwrap();
    let request = CompilationRequest::new(
        EntrySelector::Module("app".parse().unwrap()),
        vec![root],
        StandardLibrarySelection::Replacement(standard_library.clone()),
        Target::X86_64SysV,
        ArtifactOptions::new(ArtifactKind::Assembly, None),
        CompilationEnvironment::new(directory.path().to_owned(), directory.join("unused-std")),
    );

    let artifact = compile_request_to_assembly(&request).unwrap();
    assert!(artifact.report.diagnostics.is_empty());

    fs::write(
        standard_library.join("std/error.ska"),
        "public fn panic(message: i64) -> unit {}\n",
    )
    .unwrap();
    let CompilationError::Diagnostics(report) = compile_request_to_assembly(&request).unwrap_err()
    else {
        panic!("expected canonical intrinsic diagnostics");
    };
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == crate::resolve::INVALID_INTRINSIC_DECLARATION));
}

#[test]
fn canonical_standard_library_cycle_obeys_default_replacement_and_disabled_selection() {
    let directory = TemporaryDirectory::new("request-standard-library-cycle").unwrap();
    let application = directory.join("application");
    let installed = directory.join("installed");
    let replacement = directory.join("replacement");
    let self_contained = directory.join("self-contained");
    fs::create_dir_all(&application).unwrap();
    fs::write(
        application.join("app.ska"),
        "from std::str import Str;\nfn main() -> i64 { var value: Str = \"ok\"; return (i64) value.len(); }\n",
    )
    .unwrap();
    write_canonical_standard_library(&installed);
    write_canonical_standard_library(&replacement);
    fs::create_dir_all(&self_contained).unwrap();
    fs::write(
        self_contained.join("app.ska"),
        "from std::str import Str;\nfn main() -> i64 { var value: Str = \"ok\"; return (i64) value.len(); }\n",
    )
    .unwrap();
    write_canonical_standard_library(&self_contained);

    let compile = |roots, standard_library, installed_root| {
        let request = CompilationRequest::new(
            EntrySelector::Module("app".parse().unwrap()),
            roots,
            standard_library,
            Target::X86_64SysV,
            ArtifactOptions::new(ArtifactKind::Assembly, None),
            CompilationEnvironment::new(directory.path().to_owned(), installed_root),
        );
        compile_request_to_assembly(&request)
    };

    for artifact in [
        compile(
            vec![application.clone()],
            StandardLibrarySelection::Default,
            installed.clone(),
        )
        .unwrap(),
        compile(
            vec![application.clone()],
            StandardLibrarySelection::Replacement(replacement),
            directory.join("unused-installed"),
        )
        .unwrap(),
        compile(
            vec![self_contained],
            StandardLibrarySelection::Disabled,
            directory.join("unused-installed"),
        )
        .unwrap(),
    ] {
        assert!(artifact.report.diagnostics.is_empty());
        // Vec is reachable through Str and ordinarily imports canonical
        // Iterable. Str's bounded scans also reach canonical Range and its
        // operator protocol, so the complete graph includes std::iter,
        // std::range, and std::ops.
        assert_eq!(artifact.report.sources.len(), 14);
        assert!(artifact.assembly.contains("call ska_rt_panic"));
    }

    let CompilationError::Diagnostics(report) = compile(
        vec![application],
        StandardLibrarySelection::Disabled,
        directory.join("unused-installed"),
    )
    .unwrap_err() else {
        panic!("disabled lookup without a provider-owned standard library must fail");
    };
    assert!(render_diagnostics(&report.sources, &report.diagnostics)
        .contains("module `std::str` was not found"));
}

#[test]
fn canonical_io_obeys_default_replacement_and_disabled_selection() {
    let directory = TemporaryDirectory::new("request-standard-io-providers").unwrap();
    let application = directory.join("application");
    let installed = directory.join("installed");
    let replacement = directory.join("replacement");
    let self_contained = directory.join("self-contained");
    let app_source = concat!(
        "import std::io;\n",
        "from std::str import Str;\n",
        "fn main() -> i64 {\n",
        "  var path: Str = \"input.bin\";\n",
        "  var stdin: Str = std::io::read_stdin();\n",
        "  var file: Str = std::io::read_file(path);\n",
        "  std::io::write_stdout(stdin);\n",
        "  std::io::write_stderr(file);\n",
        "  return 0;\n",
        "}\n",
    );
    fs::create_dir_all(&application).unwrap();
    fs::write(application.join("app.ska"), app_source).unwrap();
    write_canonical_standard_library(&installed);
    write_canonical_standard_library(&replacement);
    fs::create_dir_all(&self_contained).unwrap();
    fs::write(self_contained.join("app.ska"), app_source).unwrap();
    write_canonical_standard_library(&self_contained);

    let compile = |roots, standard_library, installed_root| {
        let request = CompilationRequest::new(
            EntrySelector::Module("app".parse().unwrap()),
            roots,
            standard_library,
            Target::X86_64SysV,
            ArtifactOptions::new(ArtifactKind::Assembly, None),
            CompilationEnvironment::new(directory.path().to_owned(), installed_root),
        );
        compile_request_to_assembly(&request)
    };

    for artifact in [
        compile(
            vec![application.clone()],
            StandardLibrarySelection::Default,
            installed.clone(),
        )
        .unwrap(),
        compile(
            vec![application.clone()],
            StandardLibrarySelection::Replacement(replacement),
            directory.join("unused-installed"),
        )
        .unwrap(),
        compile(
            vec![self_contained],
            StandardLibrarySelection::Disabled,
            directory.join("unused-installed"),
        )
        .unwrap(),
    ] {
        assert!(artifact.report.diagnostics.is_empty());
        // Vec is reachable through Str and ordinarily imports canonical
        // Iterable. Str's bounded scans also reach canonical Range and its
        // operator protocol, so the complete graph includes std::iter,
        // std::range, and std::ops.
        assert_eq!(artifact.report.sources.len(), 15);
        for runtime_symbol in [
            "ska_rt_io_standard_handle",
            "ska_rt_io_open",
            "ska_rt_io_read",
            "ska_rt_io_write",
            "ska_rt_io_close",
        ] {
            assert!(artifact
                .assembly
                .contains(&format!("call {runtime_symbol}")));
        }
    }

    let CompilationError::Diagnostics(report) = compile(
        vec![application],
        StandardLibrarySelection::Disabled,
        directory.join("unused-installed"),
    )
    .unwrap_err() else {
        panic!("disabled lookup without a provider-owned standard library must fail");
    };
    assert!(render_diagnostics(&report.sources, &report.diagnostics)
        .contains("module `std::io` was not found"));
}

#[test]
fn installed_process_arguments_reach_verified_assembly_as_ordinary_library_source() {
    let directory = TemporaryDirectory::new("request-process-arguments").unwrap();
    let application = directory.join("application");
    let installed = directory.join("installed");
    fs::create_dir_all(&application).unwrap();
    fs::write(
        application.join("app.ska"),
        concat!(
            "from std::process import args;\n",
            "import std::str;\n",
            "fn main() -> i64 {\n",
            "  var values: std::str::Str[] = args();\n",
            "  return (i64) values.len();\n",
            "}\n",
        ),
    )
    .unwrap();
    write_canonical_standard_library(&installed);
    let request = CompilationRequest::new(
        EntrySelector::Module("app".parse().unwrap()),
        vec![application],
        StandardLibrarySelection::Default,
        Target::X86_64SysV,
        ArtifactOptions::new(ArtifactKind::Assembly, None),
        CompilationEnvironment::new(directory.path().to_owned(), installed),
    );

    let artifact = compile_request_to_assembly(&request).unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    // Process arguments reach Vec through Str, while their bounded scans also
    // reach canonical Range and its operator protocol.
    assert_eq!(artifact.report.sources.len(), 16);
    assert!(artifact.assembly.contains(".Lska.fn.std.process.args."));
    assert!(artifact
        .assembly
        .contains("call .Lska.fn.std.io.read_file."));
    assert_eq!(artifact.assembly.matches("call ska_rt_abi_v9\n").count(), 1);
    assert!(artifact.assembly.contains(concat!(
        "main:\n",
        "    push rbp\n",
        "    mov rbp, rsp\n",
        "    call ska_rt_abi_v9\n",
        "    call .Lska.static.initialize\n",
        "    call .Lska.fn.app.main.",
    )));
    for runtime_symbol in ["ska_rt_io_open", "ska_rt_io_read", "ska_rt_io_close"] {
        assert!(artifact
            .assembly
            .contains(&format!("call {runtime_symbol}")));
    }
    assert!(!artifact.assembly.contains("call ska_rt_io_write"));
}

#[test]
fn request_pipeline_preserves_configuration_and_source_failure_categories() {
    let directory = TemporaryDirectory::new("request-failures").unwrap();
    let invalid_root = directory.join("missing-root");
    let request = module_request(
        &directory,
        EntrySelector::Module("app::main".parse().unwrap()),
        vec![invalid_root],
    );
    let CompilationError::ProviderConfiguration(errors) =
        compile_request_to_assembly(&request).unwrap_err()
    else {
        panic!("expected provider configuration failure");
    };
    assert_eq!(errors.len(), 1);

    let missing = module_request(
        &directory,
        EntrySelector::File(directory.join("missing.ska")),
        Vec::new(),
    );
    let CompilationError::Diagnostics(report) = compile_request_to_assembly(&missing).unwrap_err()
    else {
        panic!("expected source diagnostics");
    };
    assert!(render_diagnostics(&report.sources, &report.diagnostics)
        .contains("error[MOD001]: invalid entry"));
}
