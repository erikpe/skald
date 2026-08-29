# Structured Compiler Reporting Design Proposal

Status: frozen design proposal. SR1 through SR10 were confirmed together on
2026-08-29 and promoted into the focused
[compiler reporting contract](../compiler/REPORTING.md) before roadmap
creation. The [implementation roadmap](../roadmaps/STRUCTURED_REPORTING_ROADMAP.md)
owns delivery; this document preserves the reviewed decisions.

This proposal adds request-scoped structured reporting to the Skald compiler.
It covers phase progress, elapsed time, aggregate and pass-owned statistics,
artifact notices, and configurable presentation without turning source
diagnostics or IR dumps into ordinary log strings. The same compiler pipeline
should remain quiet by default, observable through the CLI when requested, and
embeddable with a caller-supplied event observer.

The design takes the useful severity-and-detail lessons from the sibling
Niflheim compiler while adapting them to Skald's explicit compilation request,
structured diagnostics, deterministic phase products, repository-internal
library API, and potentially concurrent tooling callers.

## Intended outcome

The initial reporting system should provide:

- no operational output for an ordinary successful compilation;
- unchanged structured source diagnostics and driver error categories;
- opt-in phase progress, phase and total elapsed time, aggregate statistics,
  per-module detail, pass detail, and artifact-publication notices;
- independent control over operational reporting and warning visibility;
- stderr-only human-readable CLI reporting so stdout and artifacts remain
  clean;
- typed events and typed metric values rather than preformatted strings;
- one observer per compilation invocation, with no global logger or process
  subscriber;
- a no-op observer for existing compilation APIs and callers;
- a recording observer for deterministic tests and programmatic inspection;
- reporting of both successful and failed phase completion without duplicating
  the actual diagnostic or driver error;
- lazy collection of detail that would otherwise require nontrivial work;
- explicit accounting for repeated source discovery and final parsing work;
- a facade-oriented module whose renderer and implementation details can
  evolve without changing phase ownership; and
- an event model that can support a later machine-readable renderer without
  making a JSON schema part of the first implementation contract.

A representative CLI interaction is:

```text
$ skac app/main.ska -o build/main -vv
skac: phase: loaded 7 modules in 3.42 ms
skac: stats: source_bytes=18421, lex_runs=14, parse_runs=14, tokens=5932
skac: phase: resolved program in 2.08 ms
skac: phase: type checked program in 4.71 ms
skac: phase: lowered and verified MIR in 1.36 ms
skac: phase: emitted x86_64-sysv assembly in 2.44 ms
skac: stats: assembly_bytes=28731, assembly_lines=812
skac: artifact: published executable build/main
skac: finished in 16.83 ms
```

The exact wording and grouping are renderer concerns. Phase and metric
identities are structured before rendering.

## Current boundary and architectural evidence

Skald already owns the principal data and boundaries that reporting should
observe:

- `CompilationRequest` explicitly owns entry, roots, standard-library
  selection, target, artifact policy, and process-dependent environment;
- `compile_request_to_assembly` visibly sequences provider normalization,
  reachable loading, whole-program resolution, checking, MIR construction,
  lifecycle planning, the MIR pass pipeline, and backend emission;
- `run_cli` separately owns diagnostic rendering, host linking, artifact
  publication, process streams, and process status;
- source diagnostics are structured as severity, code, message, labels, and
  notes, and are rendered deterministically from the source database;
- dumps for resolved IR, typed HIR, MIR, module graphs, and backend products
  are phase-owned inspection products rather than incidental messages;
- the MIR pass pipeline is an explicit boundary even though it currently runs
  verification without transformations; and
- the repository-internal compiler API is used by tests and tooling, so CLI-
  local logging would leave important callers unable to observe compilation.

The architecture requires sources, diagnostics, targets, phase products, and
artifacts to belong to a request rather than hidden globals. Reporting must
follow the same rule. It may observe a phase product but must neither own the
phase decision nor introduce a backward dependency from a phase to the
driver.

There are also current details that prevent a shallow timing wrapper around
the top-level CLI from being sufficient:

- reachable module loading parses each staged source during dependency
  discovery and again when constructing the final graph;
- the in-memory singleton adapter performs lexing and parsing outside the
  request-based module loader, then joins the shared semantic and backend
  completion path;
- static lifecycle planning contains distinct construction, planning,
  verification, and synthesis trust boundaries; and
- executable linking and artifact publication occur after the assembly
  compilation API returns.

The reporting contract must therefore span library compilation and driver
completion while preserving their existing ownership boundary.

## Niflheim precedent

Niflheim's compiler logging provides four severity levels, a separate integer
verbosity filter, repeatable `-v` and `-q` flags, phase progress at `info`, and
timings and statistics at verbose `debug`. Its resolver aggregates token,
file, parse-stream, and duration counters. Optimization pipelines time each
registered pass, while individual passes own counters such as successful
folds, removed statements, simplified branches, and eliminated backend
instructions. CLI tests verify filtering, stderr routing, phase ordering, and
the presence of pass statistics.

Those are useful behavior precedents:

- quiet default behavior;
- a compact CLI detail ladder;
- separate high-level progress and detailed measurements;
- phase wrappers around monotonic timing;
- statistics collected by the responsibility that understands them; and
- exact stream and filtering tests.

Skald should not copy Niflheim's global Python logger configuration or its
preformatted message API. A global logger conflicts with request-local state
and parallel callers. Strings such as “removed 12 instructions” discard the
identity, unit, and value needed by recording observers or later formats.
Niflheim also routes some compiler warnings through the logger; Skald already
has a stronger structured diagnostic model and should retain it.

## Design principles

1. **Diagnostics are not logs.** A source warning or error remains structured
   compiler output with a code and source labels.
2. **Observation is request-scoped.** One invocation receives one explicit
   observer; no process-global configuration affects another invocation.
3. **Events precede presentation.** Phases publish typed facts, while sinks
   decide whether and how to render them.
4. **Reporting is non-semantic.** Enabling or disabling reporting cannot change
   phase order, diagnostics, IR, assembly, artifacts, or exit meaning.
5. **Failures have one source of truth.** A failed phase event records outcome
   and duration but does not copy or replace its diagnostic or driver error.
6. **Metrics have owners.** The phase or pass that understands a count defines
   and populates it; the driver does not rediscover private implementation
   facts by inspecting rendered output.
7. **Work is measured honestly.** Repeated discovery, parsing, verification,
   or pass executions are counted as executions rather than collapsed into a
   misleading source count.
8. **Default output remains stable.** Existing successful compiler, golden,
   and tooling invocations gain no new bytes unless reporting is requested.
9. **Nondeterministic values are opt-in.** Timings never enter deterministic
   dumps, diagnostics, assembly, default golden observations, or cache keys.
10. **The first system stays small.** A dedicated observer and renderer are
    sufficient; a general application logging framework and stable external
    telemetry protocol are not prerequisites.

## Vocabulary and boundaries

This proposal uses these terms consistently:

- **diagnostic** — structured source warning or error, with code, labels, and
  notes, owned by `diagnostics` and retained in `CompilationReport`;
- **driver error** — provider configuration, internal verification, backend,
  toolchain, publication, usage, or command-output failure represented by its
  existing typed category;
- **report event** — a typed, non-semantic observation such as phase start,
  phase completion, statistics, or artifact publication;
- **detail level** — the requested amount of operational reporting;
- **metric** — one named typed value with a unit, emitted in deterministic
  order by its owner;
- **observer** — a request-local consumer of report events;
- **renderer** — a presentation component that turns events into text or a
  future machine-readable representation; and
- **dump** — a potentially large deterministic phase product written or
  printed through an explicit dump option, never embedded as a report event.

## Decision register

| ID | Question | Confirmed direction | State |
|---|---|---|---|
| [SR1](#sr1--separate-reporting-from-diagnostics-and-dumps) | What belongs to structured reporting? | Operational events only; diagnostics, driver errors, and dumps retain separate owners | **Confirmed** |
| [SR2](#sr2--request-scoped-observer-composition) | How does reporting enter the compiler? | An explicit observer passed to observed compilation adapters, with existing quiet wrappers | **Confirmed** |
| [SR3](#sr3--typed-event-and-metric-model) | What is reported? | Typed phase, outcome, metric, and artifact events with owned values and deterministic ordering | **Confirmed** |
| [SR4](#sr4--cli-configuration-and-warning-policy) | How is reporting configured? | An operational detail ladder independent from diagnostic severity | **Confirmed** |
| [SR5](#sr5--phase-timing-and-failure-semantics) | How are phases and timings represented? | Monotonic start/finish observation, including failed completion, with no exact-time correctness claims | **Confirmed** |
| [SR6](#sr6--statistics-ownership-and-source-loading-accounting) | Where do counters come from? | Phase-owned sidecars and pass results, including explicit discovery/final parse execution counts | **Confirmed** |
| [SR7](#sr7--rendering-streams-and-output-stability) | Where and how is output rendered? | Human text to stderr initially; default streams unchanged; machine format deferred | **Confirmed** |
| [SR8](#sr8--module-and-public-api-organization) | Where does implementation live? | A top-level `reporting` facade plus driver-local CLI configuration | **Confirmed** |
| [SR9](#sr9--verification-determinism-and-overhead) | How is the system tested and bounded? | Typed recording tests, fixed renderer fixtures, CLI stream tests, and disabled-path overhead checks | **Confirmed** |
| [SR10](#sr10--promotion-and-delivery-boundary) | How does the proposal become implementation work? | Freeze and promote the contract before creating a PR-sized implementation roadmap | **Confirmed** |

## SR1 — Separate reporting from diagnostics and dumps

Structured reporting should carry only operational observations:

- phase start and completion;
- elapsed phase and total time;
- aggregate source, semantic, IR, backend, and artifact metrics;
- per-module or per-pass detail at the highest detail level; and
- successful artifact publication.

Source diagnostics remain `Diagnostic` values. Reporting configuration must
not change their construction, codes, labels, notes, ordering, or
`CompilationReport` ownership. A distinct diagnostic-display policy may hide
warnings at the CLI boundary, but errors are always rendered and warning
filtering never mutates the underlying report.

Provider, verifier, backend, toolchain, usage, and publication failures retain
their existing error types and exit-status mapping. When such a failure ends a
phase, reporting emits only a failed completion observation. The CLI then
renders the actual error once through its existing owner.

IR and analysis dumps remain explicit outputs. A dump request may produce an
`ArtifactPublished` or later `DumpPublished` notice naming its destination,
but raw multiline dump contents do not become message fields. This keeps
diagnostic, report, and dump output independently selectable and testable.

## SR2 — Request-scoped observer composition

The compiler should expose an object-safe observer approximately equivalent
to:

```rust
pub trait ReportObserver {
    fn enabled(&self, detail: ReportDetail) -> bool;
    fn observe(&mut self, event: ReportEvent);
}
```

`enabled` lets an owner avoid constructing expensive optional statistics.
Basic phase timings may be collected unconditionally inside an observed
adapter because `Instant` reads are cheap, but detailed graph or IR scans must
be guarded.

The driver facade should preserve the current quiet APIs and add explicit
observed forms:

```rust
pub fn compile_request_to_assembly(
    request: &CompilationRequest,
) -> Result<AssemblyArtifact, CompilationError>;

pub fn compile_request_to_assembly_observed(
    request: &CompilationRequest,
    observer: &mut dyn ReportObserver,
) -> Result<AssemblyArtifact, CompilationError>;
```

The existing function delegates through `NullReportObserver`. The in-memory
singleton adapter receives an equivalent observed form so tests and tools do
not lose lexing and parsing events merely because they avoid filesystem module
loading.

The observer is an invocation service, not semantic request configuration. It
therefore should not be stored inside `CompilationRequest`, participate in
`Clone` or `Eq`, enter deterministic request identity, or be visible to
compiler phases that do not report events. Passing it explicitly still
satisfies request-local state and allows independent concurrent invocations.

`observe` is deliberately infallible. A generic recording observer has no I/O
failure. A text observer over a fallible writer records its first write error,
stops attempting output, and exposes that error to the process-level driver
after compilation returns. Command-output failure then retains exit status 74
without changing compilation semantics or adding reporting failures to
`CompilationError`.

The first implementation does not require `Send`, `Sync`, global installation,
or cross-thread ordering guarantees. If an individual compiler invocation
later runs work concurrently, that owner must define deterministic collection
and replay or adopt a separately reviewed synchronized observer contract.

## SR3 — Typed event and metric model

The event model should use enums and typed values rather than a generic log
record containing a severity and message. An indicative shape is:

```rust
pub enum ReportEvent {
    PhaseStarted {
        phase: ReportPhase,
    },
    PhaseFinished {
        phase: ReportPhase,
        elapsed: Duration,
        outcome: ReportOutcome,
        metrics: Vec<ReportMetric>,
    },
    ArtifactPublished {
        kind: ReportArtifactKind,
        path: PathBuf,
    },
    RunFinished {
        scope: ReportScope,
        elapsed: Duration,
        outcome: ReportOutcome,
    },
}

pub enum MetricValue {
    Count(u64),
    Bytes(u64),
}

pub struct ReportMetric {
    pub name: &'static str,
    pub value: MetricValue,
}

pub enum ReportScope {
    Compilation,
    Driver,
}

pub enum ReportArtifactKind {
    Assembly,
    Executable,
    Dump,
}
```

Exact Rust names may change during implementation, but these invariants are
part of the proposed contract:

- phases and outcomes are enums rather than arbitrary user strings;
- elapsed time is `Duration`, not a pre-rounded floating-point number;
- metrics retain a unit and integer value;
- one event owns the values needed by a recording observer;
- metric emission order is deterministic and documented by the owner;
- human labels, pluralization, alignment, and rounding belong to the text
  renderer; and
- events contain no references to large phase products or source text.

Reporting owns `ReportArtifactKind` rather than depending on the driver's
artifact-option model. The driver performs the small mapping when publication
occurs. This keeps the cross-cutting reporting facade independent from
driver-owned request parsing and artifact policy.

The initial `ReportPhase` inventory should cover:

1. provider normalization;
2. reachable module loading;
3. whole-program resolution;
4. type checking and HIR production;
5. preliminary MIR lowering;
6. preliminary MIR verification;
7. static-lifecycle planning;
8. planned MIR verification;
9. static-lifecycle synthesis;
10. the target-independent MIR pass pipeline;
11. backend assembly emission;
12. host linking; and
13. artifact publication.

Lexing, parsing, source reading, module discovery, and individual pass events
are nested detail owned by module loading, the singleton frontend, or a pass
pipeline. The first event model may represent these as distinct phase enum
variants or as typed detail events beneath an owning phase, but it must not use
unvalidated arbitrary phase names from CLI input.

## SR4 — CLI configuration and warning policy

Operational detail and diagnostic visibility should be configured
independently.

The proposed operational ladder is:

| CLI selection | Operational output |
|---|---|
| default or `--report-level off` | No report events are rendered. |
| `-v` or `--report-level phases` | Phase progress, final outcome, and artifact notices. |
| `-vv` or `--report-level details` | Phase and total timings plus aggregate metrics. |
| `-vvv` or `--report-level trace` | Per-module, verification, and per-pass detail. |

Repeated `-q` subtracts from repeated `-v` and saturates at `off`. An explicit
`--report-level` combined with `-v` or `-q` should be a usage error rather than
an order-dependent override. Detail resolution is pure CLI parsing and must
perform no I/O.

Diagnostic display should default to `warning`, meaning warnings and errors
are rendered. `--diagnostic-level error` hides warning rendering but never
removes warnings from `CompilationReport`. There is no `off` diagnostic level
because source errors must remain visible for a failed CLI compilation.

Skald should not expose Niflheim's combined `--log-level` semantics. Calling
phase progress `info` and source warnings `warning` suggests one severity
threshold even though they are different output contracts. Separate option
names make `-q` unable to hide a source warning accidentally.

No environment variable should silently change reporting in the first
implementation. A future configuration mechanism must define precedence and
test isolation explicitly.

## SR5 — Phase timing and failure semantics

Elapsed measurements use `std::time::Instant` and `Duration`. Wall-clock time,
timestamps, local time zones, process IDs, and thread IDs do not enter report
events. A phase wrapper records one start instant, emits `PhaseStarted` when
enabled, runs the phase once, and emits one `PhaseFinished` with `Completed` or
`Failed`.

A phase that produces source errors is failed for operational reporting even
when it returns a structured diagnostic collection rather than a Rust error.
No later phase start event is emitted after a terminal failure. A compiler
panic is not converted into a normal failed compilation by reporting; unwind
behavior and internal-defect policy remain unchanged.

The renderer may group related implementation steps for concise output, but
the event stream retains trust-boundary distinctions. In particular,
preliminary MIR verification, lifecycle planning, planned MIR verification,
synthesis, and final MIR pipeline verification must not become one opaque
“lowering” duration in programmatic observations.

Timings are observations, not correctness expectations. Tests must not assert
exact real durations, nonzero duration, relative phase order by duration, or a
performance threshold. Renderer tests construct events with fixed durations.
Performance benchmarks may consume real report events, but their acceptance
policy remains separate from compiler correctness.

The total compilation duration begins immediately before the first
compilation operation and ends after assembly production or the terminal
compilation failure. CLI end-to-end duration may additionally include linking
and artifact publication; the event names must distinguish compiler total from
driver total rather than silently changing the scope between assembly and
executable modes.

## SR6 — Statistics ownership and source-loading accounting

Metrics should be constructed from phase outputs or small phase-owned sidecars.
The driver may aggregate them but must not parse dumps, assembly comments, or
diagnostic wording.

The initial aggregate metric candidates are:

| Owner | Metrics |
|---|---|
| Module loading | reached modules, source files read, source bytes, discovery lex runs, discovery parse runs, final lex runs, final parse runs, tokens processed |
| Resolution | modules, classes, interfaces, functions, methods, generic templates, generated specializations |
| Type checking | checked definitions, produced HIR definitions, warning count, error count |
| MIR lowering and lifecycle | definitions, blocks, operations, planned lifecycle nodes, dependency edges, synthesized definitions |
| MIR pass pipeline | pass executions, verification executions, pass-owned transformation counters |
| Backend | emitted definitions, data objects, assembly bytes, assembly lines |
| Driver | link invocation count, artifact kind, published bytes when cheaply available |

This is an initial vocabulary, not a requirement to expose every count in the
first patch. A metric should be added only when its owner can define it
precisely and test it without coupling to unstable private representation.

Module loading requires explicit accounting. Today each staged source is
lexed and parsed once to discover reachable dependencies and again after the
complete staged set is placed in the final source database. Reporting may show
an aggregate `lex_runs=2N` and `parse_runs=2N`, or separate discovery and final
counters. It must not claim that N reached modules imply only N parser
executions. Per-module trace events should likewise say which loading stage
they describe.

When transformations are added to the MIR pipeline, a pass should return its
program plus a private or facade-exposed statistics value. It should not call
a global logger or format its own sentence. The pipeline attaches pass
identity, duration, outcome, and counters to report events. A pass with no
useful counters still receives completion timing at trace detail.

Metrics that require a full additional traversal of a large IR are guarded by
`observer.enabled(ReportDetail::Details)` or `Trace`. Counters already produced
as part of ordinary work may be retained cheaply even when not rendered.

## SR7 — Rendering, streams, and output stability

The first implementation should provide one deterministic human-text renderer.
Operational output goes to stderr. Help and version continue to use stdout,
source diagnostics continue to use stderr, and compiler artifacts continue to
use their selected paths. Reporting must never be written into generated
assembly or a linked program's stdout.

Text conventions should be modest and stable within tests:

- every operational line begins with `skac:`;
- a short category such as `phase`, `stats`, or `artifact` follows the prefix;
- durations render in a consistent adaptive or fixed millisecond form;
- metrics use deterministic owner-defined order;
- paths retain the driver's existing native-path display policy; and
- a trailing newline is emitted exactly once per rendered record.

Structured source diagnostics retain their current `error[CODE]` and
`warning[CODE]` headers rather than receiving a second `skac:` prefix.
Reporting a failed phase immediately before a diagnostic must not restate the
diagnostic message.

The default observer is silent, preserving existing CLI, golden, and tooling
bytes. Tests that request reporting opt into nondeterministic timing fields and
must use structural or reviewed partial matching rather than exact live
durations.

A JSON renderer is deliberately deferred. The typed internal event model
keeps it possible, but a machine format requires separate decisions about
schema versioning, path representation, duration units, partial output on
failure, diagnostic inclusion, and compatibility. The initial proposal does
not add `serde` to `skald-compiler` or promise stable external telemetry.

## SR8 — Module and public API organization

Reporting is a cross-cutting compiler service and should not live inside
`driver`, because module loading and future pass pipelines must publish events
without depending backward on process orchestration. The proposed structure
is:

```text
crates/skald-compiler/src/reporting/
├── mod.rs
├── event.rs
├── metrics.rs
├── text.rs
└── tests.rs

crates/skald-compiler/src/driver/cli/
├── parse.rs
└── reporting.rs
```

`reporting/mod.rs` is the facade. It owns module documentation, private module
declarations, explicit re-exports, the small observer trait, and the no-op
observer. `event.rs` owns event, phase, outcome, and detail identities.
`metrics.rs` owns metric names, values, units, and deterministic construction.
`text.rs` owns human rendering and a writer-backed observer if that
responsibility becomes substantial. `tests.rs` owns model, filtering, and
rendering tests.

`driver/cli/reporting.rs` resolves parsed CLI settings into the text observer,
owns stderr integration and deferred writer-error extraction, and keeps
process exit behavior out of the general reporting facade. Small option fields
remain in the existing typed `CompileOptions` rather than creating a second
command model.

Submodules remain private and re-exports selective. The crate is
repository-internal, but existing compilation paths should remain available so
workspace tools do not need to construct an observer. Compiler phases accept
only the narrow observer or phase-local measurement context they require; they
do not learn about CLI flags, text formatting, or stderr.

No dependency on `log`, `env_logger`, or `tracing` is required for the first
implementation. A later adapter may forward `ReportEvent` values to an
external framework without making framework macros the compiler's internal
contract.

## SR9 — Verification, determinism, and overhead

Verification should be divided by owner:

- reporting unit tests construct exact events and verify filtering, metric
  units, deterministic ordering, text formatting, and deferred writer errors;
- CLI parser tests cover repeated `-v`, repeated `-q`, saturation, explicit
  level conflicts, diagnostic-level validation, help, and non-UTF-8 path
  preservation;
- driver pipeline tests use `RecordingReportObserver` to verify phase order,
  failure cutoff, outcome, metric presence, request and singleton coverage,
  and no duplicated diagnostics;
- module-loader tests verify discovery and final parser execution accounting;
- pass tests verify that transformation counters match the changed program
  and that disabled detail avoids optional scans;
- CLI stream tests verify that operational text and diagnostics use stderr,
  help/version retain stdout, and default successful output stays empty;
- real-binary tests cover representative `-v`, `-vv`, `-q`, and warning-policy
  invocations without asserting live duration values; and
- the ordinary golden suite proves that quiet default compiler observations
  and generated artifacts remain unchanged.

The recording observer stores owned typed events. Tests assert enum values,
event sequence, integer counters, and terminal outcome. They do not search
human sentences for semantic facts.

The disabled path should perform no string formatting, path rendering, metric
sorting, heap allocation solely for events, or extra IR traversal. Reading a
small number of `Instant` values in the observed adapter is acceptable, but
the quiet wrapper may use the no-op observer's `enabled` result to avoid even
phase-start event construction. A focused benchmark or allocation-count test
may document overhead; a timing threshold does not belong in `make check`.

Repository validation for implementation should include focused reporting,
driver, module, and real-binary tests followed by `make check`. Changes to
manifests, public repository-internal APIs, or supported Rust syntax also
require `make msrv-check`.

## SR10 — Promotion and delivery boundary

SR1 through SR10 were reviewed and confirmed together on 2026-08-29 because
CLI semantics, event representation, phase ownership, error behavior, and test
strategy depend on one another. The promotion procedure then:

1. published a focused living compiler reporting contract at
   `docs/compiler/REPORTING.md`;
2. updated the compiler architecture, driver contract, debugging guide, and
   testing guide to link to that single authority;
3. moved this proposal to `docs/archive/` as the preserved decision record;
4. created and indexed a PR-sized implementation roadmap; and
5. kept source-language documentation and the runtime ABI unchanged because
   structured compiler reporting adds no source syntax or runtime service.

The implementation roadmap settles contracts before consumers in this
dependency order:

1. event, metric, observer, renderer, and CLI-detail contracts;
2. request and singleton phase observation with aggregate loading statistics;
3. driver linking, publication, warning-display policy, and real-binary
   behavior; and
4. pass-owned detailed metrics, hardening, documentation promotion, and full
   validation.

The roadmap splits these boundaries further to keep each task reviewable and
does not permit temporary global logging as an intermediate state.

## Rejected alternatives

### Use only `log` or `tracing` macros throughout compiler phases

A generic framework can route messages and spans, but direct macro use would
make framework field conventions the compiler contract, encourage global
configuration, and leave structured diagnostics and phase-owned metric
identity unresolved. An adapter remains possible after Skald has its own
event boundary.

### Add logging methods to `Diagnostics`

`Diagnostics` is durable source-facing compiler output. Progress and timings
have no source span, are nondeterministic, and should not be retained or
rendered as warnings. Combining them would weaken diagnostics for IDE and test
consumers.

### Put timings and counters only in `CompilationReport`

A final sidecar cannot report live progress, linking, publication, or partial
work before every error category. It also makes every caller retain
nondeterministic measurements. A recording observer can build a sidecar when a
caller wants one without changing the durable report.

### Report only from the CLI

CLI wrappers can time broad phases but cannot honestly account for module
discovery, repeated parsing, verifier boundaries, pass-owned changes, or the
in-memory compilation adapter. Library callers would also remain blind.

### Emit preformatted strings from each phase

Strings are convenient initially but cannot be reliably filtered, aggregated,
tested structurally, pluralized by another renderer, or serialized later.
They also tempt tests to parse user-facing wording to recover numeric facts.

### Include dumps as trace-level messages

IR dumps are large deterministic products with their own formats and output
destinations. Placing them in report events would create stream mixing,
unbounded event payloads, and unclear machine-format behavior.

### Collect every possible statistic unconditionally

Many useful counts are cheap because a phase already computes them; others
require an extra traversal or retained private state. The observer's detail
query preserves a quiet low-overhead path and lets metrics be added only when
their meaning is stable.

## Deferred extensions

This proposal deliberately leaves the following for later focused decisions:

- a versioned JSON, SARIF, OpenTelemetry, or other external schema;
- environment-variable or configuration-file precedence;
- color, terminal detection, progress bars, or in-place terminal updates;
- timestamps, process metadata, thread metadata, or distributed tracing;
- concurrent phase event ordering and synchronized observers;
- stable public API guarantees outside this repository;
- warning groups, warning promotion to errors, per-code suppression, or source
  attributes controlling warnings;
- compiler caches and incremental-compilation hit/miss reporting;
- runtime program logging or runtime panic-trace configuration; and
- generalized dump selection, naming, or retention policy.

Each can consume the typed observer boundary without changing source
diagnostics, phase ownership, or the quiet default established here.
