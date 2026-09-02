# Driver and Artifacts

Status: authoritative for compiler orchestration, command-line behavior,
target and toolchain selection, runtime archive selection, output publication,
and driver failure boundaries. Compiler phases are owned by
[Phases and IR](PHASES_AND_IR.md), target emission by the
[Backend and Target Contract](BACKEND.md), and the linked C surface by the
[Runtime ABI](RUNTIME_ABI.md). Multiple-file CLI, provider, and entry behavior
is owned by the
[Module-System Compiler Contract](MODULE_SYSTEM.md). The implemented
operational-reporting boundary and its explicitly deferred extensions are
owned by [Structured Compiler Reporting](REPORTING.md).

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
`Target`, `ArtifactOptions`, `MirOptimizationOptions`, and an explicit
`CompilationEnvironment`.
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
Structural preliminary-MIR verification consumes the raw product and returns
an opaque read-only `VerifiedPreliminaryMirProgram`. Static-effect inference
and lifetime planning accept only that seal and run before final MIR
conversion; malformed preliminary identities therefore cannot reach their
otherwise infallible adapters. Planned verification
consumes the draft product, checks its canonical lifecycle definitions,
activation order, compact authority, dynamic targets, and derived dependency
edges, and returns the only sealed planned product accepted by synthesis.
Shutdown, positions, and planned transition views are reconstructed from that
canonical data.
Static self-dependencies and cycles are ordinary source diagnostics; malformed
preliminary or planned MIR remains a distinct verification failure. A valid
explicit initializer is synthesized directly into structured final coordinator
regions and passes the ordinary target-independent verifier pipeline exactly
once after all registered passes. The pipeline returns the sealed, read-only
final product required by backend input. These
regions are the sole executable lifecycle representation consumed by both
verification and the backend. The x86-64 backend then emits private initializer
bodies and a dependency-ordered program initializer, and the existing host
wrapper calls it after the runtime ABI marker and before the selected Skald
entry function.
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

Structured reporting retains separate planning, planned-verification,
synthesis, MIR-pipeline, and backend phases. Planned and final verification
executions are counted by their owning phases; synthesis is infallible after
its sealed input, and the backend does not hide another target-independent
verification execution.

## Final-MIR optimization selection

The confirmed
[selectable final-MIR pipeline design](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_DESIGN_PROPOSAL.md)
adds target-independent optimization policy to the typed compilation request
and CLI. Its
[completed implementation roadmap](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
records delivery. Typed request/CLI selection, deterministic scheduling, the
verified multi-pass runner, reporting, checkpoints, and the first production
pass are implemented.

`CompilationRequest` contains a typed `MirOptimizationOptions` value and the
non-breaking `with_mir_optimization` builder. Options select a typed
`MirOptimizationProfile` and a canonical lexical set of disabled stable pass
names; duplicate disabling is idempotent in request identity. Existing request
construction and singleton compilation helpers select `default`. The
supported profiles are `none` and `default`: `none` resolves to the empty
verification-only schedule, while `default` contains
`dead-pure-definition-elimination` followed by `whole-world-reachability`,
each exactly once. Disabling both passes from `default`, including duplicate
disabling, resolves to the same schedule and product as `none`. `none` remains
the reference unoptimized mode and preserves raw final MIR after its required
central verification.

The implemented command-line surface is:

```text
--mir-optimization <none|default>
--disable-mir-pass <name>
--list-mir-passes
```

These options are implemented. `--mir-optimization` may appear once.
`--disable-mir-pass` is repeatable and
removes every occurrence of the named pass from the selected profile;
duplicate disabling is idempotent. Unknown profile or pass names are usage
errors before provider or source I/O, and unknown and known pass-name lists are
sorted lexically. The current registry contains the stable
`dead-pure-definition-elimination` and `whole-world-reachability` names.
`--list-mir-passes` succeeds without
an input file and prints every registered stable name and description in
lexical name order. Library tools can inspect the same canonical metadata
through `passes::available_mir_passes` and `MirPassDescriptor`; neither query
constructs a schedule or performs source or provider I/O. The CLI
does not initially expose arbitrary pass order, `-O`, or numeric optimization
levels. A crate-private exact-schedule API belongs to compiler tests and tools,
not the public driver policy.

Optimization options are semantic compilation configuration and participate
in request equality. Pass reports and verified MIR checkpoint observers are
invocation services: they do not live in the request or affect compilation
identity. Profile selection and exclusions are independent of target,
artifact kind, runtime-trace policy, diagnostic presentation, and operational
report detail. Selection never changes source acceptance or diagnostics;
malformed pass output remains a structured compiler failure rather than a
source diagnostic. The final-MIR runner distinguishes input verification, pass
execution, structural rewrite, and changed-output verification failures. A
pass-attributed failure carries its exact stable name, internal identity,
schedule position, and occurrence number, and the driver stops before backend
emission without exposing a partial product.

Verified checkpoint inspection is implemented by the pass facade's
`run_mir_pipeline_inspected` entry point rather than by
`CompilationRequest` or `ReportObserver`. The ordinary driver supplies no
inspector and performs no checkpoint work. General driver/CLI dump destination
and retention policy remain intentionally deferred.

Verified static-activation inspection uses the same request-local separation
without becoming a selectable MIR checkpoint. Tools opt into
`compile_request_to_assembly_observed_inspected` or
`compile_source_to_assembly_observed_inspected` and receive exactly one
borrowed `StaticActivationInspection` after planned-MIR verification. Its
label and allocation-free statistics may be queried without formatting; its
focused dump is built only by an explicit `activation_dump` call. The ordinary
compile adapters pass no inspector, while `CompilationRequest`, report detail,
diagnostics, generated artifacts, and CLI options remain unchanged.

## Frozen local final-MIR simplification selection direction

The confirmed
[local final-MIR simplification design](../roadmaps/LOCAL_FINAL_MIR_SIMPLIFICATION_DESIGN_PROPOSAL.md)
and active
[roadmap](../roadmaps/LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md) extend the
existing registry/profile/exclusion surface without adding a request field or
CLI category. `primitive-constant-folding` is now registered and appears
through pass discovery, exact compiler-internal schedules, lexical known-name
diagnostics, and pass-attributed failures. It is not yet part of `default`.

Delivery will additionally register the stable names
`primitive-algebraic-simplification` and `conservative-cfg-cleanup`. Together
the three local passes appear through the same
`passes::available_mir_passes` query, `--list-mir-passes` output, lexical known-
name diagnostics, and pass-attributed errors. Numeric pass identities remain
private.

The future `default` profile will contain dead-pure elimination, constant
folding, algebraic simplification, repeated constant/dead-pure cleanup,
conservative CFG cleanup, final dead-pure cleanup, and whole-world reachability
in the exact order frozen by the compiler phase contract. `none` remains
empty. `--disable-mir-pass <name>` removes every occurrence of a repeated pass,
and disabling all five stable names must equal `none`.

No arbitrary pass ordering, `-O` level, dynamic plugin, target-specific pass
selection, or optimization-dependent static activation is added. Selection
continues to be resolved before provider/source I/O, and malformed transformed
MIR remains a structured compiler failure before backend emission or artifact
publication.

## Whole-world reachability selection

The confirmed
[whole-world reachability design](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_DESIGN_PROPOSAL.md)
uses the existing final-MIR registry, profile, exclusion, listing, and verified
runner contracts. It adds no new request field or command-line category.

The stable registered name is `whole-world-reachability`.
`--list-mir-passes`, the public descriptor query,
unknown-name diagnostics, and repeatable `--disable-mir-pass` selection
discover it through the same canonical registry metadata as every other pass.
The supported `default` schedule contains `dead-pure-definition-elimination`
followed by `whole-world-reachability`, while `none` remains empty.

Disabling reachability preserves complete final MIR and the prior backend
input domain. Disabling both registered passes from `default` must match
`none`. Selection never changes source loading, acceptance, diagnostics,
static-lifecycle planning, target choice, runtime-trace policy, artifact paths,
or publication behavior. A pass or changed-output verification failure stops
before backend emission and artifact publication through the existing
structured pipeline error boundary.

## Frozen static activation orchestration

Status: **implemented**. After successful preliminary-MIR verification, the
static-lifecycle planning boundary extracts shared dependencies and computes
the exact activation closure. Under the accepted
[reachability-gated contract](PHASES_AND_IR.md#frozen-reachability-gated-static-lifecycle-direction),
the driver runs exact static activation once after verified preliminary MIR
and before the eager plan is built. That analysis is mandatory compiler semantics and
is not represented by `MirOptimizationProfile`, `--mir-optimization`, or
`--disable-mir-pass`.

The resulting immutable active authority flows through planned
verification, synthesis, final verification, and backend input. The driver may
adapt its already-known counts to structured reporting and expose its focused
dump through request-local inspection, but cannot select, expand, narrow, or
recompute activation after final-MIR optimization. `none`, `default`, selective
pass disabling, assembly emission, executable linking, and every target must
therefore share one active set and identical static startup/shutdown effects.

No CLI flag, request field, environment setting, module-loading switch, or
target option forces eager or lazy activation. Whole-world
source loading and checking remain unchanged, and single-threaded generated
execution adds no runtime coordination path.

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

## Structured reporting and diagnostics

The CLI implements the frozen [structured reporting contract](REPORTING.md).
Operational selection defaults to off. Repeatable `-v` and `-q` use saturating
subtraction, while `--report-level off|phases|details|trace` selects an explicit
level and cannot be combined with either shorthand. Repeated or invalid
explicit options are usage errors.

Operational reports go only to stderr. Phase detail reports progress, outcome,
artifact notices, and compilation/driver totals. Details add elapsed durations
and phase-owned metrics; trace adds discovery/final module parse records.
Executable mode reports host linking before publication. Assembly and
executable modes report atomic publication and emit the artifact notice only
after the final rename succeeds. Default compilation remains silent when it
has no source diagnostic or driver error.

Diagnostic visibility is independent. `--diagnostic-level warning` is the
default and renders warnings plus errors; `--diagnostic-level error` filters
only warnings at this CLI boundary. Every diagnostic remains in the library
`CompilationReport`, and source errors are always rendered. There is no
diagnostic-off mode.

The CLI retains the text observer's first writer failure through command
execution and returns status 74 at the process boundary. That presentation
failure does not enter `CompilationError` or cancel artifact production.

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

Report output failure is command-output I/O and therefore status 74 even when
compilation and artifact publication completed successfully.

Exact diagnostics are tested at their owning source, CLI, artifact, or
toolchain boundary. Host operating-system and tool messages are retained as
details and are not portable compiler wording.

## Verification

Driver tests are divided by responsibility:

- CLI tests cover help, version, selectors, roots, argument rejection, report
  and diagnostic levels, output defaults, suffix, target, trace omission, and
  OS-string rules;
- pipeline tests compose singleton and request-based whole-program phases,
  trace policy/source handoff, and structured failures;
- artifact tests cover assembly output, source alias rejection, preservation,
  and temporary cleanup;
- toolchain tests cover missing archives, process failures, unresolved
  externals, version-8/version-9 ABI mismatch, captured status, and executable
  preservation; and
- CLI reporting tests cover compiler/link/publication phase order, separate
  totals, failures, diagnostic filtering, default-off byte stability,
  concurrent destination isolation, quiet-path gating, and retained writer
  errors; and
- `crates/skac` integration tests exercise both entry forms, the detail ladder,
  repeated roots, standard-library selection, relative and non-UTF-8 paths,
  default-versus-explicit-off process bytes, structural failed reports, status
  74, and assembly/executable publication through the real binary entry point.

Complete native golden cases additionally cover the real compiler process,
runtime archive, linker, published executable, stdout, stderr, and process
status.
