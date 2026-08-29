# Driver and Artifacts

Status: authoritative for compiler orchestration, command-line behavior,
target and toolchain selection, runtime archive selection, output publication,
and driver failure boundaries. Compiler phases are owned by
[Phases and IR](PHASES_AND_IR.md), target emission by the
[Backend and Target Contract](BACKEND.md), and the linked C surface by the
[Runtime ABI](RUNTIME_ABI.md). Multiple-file CLI, provider, and entry behavior
is owned by the
[Module-System Compiler Contract](MODULE_SYSTEM.md). The operational-reporting
boundary and its remaining planned extensions are owned by
[Structured Compiler Reporting](REPORTING.md).

## Driver facade

The repository-internal `skald_compiler::driver` facade exposes seven ways to
compose the compiler:

- `compile_request_to_assembly` loads and compiles the selected entry's
  reachable module program from a typed `CompilationRequest`;
- `compile_request_to_assembly_observed` performs the same work through an
  explicitly supplied request-local `ReportObserver`;
- `compile_source_to_assembly` runs one in-memory source through the complete
  semantic, MIR, backend, and assembly pipeline without filesystem discovery;
- `compile_source_to_assembly_observed` performs that singleton compilation
  with explicit lexing, parsing, shared compiler-phase, and total events;
- `Toolchain::link_assembly` sends assembly to a configured host compiler
  driver and publishes the linked executable;
- `Toolchain::link_assembly_with` preserves the same link command, runtime
  validation, failure interpretation, and publication policy while allowing
  repository tooling to supply a bounded process executor; and
- `run_cli` owns process arguments, source and output I/O, diagnostic
  rendering, toolchain selection, and process exit status.

`crates/skac` is deliberately only a process entry point: it forwards
`args_os()` to `run_cli` and exits with the returned status. The compiler crate
is unpublished and does not promise a version-stable API outside this
repository; see the [compiler crate API policy](README.md#compiler-crate-api-policy).

The facade exposes the typed `CompilationRequest` contract:
`EntrySelector`, repeatable module-root paths, `StandardLibrarySelection`,
`Target`, `ArtifactOptions`, and an explicit `CompilationEnvironment`.
Construction resolves mutually exclusive entry and standard-library option
forms but performs no filesystem access. Request compilation normalizes the
selected ordinary and standard-library roots, loads only the reachable parsed
graph, resolves and checks one whole program, runs the verified MIR pipeline,
and emits target assembly.

## Compilation orchestration

`compile_request_to_assembly(&request)` owns provider normalization, reachable
filesystem loading, whole-program resolution and type checking, MIR lowering,
the verified MIR pass pipeline, and target assembly emission. Provider
configuration failures remain structured separately from source diagnostics.
The returned report owns every reached source and diagnostic.

The observed request and singleton forms emit typed start/finish events at
these existing boundaries plus one compilation total. Source-diagnostic
failures produce a failed owning phase and stop later phase starts. Details
observers receive deterministic phase-owned metrics, and trace observers also
receive discovery/final module parse events. The quiet forms delegate through
`NoopObserver`; observation does not change returned assembly, diagnostics,
failure categories, or source ownership.

Both public compilation adapters request closed-world target artifact
retention. Verified HIR and MIR stay complete, while functions and data not
reachable from an exported machine symbol are omitted from published assembly.
Direct backend consumers may retain complete output when inspecting lowering
of otherwise uncalled MIR definitions.

`compile_source_to_assembly(path, text, target)` is the in-memory singleton
adapter. Its path labels diagnostics but is never read, and it gains no module
root discovery. After lexing and parsing, it uses the same program resolver,
type checker, MIR pipeline, and backend completion path as request
compilation. A source-phase error stops later phases. HIR lowering, MIR
verification, and backend failures remain distinct structured categories.
After structural preliminary-MIR verification, static effect inference and
lifetime planning run before final MIR conversion. The resulting explicit
lifecycle definitions, transitions, summaries, dynamic targets, dependency
edges, and plan indices are verified again at a dedicated trust boundary.
Static self-dependencies and cycles are ordinary source diagnostics; malformed
preliminary or planned MIR remains a distinct verification failure. A valid
explicit initializer is synthesized into final coordinator MIR and passes the
ordinary target-independent verifier pipeline. The x86-64 backend then emits
private initializer bodies and a dependency-ordered program initializer, and
the existing host wrapper calls it after the runtime ABI marker and before the
selected Skald entry function.
Static inheritance, inherited access, class/`Obj` alias views, and inline
slicing reach verified target-independent MIR and execute through the current
x86-64 base layout and internal static-view calling convention.

None of the four compilation APIs invokes the host toolchain or publishes an
artifact. Linking and publication therefore do not enter their compilation
totals.

The runtime-trace extension changes the final handoff only: the driver
passes final verified MIR, the report's read-only `SourceDatabase`, and the
selected trace policy together to backend emission. Source ownership remains
with the compilation report, and no target trace record or rendered path is
written back into MIR. Both request compilation and the in-memory singleton
adapter enable tracing by default; `ArtifactOptions` carries the explicit
policy used by request compilation.

## Command-line modes

`skac --help` is the exact option reference. One invocation requires exactly
one positional `.ska` entry or one logical `--entry module::path`. The forms
are mutually exclusive. `--module-root <directory>` is repeatable;
`--stdlib-root <directory>` replaces the installed standard-library root and
is mutually exclusive with `--no-stdlib`.

| Entry | Executable default | `--emit asm` default |
|---|---|---|
| `app/main.ska` | `app/main` | `app/main.s` |
| `--entry app::main` | `main` | `main.s` |

`-o` or `--output` selects another destination. Assembly mode runs the same
frontend and backend but does not require a runtime archive or invoke the host
toolchain. `--version`, `-h`, and `--help` complete without compilation.

Runtime traces are enabled by default for both executable and assembly modes.
`--omit-runtime-trace` selects compile-time omission for that
invocation. It is a trace policy rather than a runtime toggle: omitted output
contains no trace frame homes, TLS maintenance, trace metadata, or trace-only
source lookup. The option takes no value and repeated use is a command-usage
error.

For example, split application, dependency, and SDK trees compose without
source-visible root bindings:

```text
skac --entry app::main \
  --module-root application/modules \
  --module-root dependencies/modules \
  --stdlib-root sdk/modules
```

Imports in those sources use only logical paths such as `app::model`,
`math::geometry`, or `std::Str`.

Logical paths and target/emission names require UTF-8. Positional files,
provider roots, standard-library roots, and output paths retain native OS
strings. Loading reads every reached source as UTF-8 text. Invalid entries,
missing modules, unreadable or malformed reached sources, ambiguity, and
provider failures are compilation failures with structured diagnostics or
configuration errors.

## Planned structured reporting

The current CLI has no reporting or diagnostic-display level options. The
frozen [structured reporting contract](REPORTING.md) now provides typed events,
metrics, observers, deterministic human rendering, and observed library
compilation. The CLI still uses the quiet request wrapper and therefore emits
no operational text. Planned selection will use `-v`/`-q`, `--report-level`, and
`--diagnostic-level`; source diagnostics remain a separate structured channel
and a failed compilation can never hide its errors. Linking and artifact
publication are driver events, not compiler-phase events. The
[implementation roadmap](../roadmaps/STRUCTURED_REPORTING_ROADMAP.md) owns the
rollout; until its tasks land, `skac --help` and this document's other sections
remain authoritative for implemented command-line behavior.

## Target selection

`--target <name>` is resolved through the public backend registry. Omitting it
uses `backend::DEFAULT_TARGET_NAME`; an unsupported name is a usage error and
never silently falls back. The current registry and target-specific behavior
are authoritative in the [backend contract](BACKEND.md#backend-interface-and-target-registry).

## Host toolchain and runtime selection

Executable mode streams generated assembly to the configured C compiler
driver through standard input. It constructs a subprocess directly rather
than a shell command. The invocation treats stdin as assembler input, passes
the runtime archive as a link input, and asks the tool to write to the pending
executable path.

The default configuration is:

| Setting | Default | Override |
|---|---|---|
| Host compiler driver | `cc` | `CC` |
| Runtime archive | `build/runtime/libskald_runtime.a` | `SKALD_RUNTIME_ARCHIVE` |
| Installed standard-library root | repository `std/` installation path | `SKALD_STDLIB_ROOT` |

`CC` names one executable path; it is not parsed as a shell fragment or a list
of flags. The runtime path must identify an existing regular file before the
tool is started. Runtime ABI compatibility is then enforced by the
[version-specific link marker](RUNTIME_ABI.md#version-and-link-compatibility).

The driver captures tool stdout and stderr. Start, input-write, wait, nonzero
termination, and publication failures are returned as structured
`ToolchainError` categories. A nonzero tool result includes its exit status or
signal state and captured details in the user-facing error.

The golden runner uses the bounded executor form so compiler, linker, and
generated-program timeouts share one process-group policy. `Toolchain` still
constructs the exact host command, owns its pending output, interprets the
captured result, and publishes the executable atomically; the runner does not
reimplement those driver responsibilities. Native selections prepare one
runtime archive, compile each selected build to checked assembly, and ask the
same `Toolchain` API to link each assembly into its independently owned golden
artifact directory.

## Input protection and artifact publication

For a positional entry, an explicit output is rejected when existing file
metadata shows that it is the selected input itself, a symbolic link to it, or
a hard link to it. The check compares the resolved Unix device and inode. It
does not broaden alias policy to imported files or physically shared module
candidates. Logical entries have no selected input file for this check.

Assembly and executable outputs use the same publication protocol:

1. reserve a unique temporary file in the destination directory;
2. write assembly there or direct the host toolchain to that path;
3. leave any existing destination untouched until work succeeds; and
4. publish with one same-directory rename.

Ordinary failure and unwind paths remove the unpublished temporary through its
owner. Compilation, linking, and publication failures therefore preserve an
existing destination; no partial result is intentionally published. The
destination directory must already exist and permit temporary-file creation
and rename.

## Diagnostics and exit status

Source diagnostics use the compiler's structured renderer. A valid feature
whose next IR stage is not implemented is reported as a compiler-stage
limitation. Invalid MIR or backend failures are reported as internal compiler
failures, while host-tool and artifact errors retain their driver category.
User-controlled failures do not become compiler panics.

The CLI process statuses are:

| Status | Meaning |
|---:|---|
| `0` | Help, version, or compilation completed successfully. |
| `1` | Provider setup, reached source/module compilation, internal verification/backend processing, or host toolchain failed. |
| `2` | Command usage, target selection, source suffix, or input/output alias was invalid. |
| `74` | Working-directory, artifact, or command-output I/O failed. |

Exact diagnostics are tested at their owning source, CLI, artifact, or
toolchain boundary. Host operating-system and tool messages are retained as
details and are not portable compiler wording.

## Verification

Driver tests are divided by responsibility:

- CLI tests cover help, version, selectors, roots, argument rejection, output
  defaults, suffix, target, trace omission, and OS-string rules;
- pipeline tests compose singleton and request-based whole-program phases,
  trace policy/source handoff, and structured failures;
- artifact tests cover assembly output, source alias rejection, preservation,
  and temporary cleanup;
- toolchain tests cover missing archives, process failures, unresolved
  externals, version-8/version-9 ABI mismatch, captured status, and executable
  preservation; and
- `crates/skac` integration tests exercise both entry forms, repeated roots,
  standard-library selection, relative and non-UTF-8 paths, and output
  publication through the real binary entry point.

Complete native golden cases additionally cover the real compiler process,
runtime archive, linker, published executable, stdout, stderr, and process
status.
