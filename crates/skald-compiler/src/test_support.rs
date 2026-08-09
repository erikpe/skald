//! Test-only compiler pipelines and temporary resources.
//!
//! Each source helper asserts success only for phases before the boundary its
//! name exposes. Tests remain responsible for checking the requested phase.

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{
    backend::{
        emit_assembly, BackendError, BackendInput, RuntimeTracePolicy, Target,
        RUNTIME_ABI_MARKER_SYMBOL,
    },
    driver::EntrySelector,
    lexer::{lex, LexOutput},
    mir::{lower_hir, lower_preliminary_hir, MirProgram},
    module::{load_module_graph, normalize_provider_roots, ModuleGraph, ProviderRootConfiguration},
    resolve::{resolve, resolve_with_source_path, ResolveOutput},
    source::{SourceDatabase, SourceId},
    syntax::{parse, ParseOutput},
    typeck::{type_check, TypeCheckOutput},
};

#[path = "../test_support/standard_library.rs"]
mod standard_library;
pub(crate) use standard_library::{
    canonical_standard_library_sources, CANONICAL_F64_SOURCE, CANONICAL_IO_SOURCE,
    CANONICAL_STR_SOURCE,
};

static NEXT_TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) const INLINE_FIELD_SOURCE: &str = concat!(
    "class Root {\n",
    "  flag: bool; left: Branch; right: Branch;\n",
    "  init(left: i64, right: i64) {\n",
    "    self.right = Branch(right);\n",
    "    self.flag = true;\n",
    "    self.left = Branch(left);\n",
    "  }\n",
    "  fn total() -> i64 { return self.left.leaf.value + self.right.leaf.read(); }\n",
    "  mut fn adjust() -> i64 {\n",
    "    self.left.leaf.value = self.left.leaf.value + 1;\n",
    "    return mutate(self.right.leaf, 5) + self.left.leaf.read();\n",
    "  }\n",
    "}\n",
    "class Empty { init() {} }\n",
    "class Leaf {\n",
    "  small: u8; value: i64;\n",
    "  init(value: i64) { self.value = value; self.small = 1u8; }\n",
    "  fn read() -> i64 { return self.value; }\n",
    "  mut fn add(delta: i64) -> i64 { self.value = self.value + delta; return self.value; }\n",
    "}\n",
    "class Branch {\n",
    "  tag: u8; empty: Empty; leaf: Leaf; tail: u8;\n",
    "  init(value: i64) {\n",
    "    self.leaf = Leaf(value); self.tag = 2u8; self.empty = Empty(); self.tail = 3u8;\n",
    "  }\n",
    "}\n",
    "fn read(ref leaf: Leaf) -> i64 { return leaf.read(); }\n",
    "fn mutate(mut ref leaf: Leaf, delta: i64) -> i64 { return leaf.add(delta); }\n",
    "fn forward(mut ref root: Root) -> i64 {\n",
    "  return mutate(root.left.leaf, 3) + read(root.right.leaf);\n",
    "}\n",
    "fn main() -> i64 {\n",
    "  var root: Root = Root(10, 20);\n",
    "  return forward(root) + root.adjust() + root.total();\n",
    "}\n",
);

pub(crate) fn lex_source(text: impl Into<String>) -> (SourceDatabase, SourceId, LexOutput) {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("test.ska", text);
    let output = lex(sources
        .get(source_id)
        .expect("test source was just inserted"));
    (sources, source_id, output)
}

pub(crate) fn parse_source(text: impl Into<String>) -> (SourceDatabase, ParseOutput) {
    let (sources, source_id, lexed) = lex_source(text);
    assert_phase_succeeded("lexing", &lexed.diagnostics);
    let parsed = parse(
        sources
            .get(source_id)
            .expect("test source was just inserted"),
        &lexed.tokens,
    );
    (sources, parsed)
}

pub(crate) fn resolve_source(text: impl Into<String>) -> ResolveOutput {
    let (_, parsed) = parse_source(text);
    assert_phase_succeeded("parsing", &parsed.diagnostics);
    resolve(&parsed.ast)
}

pub(crate) fn type_check_source(text: impl Into<String>) -> TypeCheckOutput {
    let resolved = resolve_source(text);
    assert_phase_succeeded("resolution", &resolved.diagnostics);
    type_check(&resolved.program)
}

pub(crate) fn lower_source_to_mir(text: impl Into<String>) -> MirProgram {
    let checked = type_check_source(text);
    assert_phase_succeeded("type checking", &checked.diagnostics);
    lower_hir(
        &checked
            .hir
            .expect("successful type checking must produce typed HIR"),
    )
}

/// Lowers source through static lifecycle planning and synthesis into the
/// exact final MIR product accepted by target backends.
pub(crate) fn lower_source_to_final_mir(text: impl Into<String>) -> MirProgram {
    let checked = type_check_source(text);
    assert_phase_succeeded("type checking", &checked.diagnostics);
    let hir = checked
        .hir
        .expect("successful type checking must produce typed HIR");
    lower_hir_to_final_mir(&hir)
}

pub(crate) struct FinalMirWithSources {
    pub sources: SourceDatabase,
    pub mir: MirProgram,
}

impl FinalMirWithSources {
    pub(crate) fn backend_input(&self, policy: RuntimeTracePolicy) -> BackendInput<'_> {
        match policy {
            RuntimeTracePolicy::Enabled => {
                BackendInput::with_runtime_trace(&self.mir, &self.sources)
            }
            RuntimeTracePolicy::Omitted => BackendInput::without_runtime_trace(&self.mir),
        }
    }

    pub(crate) fn emit_assembly(
        &self,
        target: Target,
        policy: RuntimeTracePolicy,
    ) -> Result<String, BackendError> {
        emit_assembly(target, self.backend_input(policy))
    }
}

/// Retains the source database paired with the exact final MIR passed to a
/// backend, allowing enabled and omitted metadata paths to share one product.
pub(crate) fn lower_source_to_final_mir_with_sources(
    path: impl AsRef<Path>,
    text: impl Into<String>,
) -> FinalMirWithSources {
    let path = path.as_ref();
    let mut sources = SourceDatabase::new();
    let source_id = sources.add(path, text);
    let source = sources
        .get(source_id)
        .expect("test source was just inserted");
    let lexed = lex(source);
    assert_phase_succeeded("lexing", &lexed.diagnostics);
    let parsed = parse(source, &lexed.tokens);
    assert_phase_succeeded("parsing", &parsed.diagnostics);
    let resolved = resolve_with_source_path(&parsed.ast, path);
    assert_phase_succeeded("resolution", &resolved.diagnostics);
    let checked = type_check(&resolved.program);
    assert_phase_succeeded("type checking", &checked.diagnostics);
    let hir = checked
        .hir
        .expect("successful type checking must produce typed HIR");
    FinalMirWithSources {
        sources,
        mir: lower_hir_to_final_mir(&hir),
    }
}

/// Runs lifecycle planning and synthesis for already type-checked test HIR.
pub(crate) fn lower_hir_to_final_mir(hir: &crate::hir::HirProgram) -> MirProgram {
    let preliminary = lower_preliminary_hir(hir);
    let planned = crate::passes::static_lifecycle::plan_static_lifetimes(preliminary)
        .unwrap_or_else(|failure| {
            panic!(
                "test source failed during static lifecycle planning: {:?}",
                failure.into_diagnostics()
            )
        });
    crate::passes::static_lifecycle::synthesize_static_lifecycle(planned)
        .expect("test source must produce valid synthesized MIR")
}

pub(crate) fn lower_source_to_assembly(
    text: impl Into<String>,
    target: Target,
) -> Result<String, BackendError> {
    let mir = lower_source_to_final_mir(text);
    emit_assembly_without_runtime_trace(target, &mir)
}

/// Emits final MIR through the intentionally metadata-free backend path used
/// by tests whose concern predates runtime trace metadata.
pub(crate) fn emit_assembly_without_runtime_trace(
    target: Target,
    mir: &MirProgram,
) -> Result<String, BackendError> {
    emit_assembly(target, BackendInput::without_runtime_trace(mir))
}

pub(crate) fn load_module_sources(
    entry: &str,
    sources: &[(&str, &str)],
) -> (TemporaryDirectory, ModuleGraph) {
    let workspace = TemporaryDirectory::new("module-sources").unwrap();
    let root = workspace.join("modules");
    for (relative, text) in sources {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("module source has a parent")).unwrap();
        fs::write(path, text).unwrap();
    }
    let providers = normalize_provider_roots(
        workspace.path(),
        &[ProviderRootConfiguration::module_root(root)],
    )
    .unwrap();
    let graph = load_module_graph(
        &EntrySelector::Module(entry.parse().unwrap()),
        workspace.path(),
        &providers,
    )
    .unwrap();
    (workspace, graph)
}

pub(crate) fn load_module_sources_with_standard_library(
    entry: &str,
    sources: &[(&str, &str)],
) -> (TemporaryDirectory, ModuleGraph) {
    load_module_sources_with_standard_library_overrides(entry, sources, &[])
}

pub(crate) fn load_module_sources_with_standard_library_overrides<'a>(
    entry: &str,
    sources: &[(&'a str, &'a str)],
    overrides: &[(&str, &'a str)],
) -> (TemporaryDirectory, ModuleGraph) {
    let canonical = canonical_standard_library_sources(overrides);
    for (path, _) in sources {
        assert!(
            !canonical
                .iter()
                .any(|(canonical_path, _)| canonical_path == path),
            "canonical module `{path}` must be supplied as an explicit override"
        );
    }
    let mut complete_sources = Vec::with_capacity(sources.len() + canonical.len());
    complete_sources.extend_from_slice(sources);
    complete_sources.extend(canonical);
    load_module_sources(entry, &complete_sources)
}

pub(crate) fn assert_system_assembler_accepts(output: &str) {
    let mut child = Command::new("cc")
        .args(["-x", "assembler", "-c", "-o", "/dev/null", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("native compiler tests require the Linux `cc` toolchain");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(output.as_bytes())
        .unwrap();
    let result = child.wait_with_output().unwrap();

    assert!(
        result.status.success(),
        "assembler rejected generated output:\n{}\nassembly:\n{output}",
        String::from_utf8_lossy(&result.stderr)
    );
}

pub(crate) fn assembly_relocations(output: &str) -> String {
    let object = TemporaryFile::new("assembly-object").unwrap();
    let mut child = Command::new("cc")
        .args(["-x", "assembler", "-c", "-o"])
        .arg(object.path())
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("relocation tests require the Linux `cc` toolchain");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(output.as_bytes())
        .unwrap();
    let assembled = child.wait_with_output().unwrap();
    assert!(
        assembled.status.success(),
        "assembler rejected generated output:\n{}\nassembly:\n{output}",
        String::from_utf8_lossy(&assembled.stderr)
    );

    let inspected = Command::new("readelf")
        .args(["--relocs", "--wide"])
        .arg(object.path())
        .output()
        .expect("relocation tests require the Linux `readelf` tool");
    assert!(
        inspected.status.success(),
        "readelf rejected generated object:\n{}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    String::from_utf8(inspected.stdout).expect("readelf output must be UTF-8")
}

pub(crate) fn run_native_assembly(output: &str) -> std::process::ExitStatus {
    let (_executable, mut command) = build_native_assembly(output);
    command.status().unwrap()
}

pub(crate) fn run_native_assembly_output(output: &str) -> std::process::Output {
    let (_executable, mut command) = build_native_assembly(output);
    command.output().unwrap()
}

/// Links generated assembly with the real runtime and the trace-chain probe
/// used by backend activation tests. The probe also validates at process exit
/// that the source entry function restored the outermost TLS link to null.
pub(crate) fn run_native_assembly_with_runtime_trace_probe(output: &str) -> std::process::Output {
    let executable = TemporaryFile::new("native-runtime-trace-executable").unwrap();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("compiler crate must live beneath the repository root");
    let include = repository.join("runtime/include");
    let runtime = repository.join("runtime/src");
    let probe = repository.join("tests/runtime/compiler_trace_probe.c");
    let mut child = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg("-I")
        .arg(include)
        .args(["-x", "assembler", "-", "-x", "c"])
        .arg(runtime.join("skald_runtime.c"))
        .arg(runtime.join("panic.c"))
        .arg(runtime.join("io.c"))
        .arg(probe)
        .arg("-Wl,--wrap=malloc")
        .arg("-o")
        .arg(executable.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("runtime-trace tests require the Linux `cc` toolchain");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(output.as_bytes())
        .unwrap();
    let linked = child.wait_with_output().unwrap();
    assert!(
        linked.status.success(),
        "linker rejected generated runtime-trace output:\n{}\nassembly:\n{output}",
        String::from_utf8_lossy(&linked.stderr)
    );

    Command::new(executable.path()).output().unwrap()
}

fn build_native_assembly(output: &str) -> (TemporaryFile, Command) {
    let executable = TemporaryFile::new("native-executable").unwrap();
    // Backend execution tests deliberately avoid depending on a prebuilt C
    // runtime. Supply only the link guard; driver and golden tests exercise
    // the real archive boundary.
    let panic_link_guard = if output.contains("\nska_rt_panic:\n") {
        ""
    } else {
        ".globl ska_rt_panic\n.type ska_rt_panic, @function\n\
         ska_rt_panic:\n    ud2\n.size ska_rt_panic, .-ska_rt_panic\n"
    };
    let linkable_output = format!(
        "{output}\n.section .tbss,\"awT\",@nobits\n\
         .p2align 3\n\
         .globl {1}\n.hidden {1}\n.type {1}, @object\n.size {1}, 8\n\
         {1}:\n    .zero 8\n\
         .text\n\
         .globl {0}\n.type {0}, @function\n{0}:\n    ret\n.size {0}, .-{0}\n\
         {panic_link_guard}",
        RUNTIME_ABI_MARKER_SYMBOL,
        crate::backend::RUNTIME_TRACE_TOP_SYMBOL,
    );
    let mut child = Command::new("cc")
        .args(["-x", "assembler", "-o"])
        .arg(executable.path())
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("native compiler tests require the Linux `cc` toolchain");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(linkable_output.as_bytes())
        .unwrap();
    let linked = child.wait_with_output().unwrap();
    assert!(
        linked.status.success(),
        "linker rejected generated output:\n{}\nassembly:\n{output}",
        String::from_utf8_lossy(&linked.stderr)
    );

    let command = Command::new(executable.path());
    (executable, command)
}

fn assert_phase_succeeded(phase: &str, diagnostics: &crate::diagnostics::Diagnostics) {
    assert!(
        diagnostics.is_empty(),
        "test source failed during {phase}: {:?}",
        diagnostics
    );
}

#[derive(Debug)]
pub(crate) struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    pub(crate) fn new(label: &str) -> io::Result<Self> {
        loop {
            let path = temporary_path(label, "dir");
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.path.join(path)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Debug)]
pub(crate) struct TemporaryFile {
    path: PathBuf,
}

impl TemporaryFile {
    pub(crate) fn new(label: &str) -> io::Result<Self> {
        loop {
            let path = temporary_path(label, "file");
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    drop(file);
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn temporary_path(label: &str, kind: &str) -> PathBuf {
    let id = NEXT_TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "skald-test-{kind}-{}-{id}-{label}",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn phase_helpers_leave_the_requested_boundary_for_the_test() {
        let (_, _, lexed) = lex_source("@");
        assert!(lexed.has_errors());

        let (_, parsed) = parse_source("fn");
        assert!(parsed.has_errors());

        let resolved = resolve_source("fn main() -> i64 { return missing; }");
        assert!(resolved.has_errors());

        let checked = type_check_source("fn main() -> i64 { return true; }");
        assert!(checked.has_errors());

        let mir = lower_source_to_mir("fn main() -> i64 { return 0; }");
        assert!(!mir.definitions.is_empty());
        assert!(
            lower_source_to_assembly("fn main() -> i64 { return 0; }", Target::X86_64SysV).is_ok()
        );
    }

    #[test]
    fn canonical_standard_library_closure_is_complete_and_overridable() {
        let canonical = canonical_standard_library_sources(&[]);
        assert_eq!(canonical.len(), 10);
        assert_eq!(canonical[0].0, "std/str.ska");
        assert_eq!(canonical[9].0, "std/test.ska");
        assert_eq!(
            canonical
                .iter()
                .map(|(path, _)| *path)
                .collect::<HashSet<_>>()
                .len(),
            canonical.len()
        );

        let replacement = "public fn replacement() -> unit {}\n";
        let overridden = canonical_standard_library_sources(&[("std/io.ska", replacement)]);
        assert_eq!(
            overridden
                .iter()
                .find(|(path, _)| *path == "std/io.ska")
                .unwrap()
                .1,
            replacement
        );
        assert_eq!(
            overridden
                .iter()
                .find(|(path, _)| *path == "std/str.ska")
                .unwrap()
                .1,
            CANONICAL_STR_SOURCE
        );
    }

    #[test]
    #[should_panic(expected = "standard-library override `std/missing.ska` is not canonical")]
    fn canonical_standard_library_closure_rejects_unknown_overrides() {
        canonical_standard_library_sources(&[("std/missing.ska", "")]);
    }

    #[test]
    #[should_panic(expected = "standard-library module `std/io.ska` is overridden more than once")]
    fn canonical_standard_library_closure_rejects_duplicate_overrides() {
        canonical_standard_library_sources(&[("std/io.ska", "first"), ("std/io.ska", "second")]);
    }

    #[test]
    fn temporary_directories_are_unique_and_removed_on_drop() {
        let first = TemporaryDirectory::new("unique").unwrap();
        let second = TemporaryDirectory::new("unique").unwrap();
        let first_path = first.path().to_owned();
        let second_path = second.path().to_owned();

        assert_ne!(first_path, second_path);
        assert!(first_path.is_dir());
        assert!(second_path.is_dir());
        drop(first);
        drop(second);
        assert!(!first_path.exists());
        assert!(!second_path.exists());
    }

    #[test]
    fn temporary_files_are_unique_and_removed_on_drop() {
        let first = TemporaryFile::new("unique").unwrap();
        let second = TemporaryFile::new("unique").unwrap();
        let first_path = first.path().to_owned();
        let second_path = second.path().to_owned();

        assert_ne!(first_path, second_path);
        assert!(first_path.is_file());
        assert!(second_path.is_file());
        drop(first);
        drop(second);
        assert!(!first_path.exists());
        assert!(!second_path.exists());
    }

    #[test]
    fn temporary_resources_are_removed_during_assertion_unwinding() {
        let directory = TemporaryDirectory::new("unwind").unwrap();
        let file = TemporaryFile::new("unwind").unwrap();
        let directory_path = directory.path().to_owned();
        let file_path = file.path().to_owned();

        let result = std::panic::catch_unwind(move || {
            let _directory = directory;
            let _file = file;
            panic!("simulated failed assertion");
        });

        assert!(result.is_err());
        assert!(!directory_path.exists());
        assert!(!file_path.exists());
    }
}
