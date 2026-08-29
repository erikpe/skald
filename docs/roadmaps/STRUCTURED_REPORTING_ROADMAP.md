# Structured Compiler Reporting Roadmap

Status: in progress; REP0 is complete and REP1 is next.

This roadmap implements the frozen
[structured compiler reporting contract](../compiler/REPORTING.md) and its
archived [design record](../archive/STRUCTURED_REPORTING_DESIGN_PROPOSAL.md).
It adds request-scoped typed observation to library compilation, then exposes
that observation through an opt-in human CLI without changing default
compiler output, source diagnostics, generated artifacts, or runtime behavior.

## Scope and invariants

- Add typed phase, outcome, metric, artifact, and run-completion events behind
  one repository-internal `reporting` facade.
- Pass one explicit object-safe observer through an observed compilation
  adapter; retain the current quiet request and singleton APIs through a no-op
  observer.
- Keep source diagnostics, driver errors, deterministic dumps, artifacts, and
  runtime program output separate from operational reports.
- Time provider normalization, loading, semantic phases, MIR trust boundaries,
  backend emission, linking, publication, and scoped totals with monotonic
  `Duration` values.
- Count actual source-discovery and final parsing work rather than inferring
  parser executions from reached-module count.
- Make metrics typed, unit-bearing, owner-defined, deterministic, and lazy when
  their construction requires optional work.
- Add the frozen `-v`/`-q`, `--report-level`, and `--diagnostic-level` CLI
  surface with operational output on stderr and no diagnostic-off mode.
- Preserve status 74 for report-writer failures without adding presentation
  errors to `CompilationError`.
- Preserve byte-for-byte quiet default behavior for existing CLI, golden, and
  repository-tool invocations.
- Keep compiler phases independent of CLI flags, stderr, text formatting, and
  driver request parsing.
- Keep reporting out of `CompilationRequest` equality, deterministic request
  identity, phase products, generated assembly, runtime ABI, and source
  semantics.
- Add no `log`, `env_logger`, `tracing`, Serde, JSON, terminal UI, environment
  configuration, warning-group, dump-selection, cache, or concurrency protocol
  during this roadmap.
- Keep `mod.rs` files as concise facades with private responsibility-oriented
  implementation modules and explicit minimal re-exports.
- Keep the root Makefile as the validation interface; add no repository CI.

## Progress

- [x] REP0 — Establish typed reporting and human rendering
- [ ] REP1 — Observe the complete compilation pipeline
- [ ] REP2 — Publish honest phase-owned statistics
- [ ] REP3 — Integrate CLI selection, linking, and artifact reporting
- [ ] REP4 — Harden composition, publish implementation, and close

## PR-sized implementation sequence

### REP0 — Establish typed reporting and human rendering

**Purpose:** Settle the format-neutral event, metric, observer, and renderer
boundary before compiler phases or CLI policy depend on it.

- [x] Add a top-level `reporting` facade with private `event`, `metrics`, and
      `text` implementation modules plus a substantial external `tests.rs`.
- [x] Define `ReportDetail`, `ReportPhase`, `ReportScope`, `ReportOutcome`,
      `ReportArtifactKind`, unit-bearing integer metric values, deterministic
      metrics, and owned `ReportEvent` variants for start, finish, publication,
      and scoped run completion.
- [x] Define the object-safe `ReportObserver` contract with an `enabled` query,
      and provide quiet no-op and owned recording observers through selective
      facade re-exports.
- [x] Implement deterministic human event rendering with the `skac:` category
      prefix, one duration policy, owner-order metrics, native path display,
      and exactly one trailing newline per event.
- [x] Implement a writer-backed text observer that records its first write
      error, suppresses later writes, and exposes the retained error without
      making `observe` fallible.
- [x] Keep event types independent of driver-owned `ArtifactKind`; map only
      through the reporting-owned artifact vocabulary.
- [x] Add compile-time public API coverage for intentional reporting facade
      paths without exposing private module layout.
- [x] Update the reporting contract and compiler facade inventory only for the
      model and renderer behavior implemented in this task; retain explicit
      wording that no compiler phase or CLI option emits events yet.

**Tests:** Exact event construction and equality; detail ordering and no-op
filtering; count and byte metric units; deterministic metric order; fixed
`Duration` rendering; every phase/scope/outcome/artifact category; Unicode and
non-UTF-8 Unix paths; short writes and first writer error retention; recording
observer ownership; public API compile coverage; no source-diagnostic or dump
type inside report events.

**Gates:** `cargo test --locked -p skald-compiler reporting`;
`cargo test --locked -p skald-compiler --test public_api`; `make fmt-check`;
`make lint`; `make docs-check`; `make msrv-check`; and `git diff --check`.

**Exit criteria:** Repository callers can construct, filter, record, and render
the complete frozen event vocabulary through `skald_compiler::reporting`, text
writer failure is observable without a fallible observer method, existing
compiler entry points remain quiet, and production dependencies are unchanged.

### REP1 — Observe the complete compilation pipeline

**Purpose:** Thread one explicit observer through the existing phase
orchestration and prove success/failure sequencing before statistics or CLI
presentation add further consumers.

- [ ] Add observed request and in-memory singleton compilation functions while
      preserving the current signatures as no-op-observer wrappers.
- [ ] Add one small generic phase-observation helper that reads monotonic time,
      emits start and exactly one completed/failed finish, and returns the
      original phase result without changing its type or error.
- [ ] Observe provider normalization, reachable loading, resolution, type
      checking, preliminary MIR lowering and verification, lifecycle planning,
      planned MIR verification, lifecycle synthesis, the MIR pass pipeline,
      and backend assembly emission at their actual orchestration boundaries.
- [ ] Emit a compilation-scope total ending after assembly production or the
      terminal compilation failure; do not include host linking or publication.
- [ ] Treat terminal source diagnostics as failed phase outcomes while leaving
      their structured values and rendering unchanged.
- [ ] Stop event production with the same phase cutoff as compilation: no later
      phase starts after provider, source, verification, or backend failure.
- [ ] Keep compiler panics as internal defects rather than converting unwind
      into an ordinary failed event/result pair.
- [ ] Preserve request artifacts, IR, assembly, diagnostics, errors, and public
      quiet-path behavior exactly when comparing observed and unobserved runs.
- [ ] Document the implemented observed library surface and phase inventory in
      the reporting, architecture, driver, debugging, and testing authorities.

**Tests:** Recording-observer order for request and singleton success; provider
failure; lex/parse diagnostic failure; resolution failure; type-check failure;
planned-lifecycle failure; malformed MIR fixture; backend failure; one finish
per start; no event after cutoff; compilation total scope; observed/unobserved
artifact and diagnostic equality; independent observers across repeated and
parallel outer callers; exact phase enum coverage.

**Gates:** Focused reporting and driver pipeline tests;
`cargo test --locked -p skald-compiler --test public_api`;
`cargo test --locked -p skald-compiler --test pipeline_determinism`;
`make compiler-test`; `make docs-check`; `make msrv-check`; and
`git diff --check`.

**Exit criteria:** Every frozen compiler phase and trust boundary emits a
deterministic typed start/finish sequence to an explicit observer, all terminal
outcomes stop at their current boundary, the two existing compilation APIs are
quiet compatibility wrappers, and enabling observation changes no durable
compiler result.

### REP2 — Publish honest phase-owned statistics

**Purpose:** Add useful counters at their semantic owners after event ordering
is stable, with special care for repeated module-loading work and disabled-path
overhead.

- [ ] Introduce a module-loading measurement sidecar or narrow observed loader
      adapter without changing the existing public `load_module_graph` result
      contract used by repository callers.
- [ ] Count reached modules, source reads, source bytes, discovery lex/parse
      executions, final lex/parse executions, and tokens processed; expose
      discovery and final work separately or through an explicitly totalled
      pair.
- [ ] Add stable cheap output metrics for resolution, typed HIR, preliminary
      and final MIR, lifecycle planning/synthesis, diagnostics, and backend
      assembly only where the owning product defines the count precisely.
- [ ] Add MIR pipeline statistics for verification and registered pass
      executions without inventing transformation counters while the pipeline
      has no transforming pass.
- [ ] Establish the return-shape rule that future transformations publish
      pass-owned statistics to the pipeline rather than logging formatted
      sentences internally.
- [ ] Emit unit-bearing metrics in one documented deterministic order for each
      phase and attach them only to the corresponding finish event.
- [ ] Guard extra IR traversals, sorting, allocations, and formatting behind
      the observer's `Details` or `Trace` query; retain already-computed cheap
      counters without changing phase products unnecessarily.
- [ ] Add trace-detail module observations that identify discovery versus final
      parsing without including source contents or nondeterministic filesystem
      metadata.
- [ ] Update reporting and module/debugging documentation with the implemented
      metric vocabulary and exact repeated-parse accounting.

**Tests:** Multi-module graphs with explicit, compiler-injected, duplicate, and
cyclic dependencies; malformed reached sources; singleton lex/parse counts;
exact discovery/final execution counts; UTF-8 source byte and token totals;
resolved/HIR/MIR definition and block counts; lifecycle and pass-pipeline
counts; assembly bytes/lines; diagnostic counts; deterministic owner order;
quiet/phases detail causing no optional scans; metrics on failed phases only
when the owner has completed the relevant work.

**Gates:** Focused reporting, module-graph, phase, MIR pipeline, and backend
tests; `cargo test --locked -p skald-compiler --test pipeline_determinism`;
`make compiler-test`; `make docs-check`; `make msrv-check`; and
`git diff --check`.

**Exit criteria:** Details-level events expose precise, deterministic, typed
statistics from every implemented owner selected for the initial vocabulary,
a reached graph reports its real discovery and final parser work, the quiet
path performs no report-only scans or formatting, and future pass statistics
have one explicit pipeline-owned route.

### REP3 — Integrate CLI selection, linking, and artifact reporting

**Purpose:** Make the typed library observations configurable and useful from
the real compiler process while preserving independent diagnostic policy and
all existing stream/status behavior.

- [ ] Extend typed CLI parsing with repeatable `-v` and `-q`, explicit
      `--report-level off|phases|details|trace`, and
      `--diagnostic-level warning|error`.
- [ ] Resolve `-v`/`-q` by saturating subtraction; reject either shorthand
      combined with explicit report level and reject repeated or invalid
      explicit options as command-usage errors.
- [ ] Update exact help text and typed `CompileOptions` without filesystem
      access or environment-dependent defaults.
- [ ] Construct the text observer over the CLI's borrowed stderr, invoke the
      observed compiler adapter, and render selected events without mixing
      operational text into stdout, diagnostics, assembly, or executable
      output.
- [ ] Filter warning rendering only at the CLI boundary; retain every warning
      in `CompilationReport`, always render source errors, and never make
      operational quietness suppress diagnostics.
- [ ] Observe host linking and assembly/executable publication with the same
      invocation observer, and add a driver-scope total distinct from the
      compiler-scope total.
- [ ] Emit successful artifact notices only after atomic publication; failed
      linking/publication gets a failed phase outcome followed by the existing
      driver error exactly once.
- [ ] Extract a retained report-writer error at the process boundary and return
      status 74 without replacing an earlier compiler result with a
      `CompilationError` variant.
- [ ] Add real-binary and driver tests for every detail/diagnostic combination,
      both entry forms, assembly/executable modes, failure categories, native
      path arguments, and exact stdout/stderr ownership.
- [ ] Update the driver contract, development workflow, debugging guide,
      testing guide, README command reference, and any golden-fixture guidance
      affected by the implemented CLI.

**Tests:** Pure option resolution for zero through excessive `-v`/`-q` counts;
all explicit values and conflicts; help/version; phase-only text; details and
trace structural text with unconstrained live durations; warning filtering
over a constructed report; errors at both diagnostic levels; provider,
backend, linker, and publication failures; report-writer failure; no duplicate
error wording; assembly/executable artifact notices after publication; separate
compiler/driver totals; default success with empty stderr; real `skac` stdout,
stderr, status, and output preservation.

**Gates:** Focused reporting and driver CLI tests; `make cli-test`;
`make golden-filter GOLDEN_FILTER='driver/**'` when matching fixtures exist;
`make compiler-test`; `make docs-check`; `make msrv-check`; and
`git diff --check`.

**Exit criteria:** The real `skac` process implements the frozen detail ladder
and diagnostic policy on stderr, reports compilation/link/publication through
one typed observer, preserves all quiet defaults and error categories, and
maps report-output failure to status 74 without changing compiler semantics.

### REP4 — Harden composition, publish implementation, and close

**Purpose:** Prove the reporting boundary under complete repository
composition, remove rollout-only wording, and leave current behavior in living
documentation before archiving the roadmap.

- [ ] Audit every `ReportPhase`, scope, outcome, artifact kind, metric unit, and
      CLI selection for focused owner coverage and remove unreachable or
      redundant event vocabulary.
- [ ] Verify independent observers across repeated library compilation,
      concurrent outer tooling calls, golden-runner process scheduling, and
      test execution without global state or cross-request leakage.
- [ ] Prove default byte stability for successful, warning-bearing, diagnostic
      failure, provider failure, backend failure, toolchain failure, help, and
      version invocations; keep live durations out of exact default goldens.
- [ ] Add structural real-process observations for phases, details, trace,
      warning filtering, artifacts, and failures without asserting exact real
      elapsed values.
- [ ] Audit the quiet path for report-only formatting, sorting, path rendering,
      allocation, or full-product traversal; record any optional focused
      overhead measurement without adding a timing gate to `make check`.
- [ ] Confirm production dependencies remain unchanged and no global logging,
      JSON schema, runtime surface, source syntax, or dump policy entered the
      implementation.
- [ ] Update all living reporting, architecture, driver, development, testing,
      debugging, and public API documentation to implemented present tense;
      remove REP task codes and rollout gates outside roadmap history.
- [ ] Run the complete ordinary and MSRV gates from an artifact-free snapshot,
      resolve high-priority responsibility hotspots, and record any lower-
      priority follow-up in an indexed discovery document rather than
      expanding this roadmap.
- [ ] Mark every task complete, set this roadmap status to complete, move it to
      `docs/archive/`, remove it from the active index, add it to the archive
      index, and repair all incoming links.

**Tests:** Full event/metric/CLI inventory audit; independent-request stress;
real-binary default byte comparisons; complete source-diagnostic and native
goldens; full compile and native determinism modes; public API paths; docs
links/indexes; repository status and diff hygiene.

**Gates:** Focused reporting suites; `make check`;
`make golden-determinism-test`; `make msrv-check`; `make docs-check`; and
`git diff --check` from an artifact-free snapshot or clean checkout.

**Exit criteria:** The frozen reporting contract is fully implemented and
documented, quiet defaults and durable compiler products remain unchanged,
request-local observations compose across all repository callers, no
high-priority reporting hotspot remains, and the completed roadmap is archived
with only current behavior in living documentation.

## Ordering and dependencies

REP0 fixes event identity, filtering, ownership, and text rendering before any
phase or CLI becomes a producer. REP1 then proves observer threading and
failure sequencing with duration-only phase events. REP2 builds statistics on
that stable route, especially the loader's repeated work, without forcing the
CLI and metrics changes into one review. REP3 exposes the completed library
surface through process streams and extends observation to linking and atomic
publication. REP4 is intentionally last because default compatibility,
cross-caller isolation, overhead, and documentation closure require every
producer and renderer to exist.

The roadmap depends only on the implemented request-based driver, explicit
phase pipeline, structured diagnostics, module loader, MIR verification/pass
boundary, backend registry, and atomic artifact publication. It has no
dependency on another active roadmap. REP0 must precede all later work; REP1
must precede REP2 and REP3; REP2 should precede REP3 so the CLI does not define
temporary metric wording. Focused documentation work travels with the task
whose behavior becomes current rather than being postponed wholesale to REP4.
