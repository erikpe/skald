# Structured Compiler Reporting

Status: authoritative for the implemented structured-reporting contract. The
typed event and metric model, request-local observers, compiler and driver
instrumentation, human renderer, CLI selection, and report-writer failure
handling described here are current. Historical design alternatives are
preserved in the archived
[design proposal](../archive/STRUCTURED_REPORTING_DESIGN_PROPOSAL.md), and the
completed [implementation roadmap](../archive/STRUCTURED_REPORTING_ROADMAP.md)
preserves delivery and validation history.

Structured reporting makes one compiler invocation observable without making
observation part of compilation semantics. It covers phase progress, elapsed
time, aggregate and pass-owned statistics, and artifact publication. Source
diagnostics, driver errors, deterministic phase dumps, generated artifacts,
and runtime program output retain their existing owners.

The design is request-scoped and event-first. Repository tools can compile
through an explicitly supplied observer, collect owned typed events without
global state, and render them independently. The CLI renders selected
observations to stderr through the same request-local observer.

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

A report event is a small typed operational observation. Event families are
phase start, phase completion, trace-level module parsing, artifact
publication, and scoped run completion. Events may carry duration, outcome,
and deterministically ordered metric values. They do not retain source text or
large phase products.

### Dumps

Token, AST, module-graph, resolved, HIR, MIR, analysis, and assembly dumps
remain phase-owned formats selected through explicit APIs or future dump
options. The implemented artifact-report vocabulary contains only assembly
and executable publication because the CLI publishes no dump artifact. Future
dump publication must extend that vocabulary explicitly; dump contents must
not become trace-level messages.

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

The driver preserves its quiet compilation functions and provides observed
counterparts:

```rust
pub fn compile_request_to_assembly(
    request: &CompilationRequest,
) -> Result<AssemblyArtifact, CompilationError>;

pub fn compile_request_to_assembly_observed(
    request: &CompilationRequest,
    observer: &mut dyn ReportObserver,
) -> Result<AssemblyArtifact, CompilationError>;
```

The quiet request function delegates through a no-op observer. The in-memory
`compile_source_to_assembly` adapter has the same quiet and observed surfaces,
with the observer as the final argument, so its lexing and parsing work is
visible without filesystem module loading.

The observer is an invocation service, not semantic request configuration. It
does not live inside `CompilationRequest`, participate in `Clone` or `Eq`,
affect deterministic request identity, or enter a phase product. Passing it
explicitly permits separate callers and future concurrent outer tooling to use
independent sinks.

`observe` is infallible from the compiler's perspective. The implemented
writer-backed text observer remembers its first write failure, stops attempting
output, disables every later detail query, and exposes the retained error
through `error` and `into_parts`. The process driver extracts that error after
the command operation, returns status 74, and does not add presentation
failures to `CompilationError` or cancel a successfully produced artifact.

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
    ModuleParsed {
        module: String,
        stage: ReportModuleStage,
        tokens: u64,
        outcome: ReportOutcome,
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
Rust names may evolve with reviewed compiler changes, but these properties do
not change implicitly:

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

Request compilation emits these boundaries in order:

1. provider normalization;
2. reachable module loading;
3. whole-program resolution;
4. type checking and HIR production;
5. preliminary MIR lowering;
6. preliminary MIR verification;
7. static-lifecycle planning;
8. planned MIR verification;
9. static-lifecycle synthesis;
10. the target-independent MIR pass pipeline; and
11. backend assembly emission.

Singleton compilation emits lexing and parsing before the same resolution
through backend sequence. Its `Lexing` and `Parsing` identities are explicit
because no module loader owns that work. Executable driver composition emits
host linking, while assembly and executable composition both emit artifact
publication.

Within request compilation, source reading, discovery lexing/parsing, and final
lexing/parsing remain beneath module loading. Their aggregate work is attached
to the module-loading finish event at details level. Trace additionally emits
one `ModuleParsed` event for each parser execution, with a typed `Discovery` or
`Final` stage and canonical logical module path. It includes neither source
contents nor physical filesystem metadata.

Elapsed measurements use `std::time::Instant` and `Duration`. Wall-clock
timestamps, time zones, process IDs, and thread IDs do not enter the event
model. Each normally returning started phase produces exactly one completed or
failed finish event. Details and trace observers receive that owner's stable
metrics; phases-only observers receive an empty metric vector. A source phase
that returns terminal diagnostics is failed for reporting purposes even though
the diagnostics remain ordinary data. No later phase starts after terminal
failure. The final compilation-scoped `RunFinished` ends after assembly
production or the terminal compiler failure and excludes host linking and
publication.

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

The implemented metrics appear in this deterministic owner order:

| Phase | Successful finish metrics |
|---|---|
| Singleton lexing | `lex executions`, `source bytes`, `tokens`, `diagnostics`, `warnings`, `errors` |
| Singleton parsing | `parse executions`, `tokens`, `diagnostics`, `warnings`, `errors` |
| Module loading | `reached modules`, `source reads`, `source bytes`, discovery lex/parse executions and tokens, then final lex/parse executions and tokens |
| Resolution | modules, function declarations/definitions, class declarations/definitions, interface declarations, diagnostics, warnings, errors |
| Type checking | produced HIR modules and function/class definitions, then diagnostics, warnings, errors; failed checking has only diagnostic metrics because it produces no HIR |
| Preliminary MIR lowering | definitions, blocks, instructions |
| MIR verification boundaries | verification executions; failed verification also includes verification errors |
| Static-lifecycle planning | effect summaries, dependencies; declared, active, inactive, active-explicit, active-zero-default, and inactive-explicit static fields; activation execution nodes, edges, and conservative targets; activation fields, shutdown fields, and retained preliminary static initializers |
| Static-lifecycle synthesis | final MIR definitions, blocks, instructions, followed by lifecycle definitions and activation/shutdown regions when present |
| MIR pass pipeline | verification executions; normalization execution and deterministic structural counts; pass executions; when passes run, processed and changed callables, structural rewrite counts, and pass-owned counters grouped in first-owner/first-counter order; then successful final MIR definitions, blocks, instructions |
| Backend emission | assembly bytes, assembly lines |

A failed loader retains completed loader counters before diagnostic counts. A
failed lifecycle plan retains the dependency and diagnostic counts it actually
produced. Phases that fail before producing a count do not invent one.

Module loading accounts for actual work. `source reads` counts attempted source
reads; `reached modules` counts successfully decoded and staged logical
modules; `source bytes` counts their UTF-8 bytes. The loader lexes each staged
source once during dependency discovery and again after the final source
database is assembled. It invokes the parser after each successful lex, even
when parsing then produces diagnostics. Discovery and final counters are
therefore separate. Token totals count the actual token vectors, including the
required EOF sentinel, and never imply that N reached modules means only N
parser runs.

Metrics requiring diagnostic or MIR traversal, assembly line scanning,
allocation, or formatting are constructed only after a `Details` or `Trace`
query succeeds. Trace module records are collected only for `Trace`. Quiet and
phases-only compilation performs no report-only product scan, sort, or text
formatting; already-known phase execution counts remain local to the observed
adapter.

The `none` MIR schedule performs proof verification, one mandatory
normalization with normalized verification, and zero pass executions.
The `default` schedule executes ten pass occurrences in the exact optimization
order documented by the compiler phase contract; an
unchanged result retains the input seal, while a changed result performs one
additional immediate verification. The runner owns verified execution, atomic
changed-result commit, immediate reverification, aggregate accounting, and
optional per-occurrence timing. Passes return outcomes and already-known
integer data; they do not format sentences or call a global logger. Aggregate
metrics distinguish callables processed by a pass from callables it actually
changed.

## Final-MIR pass reporting

The confirmed
[selectable final-MIR pipeline design](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_DESIGN_PROPOSAL.md)
extends this reporting boundary with structured pass-occurrence observation.
Its
[completed implementation roadmap](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
records delivery. Registry, request/CLI schedule selection, the verified
runner, and structured pass reporting are implemented. Ordinary production
traces contain one pass-finished event for each of the nine default schedule
occurrences; `none` contains none.

Every attempted selected occurrence produces one pipeline-owned record in
schedule order. Its stable identity consists of schedule position, typed pass
identity, stable name and stage, and that pass's zero-based occurrence number. The
record carries elapsed duration, `unchanged`, `changed`, or `failed` outcome,
processed and changed callable counts when pass data exists, structural
commit counts, verification executions caused by the occurrence, and
deterministically ordered pass-owned integer measurements. A failure before a
pass outcome carries no invented pass data. The structured pipeline error
remains authoritative, and no successful later occurrence or phase product is
reported.

The MIR-pipeline finish event retains aggregate owner order: verification
executions, pass executions, processed and changed callables plus structural
rewrite counts when passes ran, pass-owned aggregate counters in first pass
owner and first counter order, then successful final MIR definitions, blocks,
and instructions. Text rendering qualifies pass-owned counters with their
stable pass name. The first canary owns
removed assignment, removed value-declaration, and changed-callable counts.
Trace reporting additionally emits one typed pass-finished event per attempted
occurrence, including a failed outcome for the occurrence attributed by the
pipeline error; details retain the deterministic aggregate metrics without
parsing those events or a MIR dump. Timings remain
nondeterministic observations, so correctness and determinism tests assert
identity, order, outcome, and integer measurements rather than live duration
values.

Pass timers and the occurrence vector are enabled only after a trace-detail
query succeeds. Off, phases-only, and details-only observation therefore runs
the aggregate coordinator without occurrence timing or report-record
allocation. Phase metric and event construction remains guarded by the
existing detail queries.

Pass modules return data and never call observers, loggers, formatters, or
filesystem services. The pipeline coordinator converts outcomes into report
data, and reporting owns rendering. Stage-aware MIR checkpoints use the
separate request-local `MirPipelineInspector` service. Its closed borrowed view
contains either proof-rich or normalized final MIR; only the final variant can
render seal-bound reachability. Labels are `proof-rich-input`,
`after-proof-rich-<position>-<name>-<occurrence>`,
`after-proof-normalization`, `after-final-<position>-<name>-<occurrence>`, and
`final`. Changed results are resealed before callbacks; failures produce no
checkpoint for an unpublished product and no product-final checkpoint.
Checkpoint labels and bytes are deterministic, but dump contents do not become
report events, metrics, semantic request identity, or pass logs. The disabled
ordinary path performs no dump rendering, collection, or event construction.
Filesystem publication and general CLI dump retention remain separate driver
decisions.

## Local final-MIR simplification observation

The implemented observation surface follows the
[frozen local-simplification design](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_DESIGN_PROPOSAL.md)
and [completed roadmap](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md).
It extends the existing occurrence, aggregate, and checkpoint model. Primitive
constant folding, primitive algebraic simplification, and conservative CFG
cleanup use that model under exact compiler-internal schedules and the active
repeated default profile.

Primitive constant folding reports processed/changed callables and folded
unary, binary, comparison, and cast assignments. Primitive algebraic
simplification reports constant-result rewrites, forwarded uses, removed
assignments and value declarations, protected-use rejections, and changed
callables. Conservative CFG cleanup reports constant and same-target branch
folds, removed blocks and value declarations, protected unreachable blocks,
and changed callables.

These pass-owned counters explain transformation reasons. Existing commit
statistics remain authoritative for total inserted, retained, and removed
entities and are not reconstructed by passes. Counter owner and first-counter
order remain deterministic across repeated occurrences. Timing remains an
observation excluded from determinism assertions.

The default profile produces one trace occurrence per schedule entry,
including repeated constant-folding and dead-pure occurrences with distinct
zero-based occurrence numbers. Stable-name exclusion removes all occurrences
of that pass. Verified checkpoint labels retain the existing
`after-<schedule-position>-<stable-name>-<occurrence-number>` form, so repeated
passes cannot collide.

Passes continue to return data without logging or formatting. The pipeline
owns timers, occurrence records, verification counts, and aggregate
conversion; reporting owns rendering; inspectors own optional verified MIR
and reachability dumps. Quiet, phases-only, and details-only paths retain their
current allocation and timing boundaries.

Productive coverage pins more than zero-counter plumbing. A compact final-MIR
fixture requires non-zero primitive-folding, algebraic,
CFG-cleanup, and whole-world-retention counters together with exact final
definition and block counts. Its independent-process fingerprint includes
every occurrence number, outcome, structural count, verification count,
pass-owned measurement, and final MIR dump while excluding elapsed durations.
A standard-library-backed golden supplies larger before/after structural
observations, but no correctness test imposes a wall-clock or performance
threshold.

### Checked-integer constant protocol simplification observation

The registered `checked-integer-constant-folding` pass reports folded
quotient, remainder, and shift protocol counts, removed private protocol-load
value counts, and retained statically failing candidate counts in that stable
order. Generic retained, inserted, and removed MIR entity totals remain owned
by the atomic commit statistics. An unchanged occurrence reports all
processed executable callables and performs no changed-output verification;
an exact-schedule or default-profile rewrite reports changed callables and one
immediate verification. Its default occurrence follows the second primitive
constant fold and precedes dead-pure and CFG cleanup, so aggregate metrics and
trace occurrence records retain pipeline order without changing quiet,
phases-only, or details-only timing-allocation boundaries.

## Frozen proof-normalization observation

Status: **in progress**. The frozen
[design](../roadmaps/PROOF_PROVENANCE_NORMALIZATION_DESIGN_PROPOSAL.md) and
[roadmap](../roadmaps/PROOF_PROVENANCE_NORMALIZATION_ROADMAP.md) extend final-
MIR observation with an explicit proof-rich-to-final boundary. The two-seal
transition and its observation are active: aggregate counts include one
complete proof verification, one normalization execution, and one normalized
final verification even for the `none` profile.

Normalization is a mandatory phase transition rather than a registered pass.
It therefore produces no pass occurrence, selectable identity, exclusion, or
pass-owned duration contract. Details reports publish deterministic aggregate
counts for consumed path-condition records, consumed logical-expression
records, lowered path reads, reclassified activation storage, changed
callables, and blocks released from proof protection.

Inspection exposes a closed stage-typed borrowed view. Proof-rich input and
after-pass checkpoints carry `VerifiedProofMirProgram`; the named
`after-proof-normalization` checkpoint, final-stage after-pass checkpoints,
and product-final checkpoint carry `VerifiedFinalMirProgram`. Labels retain
schedule position, stable pass name, and occurrence identity where a
selectable pass exists. Final checkpoints alone expose final seal-bound
reachability facts.

A normalization failure has its own pipeline failure stage and prevents every
final-stage occurrence, backend phase, and final checkpoint. The
`post-proof-unreachable-block-elimination` canary is an ordinary selectable
final-stage pass with processed/changed callable counts and deterministic
removed-block, removed-value-declaration, and permanent-root-retention
measurements. It runs immediately after mandatory normalization in the
default profile. `whole-world-reachability` remains an ordinary
final-stage occurrence with its existing pass-owned metrics and stage-bearing
trace record.

Quiet compilation performs no checkpoint formatting, dump rendering, trace
record allocation, or optional count scan. Durations remain nondeterministic
observations; normalization structure, stage order, checkpoint labels, and
integer counts remain deterministic products. Reporting does not affect proof
verification, normalization, pass selection, MIR, backend input, diagnostics,
or runtime behavior.

## Whole-world reachability observation

The confirmed
[whole-world reachability design](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_DESIGN_PROPOSAL.md)
uses the existing occurrence, aggregate, inspection, and rendering ownership.
Reachability analysis and pass implementations do not log or call observers.

The registered `whole-world-reachability` pass reports deterministic,
already-known integer counters for examined, reachable, and removed executable
definitions, split into function, static-initializer, and member categories.
It also reports roots, reachable execution nodes and callables, dependency
edges, runtime-entity targets, virtual/interface selection, and function-value
target counts. Reporting keeps
declaration count, physically present definition count, and reachable callable
count distinct. The pipeline owns elapsed time and occurrence conversion;
reports own rendering; live durations remain excluded from deterministic
assertions.

The focused reachability graph dump remains separate from report events and
ordinary MIR checkpoint bytes. A checkpoint's `reachability_dump` method lets
compiler tools and tests inspect roots,
typed edges, possible targets, witnesses, and summary counts through borrowed
verified facts. The ordinary driver supplies no such inspector and performs no
graph rendering, label construction, or related allocation. Analysis or
retention failure uses the existing pass-attributed structured error and emits
no invented successful occurrence or later phase product.

Central final verification now derives and seal-binds those facts. Its existing
verification-execution counter counts that complete boundary once; unchanged
pass outcomes do not recompute it, and changed outcomes count the immediate
reverification that rebuilds program and facts. Reachability adds no separate
analysis event, timing, cache statistic, or preservation record.

## Frozen static activation observation

Status: **implemented**. Exact activation is a mandatory semantic phase rather
than a pass occurrence. Its immutable planning product exposes deterministic,
already-known integer counts for declared, active, inactive, active-explicit,
active-zero-default, and inactive-explicit statics plus activation execution
nodes, edges, and conservative targets. The driver adapts those values to the
existing static-lifecycle-planning finish event only at `details` or `trace`;
activation analysis itself does not observe, log, time, render, or write.

Canonical triggers, witness paths, active/inactive field inventories, and
activation/reverse-shutdown order belong to a focused deterministic activation
dump available through a `StaticActivationInspector` placed in the driver-owned
`CompilationInspectors` service. The same service may independently carry a
`MirPipelineInspector`; it is supplied to
`compile_request_to_assembly_observed_inspected` or
`compile_source_to_assembly_observed_inspected`. The activation inspector
receives one borrowed `verified-static-activation` checkpoint after
planned-MIR verification. Merely enabling the inspector does not render the
dump; the callback must request `activation_dump`. These details do not become
report event text, MIR checkpoint bytes, source diagnostics, request identity,
certificate identity, generated artifacts, or quiet-default work. There is no
per-inactive-static warning: ordinary pay-for-use library declarations are not
operational warnings, and the frozen contract deliberately adds no eager
annotation that could silence one.

## CLI selection

Operational detail and diagnostic visibility use separate controls.

| Selection | Operational output |
|---|---|
| default or `--report-level off` | No report events are rendered. |
| `-v` or `--report-level phases` | Phase progress, final outcome, and artifact notices. |
| `-vv` or `--report-level details` | Phase and total timings plus aggregate metrics. |
| `-vvv` or `--report-level trace` | Details plus discovery and final module-parser records. |

Repeated `-q` subtracts from repeated `-v` and saturates at `off`. Combining an
explicit `--report-level` with `-v` or `-q` is a usage error rather than an
order-dependent override. Shorthand letters may be repeated as separate
arguments or one cluster. Detail caps at `trace`; extra quiet flags remain at
`off`. Option resolution is pure and performs no I/O.

Diagnostic display defaults to `warning`, which renders warnings and errors.
`--diagnostic-level error` hides warnings at the CLI boundary but retains them
inside `CompilationReport`. There is no diagnostic `off` level because a
failed CLI compilation must display its source errors.

Skald does not combine operational detail with diagnostic severity under a
single `--log-level`. Quiet operational reporting cannot accidentally hide a
source warning. The implementation has no environment variable or
configuration file that silently changes report selection.

The CLI constructs one `TextObserver` over its borrowed stderr for the whole
compile command. Compilation emits its own total. Executable mode then emits a
host-linking phase, and both output modes emit an artifact-publication phase,
an artifact notice only after the final atomic rename, and a distinct driver
total. A terminal compiler, linker, or publication failure finishes the
applicable phase and totals before the existing diagnostic or driver error is
rendered once.

## Human rendering and streams

`render_event` and `TextObserver` produce deterministic human text. Every
operational line begins with `skac:` and a short category such as `phase`,
`stats`, or `artifact`. Phase detail omits durations and metrics; details and
trace render elapsed time in milliseconds with three fractional digits,
rounded to the nearest microsecond. Metrics retain owner order and bytes use
singular or plural units. Paths use Rust's native lossy display, and every
rendered record owns exactly one trailing newline. Off detail renders no text.

The renderer accepts an arbitrary writer and does not itself select a process
stream. CLI operational text goes to stderr. Help and version stay on stdout.
Source diagnostics stay on stderr with their existing `error[CODE]` and
`warning[CODE]` headers rather than gaining a second `skac:` prefix. Compiler
artifacts remain at selected paths, and generated-program stdout is unrelated
to compiler reporting.

The quiet observer preserves every existing successful CLI, golden, and
tooling byte by default. Tests that request live timings use structural or
reviewed partial matching rather than byte-exact duration strings.

A versioned JSON or other machine schema is deferred. The typed event model is
format-neutral, but external serialization requires separate decisions about
schema versioning, duration units, path representation, partial failure output,
diagnostic inclusion, and compatibility. The implemented contract adds no Serde
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
have cohesive private owners. Private driver CLI modules own typed option
resolution, stderr integration, linking/publication observation, diagnostic
filtering, and process-status handling.

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
  exposing private implementation modules; and
- driver reporting tests cover request and singleton order, every source
  cutoff, lifecycle failure, malformed MIR, backend failure, compilation
  totals, unchanged results, panic behavior, and independent parallel callers;
- CLI parser tests cover verbosity, quiet saturation, explicit-level conflicts,
  diagnostic levels, help, and pure option resolution;
- module-loader tests for discovery and final parser execution accounting;
- pass tests for transformation counters and disabled-detail optional work;
- binary integration tests for real `skac` arguments, stdout/stderr, status,
  native paths, report-writer failure, and assembly/executable artifacts;
- ordinary goldens continue to prove that default-off reporting preserves
  generated artifacts and process streams;
- default-versus-explicit-off binary comparisons cover success, source
  diagnostics, provider failure, toolchain failure, help, and version, while
  constructed owner tests pin warning and backend-failure bytes; and
- repeated and concurrent library/driver invocations prove that observers and
  destinations do not leak across requests.

The disabled path performs no monotonic timing, event construction, string
formatting, path cloning or rendering, metric sorting, heap allocation solely
for report events, or extra IR traversal. Compiler and driver run timers,
phase timers, owned artifact paths, detailed metrics, and trace records are
all guarded by the observer's detail query. Overhead may be recorded by a
focused measurement, but a timing threshold does not join `make check`.

Implementation validation uses focused reporting, driver, module, pass, and
binary tests followed by `make check`. Changes to supported Rust syntax or the
repository-internal public API also run `make msrv-check`.

## Deferred extensions

The implemented contract excludes:

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
preserves the reviewed alternatives and SR1 through SR10 decisions. The
archived [implementation roadmap](../archive/STRUCTURED_REPORTING_ROADMAP.md)
preserves task order, validation, and implementation history.
