# Structured Compiler Reporting

Status: authoritative for the frozen structured-reporting contract. The typed
event and metric model, observer facade, recording and text observers, and
human renderer are implemented. Compiler phases and the command-line driver do
not emit report events yet; their delivery is tracked by the
[structured reporting roadmap](../roadmaps/STRUCTURED_REPORTING_ROADMAP.md).
Current command-line behavior remains defined by
[Driver and Artifacts](DRIVER_AND_ARTIFACTS.md) until the responsible roadmap
tasks update it.

Structured reporting makes one compiler invocation observable without making
observation part of compilation semantics. It covers phase progress, elapsed
time, aggregate and pass-owned statistics, and artifact publication. Source
diagnostics, driver errors, deterministic phase dumps, generated artifacts,
and runtime program output retain their existing owners.

The design is request-scoped and event-first. The implemented facade lets
repository tools construct, record, and render owned typed events without
global state. Planned compiler work will emit those events to an explicitly
supplied observer, and the CLI will render selected events as human text on
stderr.

## Contract boundary

Structured reporting has these invariants:

1. Enabling, disabling, or filtering reports cannot change phase order,
   diagnostics, IR, assembly, artifacts, exit meaning, or runtime behavior.
2. One invocation receives one observer explicitly; independent invocations do
   not share hidden reporting state.
3. Diagnostics remain structured source-facing values with severity, code,
   labels, and notes.
4. Driver failures retain their provider, verification, backend, toolchain,
   usage, publication, or command-output categories.
5. Dumps remain explicit deterministic inspection products rather than large
   report messages.
6. Phase and metric identity exists before text rendering.
7. Nondeterministic elapsed values never enter deterministic dumps,
   diagnostics, assembly, artifact identity, default golden observations, or
   future cache keys.
8. Ordinary successful compilation remains operationally silent by default.
9. Errors are never hidden by report filtering.
10. Optional metrics that require extra traversal or allocation are computed
    only when the observer requests their detail level.

This is a compiler-tooling contract. It adds no Skald source syntax, generated
program service, runtime ABI, panic-trace behavior, or language-visible warning
attribute.

## Diagnostics, errors, reports, and dumps

The four output families are deliberately separate.

### Diagnostics

A source warning or error remains a `Diagnostic` retained by
`CompilationReport` and rendered against its `SourceDatabase`. Reporting does
not change construction, codes, labels, notes, or ordering. CLI warning
visibility is a presentation policy over the report; it does not delete
warnings from the library result.

### Driver errors

Provider configuration, invalid MIR, backend rejection, toolchain failure,
command usage, and publication failure keep their existing typed errors and
process-status mapping. A report event may identify which phase failed and how
long it ran, but it never restates or replaces the error message. The CLI
renders the actual failure exactly once through its existing owner.

### Report events

A report event is a small typed operational observation. Initial event
families are phase start, phase completion, artifact publication, and scoped
run completion. Events may carry duration, outcome, and deterministically
ordered metric values. They do not retain source text or large phase products.

### Dumps

Token, AST, module-graph, resolved, HIR, MIR, analysis, and assembly dumps
remain phase-owned formats selected through explicit APIs or future dump
options. Publishing a dump may produce an artifact notice naming its path;
the dump contents never become a trace-level message.

## Request-scoped observer composition

The reporting facade exposes this object-safe observer:

```rust
pub trait ReportObserver {
    fn enabled(&self, detail: ReportDetail) -> bool;
    fn observe(&mut self, event: ReportEvent);
}
```

`NoopObserver` disables all detail and discards events. `RecordingObserver`
owns events in emission order at a selected detail level. `TextObserver<W>`
renders to any `std::io::Write`. Owners use `enabled` before computing optional
detailed statistics.

The planned driver integration preserves its current quiet compilation
functions and adds observed counterparts:

```rust
pub fn compile_request_to_assembly(
    request: &CompilationRequest,
) -> Result<AssemblyArtifact, CompilationError>;

pub fn compile_request_to_assembly_observed(
    request: &CompilationRequest,
    observer: &mut dyn ReportObserver,
) -> Result<AssemblyArtifact, CompilationError>;
```

Once pipeline integration lands, the quiet function will delegate through a
no-op observer. The in-memory `compile_source_to_assembly` adapter will receive
the same pair of quiet and observed surfaces so its lexing and parsing work is
visible to repository tools without filesystem module loading.

The observer is an invocation service, not semantic request configuration. It
does not live inside `CompilationRequest`, participate in `Clone` or `Eq`,
affect deterministic request identity, or enter a phase product. Passing it
explicitly permits separate callers and future concurrent outer tooling to use
independent sinks.

`observe` is infallible from the compiler's perspective. The implemented
writer-backed text observer remembers its first write failure, stops attempting
output, disables every later detail query, and exposes the retained error
through `error` and `into_parts`. The planned process-driver integration will
therefore retain status 74 without adding presentation failures to
`CompilationError` or changing compilation results.

The initial observer contract does not require `Send`, `Sync`, or concurrent
event ordering. A future internally parallel compiler must define
deterministic collection and replay or separately extend the observer
contract.

## Event and metric model

The implemented semantic shape is:

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
```

`ReportMetric::count` and `ReportMetric::bytes` construct named values, and
`ReportMetric::new` accepts an explicit `MetricValue`. These repository-internal
Rust names may evolve during the roadmap, but these properties may not change
implicitly:

- phases, scopes, artifact kinds, and outcomes are validated enums rather than
  arbitrary message strings;
- elapsed time is `Duration`, not a pre-rounded floating-point value;
- metrics retain their integer value and unit until rendering;
- one event owns the data needed by a recording observer;
- metrics use owner-defined deterministic order;
- formatting, alignment, pluralization, path display, and time rounding belong
  to renderers; and
- reporting owns its small artifact vocabulary instead of depending backward
  on driver request parsing or artifact policy.

The compilation and complete driver run are separate scopes. Compilation total
ends after assembly production or terminal compiler failure. Driver total may
add host linking and publication. Assembly and executable modes never silently
give one identically named total two different meanings.

## Phase inventory and outcomes

`ReportPhase` defines these boundaries before pipeline emission is added:

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

Source reading, discovery lexing/parsing, final lexing/parsing, individual
verification operations, and future named passes are detail events or owned
metrics beneath their corresponding phase. They remain distinguishable at
trace detail even when the human renderer groups adjacent successful phases
for concise output.

Elapsed measurements use `std::time::Instant` and `Duration`. Wall-clock
timestamps, time zones, process IDs, and thread IDs do not enter the event
model. Each started phase produces exactly one completed or failed finish
event. A source phase that returns terminal diagnostics is failed for reporting
purposes even though the diagnostics remain ordinary data. No later phase
starts after terminal failure.

Reporting does not turn a compiler panic into a normal compilation failure.
Unwind and internal-defect policy remain unchanged.

Timings are observations, not correctness results. Tests do not assert exact
live durations, nonzero duration, duration ordering, or performance thresholds.
Renderer tests use constructed fixed `Duration` values; benchmark tools may
consume real events under their own acceptance procedures.

## Metric ownership

The phase or pass that understands a count defines and populates it. The
driver may aggregate typed values but does not parse dumps, assembly comments,
or diagnostic wording to recover statistics.

The initial useful metric families are:

| Owner | Metric family |
|---|---|
| Module loading | reached modules, source files read, source bytes, discovery lex/parse runs, final lex/parse runs, tokens processed |
| Resolution | modules and stable declaration/specialization counts already represented by the resolved product |
| Type checking | checked definitions, produced HIR definitions, warning count, error count |
| MIR and lifecycle | definitions, blocks, operations, lifecycle plan nodes and edges, synthesized definitions |
| MIR pass pipeline | pass executions, verification executions, and future pass-owned transformation counters |
| Backend | emitted definitions or data when owned cheaply, assembly bytes, assembly lines |
| Driver | link invocation and published artifact kind/size when cheaply available |

This inventory defines ownership, not a requirement to expose every candidate
before its meaning is stable. A metric enters the initial implementation only
when its owner can state and test the count without exposing unstable private
representation.

Module loading must account for actual work. The current loader lexes and
parses each staged source once during dependency discovery and again after the
final source database is assembled. Reports therefore distinguish discovery
and final executions or clearly report their sum. They never imply that N
reached modules means only N parser runs.

Future transformation passes return their program plus pass-owned statistics.
They do not format sentences or call a global logger. The pipeline attaches
pass identity, duration, outcome, and counters to events. A pass without useful
counters may still emit completion timing at trace detail.

## CLI selection

Operational detail and diagnostic visibility use separate controls.

| Selection | Operational output |
|---|---|
| default or `--report-level off` | No report events are rendered. |
| `-v` or `--report-level phases` | Phase progress, final outcome, and artifact notices. |
| `-vv` or `--report-level details` | Phase and total timings plus aggregate metrics. |
| `-vvv` or `--report-level trace` | Per-module, verification, and per-pass detail. |

Repeated `-q` subtracts from repeated `-v` and saturates at `off`. Combining an
explicit `--report-level` with `-v` or `-q` is a usage error rather than an
order-dependent override. Option resolution is pure and performs no I/O.

Diagnostic display defaults to `warning`, which renders warnings and errors.
`--diagnostic-level error` hides warnings at the CLI boundary but retains them
inside `CompilationReport`. There is no diagnostic `off` level because a
failed CLI compilation must display its source errors.

Skald does not combine operational detail with diagnostic severity under a
single `--log-level`. Quiet operational reporting cannot accidentally hide a
source warning. The initial implementation has no environment variable or
configuration file that silently changes report selection.

## Human rendering and streams

`render_event` and `TextObserver` produce deterministic human text. Every
operational line begins with `skac:` and a short category such as `phase`,
`stats`, or `artifact`. Phase detail omits durations and metrics; details and
trace render elapsed time in milliseconds with three fractional digits,
rounded to the nearest microsecond. Metrics retain owner order and bytes use
singular or plural units. Paths use Rust's native lossy display, and every
rendered record owns exactly one trailing newline. Off detail renders no text.

The renderer accepts an arbitrary writer and does not select a process stream.
Planned CLI integration sends operational text to stderr. Help and version stay
on stdout. Source diagnostics stay on stderr with their existing `error[CODE]`
and `warning[CODE]` headers rather than gaining a second `skac:` prefix.
Compiler artifacts remain at selected paths, and generated-program stdout is
unrelated to compiler reporting.

The quiet observer preserves every existing successful CLI, golden, and
tooling byte by default. Tests that request live timings use structural or
reviewed partial matching rather than byte-exact duration strings.

A versioned JSON or other machine schema is deferred. The typed event model is
format-neutral, but external serialization requires separate decisions about
schema versioning, duration units, path representation, partial failure output,
diagnostic inclusion, and compatibility. The initial contract adds no Serde
dependency to `skald-compiler` and promises no external telemetry schema.

## Module ownership

Reporting is a cross-cutting compiler service rather than a driver-private
logger. Its implemented facade-oriented organization is:

```text
crates/skald-compiler/src/reporting/
├── mod.rs
├── event.rs
├── metrics.rs
├── text.rs
└── tests.rs
```

The top-level `reporting` facade owns module documentation, private submodule
declarations, explicit re-exports, the small observer trait, and no-op and
recording observers. Event identity, metric construction, and human rendering
have cohesive private owners. CLI configuration, stderr integration, and
process-status handling remain planned driver responsibilities.

Compiler phases depend only on the narrow reporting facade or a phase-local
measurement context. They do not learn about CLI flags, stderr, text
formatting, or driver request parsing. Existing quiet compilation paths remain
available to workspace callers.

No `log`, `env_logger`, or `tracing` dependency is required. A later adapter
may forward typed events to an external framework without making framework
macros the compiler's internal observation contract.

## Verification and overhead

Implemented tests follow existing ownership:

- reporting unit tests construct exact events and verify filtering, metric
  units, deterministic order, text rendering, and deferred writer errors;
- the public API integration test covers every intentional facade path without
  exposing private implementation modules.

Later roadmap tasks add:

- CLI parser tests for verbosity, quiet saturation, explicit-level conflicts,
  diagnostic level, help, and native path arguments;
- driver pipeline tests using a recording observer for phase order, failure
  cutoff, outcome, metrics, request compilation, and singleton compilation;
- module-loader tests for discovery and final parser execution accounting;
- pass tests for transformation counters and disabled-detail optional work;
- binary integration tests for real `skac` arguments, stdout/stderr, status,
  and artifact behavior; and
- ordinary goldens proving default observations and generated artifacts remain
  unchanged.

The disabled path performs no string formatting, path rendering, metric
sorting, heap allocation solely for report events, or extra IR traversal.
Reading a small number of monotonic instants in the observed adapter is
acceptable; the quiet wrapper may avoid event construction entirely through
the no-op observer's detail query. Overhead may be recorded by a focused
measurement, but a timing threshold does not join `make check`.

Implementation validation uses focused reporting, driver, module, pass, and
binary tests followed by `make check`. Changes to supported Rust syntax or the
repository-internal public API also run `make msrv-check`.

## Deferred extensions

The frozen initial contract excludes:

- versioned JSON, SARIF, OpenTelemetry, or another external schema;
- environment-variable or configuration-file precedence;
- color, terminal detection, progress bars, or in-place terminal updates;
- timestamps, process or thread metadata, and distributed tracing;
- synchronized observers or concurrent phase ordering;
- stable API guarantees outside this repository;
- warning groups, per-code suppression, warning promotion, or source warning
  attributes;
- cache and incremental-compilation hit/miss events;
- runtime program logging or runtime panic-trace configuration; and
- generalized dump selection, naming, or retention.

These extensions may consume the typed observer boundary but require focused
contracts of their own.

## Decision and delivery records

The archived [design proposal](../archive/STRUCTURED_REPORTING_DESIGN_PROPOSAL.md)
preserves the reviewed alternatives and SR1 through SR10 decisions. The active
[implementation roadmap](../roadmaps/STRUCTURED_REPORTING_ROADMAP.md) owns task
order, validation, and implementation status.
