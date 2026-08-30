# Selectable Final-MIR Optimization Pipeline Roadmap

Status: in progress. MPR0 through MPR2 are complete; MPR3 is next.

This roadmap implements the frozen
[selectable final-MIR optimization pipeline design](SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_DESIGN_PROPOSAL.md)
and its promoted
[compiler phase](../compiler/PHASES_AND_IR.md#frozen-selectable-final-mir-optimization-pipeline-direction),
[driver](../compiler/DRIVER_AND_ARTIFACTS.md#frozen-final-mir-optimization-selection),
and
[reporting](../compiler/REPORTING.md#frozen-final-mir-pass-reporting)
contracts. It turns the current verification-only final-MIR boundary into a
deterministic selectable production pass runner and finishes by enabling one
narrow dead-pure-definition elimination canary.

The primary result is durable optimization infrastructure rather than a broad
optimization suite. Because this work crosses phase ownership, request policy,
reporting, inspection, analysis, and dense rewriting, each task should also
remove small adjacent duplication, awkward ownership, and panic-prone internal
handling when the cleanup is cohesive. Larger findings belong in the
[dedicated discoveries record](SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_DISCOVERIES.md)
rather than expanding an active task.

## Dependencies

- The implemented
  [static-lifecycle certificate](../archive/STATIC_LIFECYCLE_CERTIFICATE_ROADMAP.md)
  provides immutable baseline authority and monotone realization after an
  effect-removing transformation.
- The implemented
  [dense callable-local MIR rewrite boundary](../archive/DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md)
  provides exhaustive identity traversal, stable sparse edits, deterministic
  dense commit, and verified seal invalidation and reconstruction.
- The current structured-reporting, request, CLI, MIR dump, verification, and
  backend-input contracts remain owners to extend; the optimizer must not
  create parallel substitutes.

## Scope and invariants

- Keep source acceptance, source diagnostics, evaluation order, checked
  failures, panic behavior, allocation behavior, ownership, aliases, mutable
  access through shared pointees, and deterministic destruction independent of
  optimization selection.
- Preserve permanent whole-world compilation and single-threaded generated
  program semantics without treating either as permission to weaken central
  MIR verification.
- Use one compiler-owned static registry of typed pass identities, unique
  stable kebab-case names, metadata, and transformation entry points; add no
  dynamic library, external pass ABI, or mutable global registry.
- Resolve typed `none` and `default` profiles into explicit immutable ordered
  schedules. Permit deliberate repetition and identify every occurrence by
  schedule position, pass identity, and per-pass occurrence number.
- Keep arbitrary exact schedules crate-private for tests and compiler tools.
  The supported CLI selects a profile and repeatable exclusions, not order.
- Verify synthesized final MIR before the first selected occurrence. Retain
  the existing seal after an unchanged result and immediately run ordinary and
  lifecycle-realization verification after every changed result.
- Give a pass only read-only verified MIR and one pipeline-owned atomic rewrite
  capability. Do not expose seal construction, raw dense table mutation,
  lifecycle-authority mutation, drivers, targets, reporters, or filesystems.
- Attribute input verification, pass execution, structural rewrite, and output
  verification failures to their exact owner and stop without publishing a
  partial or later product.
- Keep analyses pass-local initially. Every changed commit invalidates facts
  keyed by old local identities or instruction positions; add no preservation
  declarations or global analysis manager.
- Return ordered outcomes and already-known integer measurements as data.
  Distinguish processed from changed callables, and keep timings observational.
- Expose optional deterministic input, after-occurrence, and final checkpoints
  only over borrowed verified MIR. Keep MIR dump bytes outside report events.
- Implement the canary with an exhaustive conservative rvalue whitelist,
  exact value-use census, paired instruction/value-declaration removal, stable
  per-callable fixed-point waves, and one atomic callable commit.
- Keep calls, loads, callable addresses, checked operations, proof/path data,
  optional and array queries, ownership, lifecycle, storage, CFG, and every
  non-assignment result producer outside the canary.
- Do not add SSA, proof-provenance normalization, whole-program reachability,
  general alias/effect analysis, devirtualization, inlining, constant folding,
  CFG cleanup, register allocation, target LIR, target-specific passes, `-O`,
  or numeric optimization levels.
- Keep `mod.rs` files concise facades and place implementation-private helpers
  and tests with their responsibility owner.
- Keep the root Makefile as the automation interface; add no repository CI.

## Progress

- [x] MPR0 — Establish registry, profiles, and schedule resolution
- [x] MPR1 — Add typed request and CLI selection
- [x] MPR2 — Productionize the verified multi-pass runner
- [ ] MPR3 — Add structured pass measurements and reporting
- [ ] MPR4 — Add verified pipeline inspection checkpoints
- [ ] MPR5 — Publish an exhaustive value-use census
- [ ] MPR6 — Implement the dead-pure-definition canary
- [ ] MPR7 — Activate, harden, and close the canary pipeline

## PR-sized implementation sequence

### MPR0 — Establish registry, profiles, and schedule resolution

**Purpose:** Establish deterministic pass identity and policy independently of
execution, request parsing, or a production transformation.

- [x] Add a cohesive final-MIR pipeline policy owner behind the existing
      `passes` facade, with typed pass identity, stable name, description, and
      private transformation descriptor responsibilities kept distinct.
- [x] Define one immutable compiler-owned registry with deterministic lookup
      by identity and name; reject duplicate identities, duplicate names,
      invalid stable-name spelling, and descriptor/implementation mismatch in
      focused validation.
- [x] Define typed `none` and `default` profiles and resolve them to immutable
      ordered schedules without using registry iteration order.
- [x] Represent schedule position and zero-based per-pass occurrence number so
      deliberate repeated passes remain unambiguous.
- [x] Add deterministic exclusion resolution that removes every occurrence,
      treats duplicate exclusions idempotently, and reports unknown names with
      the known registry names sorted lexically.
- [x] Add a crate-private exact-schedule construction surface for isolated,
      repeated, and ordering tests without exposing arbitrary order through the
      driver API.
- [x] Keep the production registry and both profiles empty during this task;
      no callback runs and current compilation behavior remains unchanged.
- [x] Split identity, descriptor, profile, schedule, and error logic into small
      responsibility-oriented modules rather than growing the pass facade.

**Tests:** Registry uniqueness and stable-name validation; exact empty profile
expansion; synthetic schedule ordering and occurrence numbering; repeated and
duplicate exclusion behavior; lexical unknown-name diagnostics; registry
iteration independence; deterministic results from equivalent inputs.

**Gates:** `cargo test --locked -p skald-compiler passes`; `make fmt-check`;
`make lint`; `make docs-check`; and `git diff --check`.

**Exit criteria:** One typed deterministic policy layer can validate a static
registry and resolve profiles, exclusions, repetition, and exact internal
schedules, while the production pipeline still executes zero passes.

**Completed:** The pass facade now exposes a crate-private policy layer with a
typed identity, separately validated descriptor/implementation ownership, an
immutable static registry, typed empty `none` and `default` profiles,
deterministic exclusions, exact internal schedules, and positional occurrence
numbering. Registry validation rejects duplicate identities/names, malformed
stable names, empty descriptions, and identity mismatches in deterministic
order. Ten focused policy tests cover the frozen matrix, while the existing
empty-pipeline tests prove production still performs one verification and zero
passes. `cargo test --locked -p skald-compiler passes`, `make fmt-check`,
`make lint`, `make docs-check`, `make msrv-check`, and `git diff --check` passed
on 2026-08-30.

### MPR1 — Add typed request and CLI selection

**Purpose:** Make optimization selection explicit request policy with one
deterministic supported command-line surface.

- [x] Add a typed final-MIR optimization options value to
      `CompilationRequest` through a non-breaking builder and include it in
      request clone/equality behavior.
- [x] Make existing request construction and singleton helpers select
      `default`; document that its schedule remains temporarily empty until the
      canary activation task.
- [x] Thread resolved options through both quiet and observed compilation
      adapters to the pass pipeline without coupling them to target, runtime
      trace, report detail, or artifact policy.
- [x] Parse `--mir-optimization <none|default>` exactly once and repeatable
      `--disable-mir-pass <name>` with idempotent duplicate disabling.
- [x] Render deterministic usage failures for missing, repeated, invalid, and
      unknown values, with known pass names sorted lexically.
- [x] Update `skac --help` and driver-focused documentation without adding
      `-O`, numeric levels, arbitrary pass ordering, environment variables, or
      configuration files.
- [x] Ensure invalid selection performs no provider or source I/O and that
      every existing CLI/request construction path retains its prior behavior
      under the temporarily empty `default` schedule.
- [x] Consolidate adjacent option parsing or request-default duplication when
      it is small and directly reduces the new policy's maintenance cost.

**Tests:** Request default, builder, clone, and equality; quiet/observed and
singleton/request option threading; CLI help and valid forms; repeated profile,
missing value, unknown value, duplicate exclusion, and option-order cases;
usage exit status and no-I/O behavior; current assembly and diagnostic parity.

**Gates:** `cargo test --locked -p skald-compiler driver`;
`cargo test --locked -p skald-compiler passes`; `make cli-test`;
`make fmt-check`; `make lint`; `make docs-check`; and `git diff --check`.

**Exit criteria:** Every compiler entry point selects a typed profile and
exclusions deterministically, the CLI exposes only the frozen two options, and
the still-empty production schedules preserve exact current compilation.

**Completed:** `CompilationRequest` now owns typed, cloneable, comparable
final-MIR options with a non-breaking builder, a `default` profile default,
and canonical sorted/deduplicated exclusions. Quiet, observed, singleton, and
request compilation paths resolve and thread one immutable schedule into the
pipeline independently of target, reporting, runtime-trace, and artifact
policy. `skac` exposes only `--mir-optimization <none|default>` and repeatable
`--disable-mir-pass <name>`; deterministic configuration errors are reported
as usage failures before provider or source I/O. Focused request, driver,
public-API, pass-pipeline, and real-binary tests cover defaults, builder
identity, option order, invalid forms, no-I/O failure, zero pass execution, and
exact assembly/process-output parity while both production schedules remain
empty. `cargo test --locked -p skald-compiler driver`,
`cargo test --locked -p skald-compiler passes`, `make cli-test`,
`make fmt-check`, `make lint`, `make docs-check`, `make msrv-check`, and
`git diff --check` passed on 2026-08-30.

### MPR2 — Productionize the verified multi-pass runner

**Purpose:** Turn the tested rewrite coordinator into the sole production
owner of verified multi-pass execution and failure attribution.

- [x] Define a private pass capability that borrows the current verified
      product for analysis and can consume it only through the atomic
      whole-program rewrite coordinator.
- [x] Define explicit unchanged and changed outcomes. Preserve the same seal
      after unchanged; carry raw dense MIR, commit maps, change summaries,
      changed-callable accounting, and pass data after changed.
- [x] Verify synthesized MIR before invoking any selected callback, and ensure
      invalid input reports no pass execution.
- [x] Immediately call central ordinary and lifecycle-realization verification
      after every changed occurrence before another pass, checkpoint, or
      backend may receive the product.
- [x] Introduce structured pipeline errors for input verification, pass
      execution, structural rewrite, and changed-output verification, including
      exact schedule position, pass identity/name, and occurrence number where
      applicable.
- [x] Stop atomically at the first failure without exposing the consumed seal,
      malformed dense MIR, sparse editor state, partial program, or later
      callback result.
- [x] Route the ordinary production entry point and both compiler adapters
      through this runner while retaining the current one-verification,
      zero-pass path for empty schedules.
- [x] Remove or absorb the test-only transforming coordinator so there is one
      authoritative seal invalidation, rewrite, and resealing path.
- [x] Keep pass callbacks free of public raw-table mutation, lifecycle
      authority, seal construction, reporting, target, source-diagnostic, and
      filesystem capabilities.

**Tests:** Invalid input blocks callbacks; empty schedule parity; unchanged
seal and verification count; changed immediate resealing; two-pass ordering;
changed-then-unchanged and repeated schedules; each structured failure class;
no later callback/checkpoint/backend after failure; functions, members, and
static initializers; compile-fail visibility checks for seal and capability
leakage.

**Gates:** `cargo test --locked -p skald-compiler passes`;
`cargo test --locked -p skald-compiler mir::rewrite`;
`cargo test --locked -p skald-compiler mir::verify`; `make compiler-test`;
`make fmt-check`; `make lint`; `make docs-check`; and `git diff --check`.

**Exit criteria:** One production runner accepts only resolved schedules and
verified/rewrite capabilities, re-verifies every changed occurrence, retains
unchanged seals, attributes failures exactly, and never publishes raw or
partial MIR.

**Completed:** One production runner now consumes every resolved schedule. Its
private pass capability exposes borrowed verified MIR and can invalidate the
seal only through the atomic whole-program rewrite coordinator. Explicit
unchanged outcomes retain the original seal, while changed outcomes carry the
dense program, callable commit maps and change summaries, changed-callable
pass data, and immediately re-enter ordinary plus lifecycle-realization
verification. `MirPipelineError` distinguishes input verification, pass
execution, structural rewrite, and output verification; pass-attributed
failures retain stable name, internal identity, schedule position, occurrence
number, and their structured source. Execution stops at the first failure and
publishes no raw, partial, or later product. The former synthetic coordinator
was absorbed, and public compile-fail coverage proves neither the seal nor pass
capability can be forged externally. Focused exact-schedule tests cover empty,
unchanged, changed-then-unchanged, ordered/repeated, every failure class,
failure cut-off, truthful accounting, immutable lifecycle authority, backend
handoff, and function/member/static-initializer rewriting.
`cargo test --locked -p skald-compiler passes`,
`cargo test --locked -p skald-compiler mir::rewrite`,
`cargo test --locked -p skald-compiler mir::verify`, `make compiler-test`,
`make cli-test`, `make fmt-check`, `make lint`, `make docs-check`,
`make msrv-check`, and `git diff --check` passed on 2026-08-30.

### MPR3 — Add structured pass measurements and reporting

**Purpose:** Make optimization work observable as typed deterministic data
without coupling pass implementations to logging or presentation.

- [ ] Define an ordered pass-occurrence record containing stable occurrence
      identity, elapsed duration, unchanged/changed outcome, and pass-owned
      integer measurements.
- [ ] Clarify pipeline vocabulary and counters so processed callables and
      callables actually changed cannot be confused with successful commits.
- [ ] Aggregate verification executions, pass executions, pass-owned counters,
      and final MIR sizes in deterministic owner order on the MIR-pipeline
      phase finish event.
- [ ] Extend trace reporting with one typed pass-finished event for every
      attempted occurrence in schedule order, including a failed outcome.
- [ ] Preserve error ownership: the structured pipeline error remains
      authoritative for a failed occurrence, and reporting does not fabricate
      a successful outcome, later occurrence, or unavailable measurement.
- [ ] Keep pass modules limited to returning already-known counts; the runner
      owns timing and event conversion, and reporting owns rendering.
- [ ] Avoid report-only traversal, allocation, or formatting when the observer
      does not request the relevant detail.
- [ ] Keep elapsed values out of deterministic products and tests while making
      identity, order, outcome, and integer counts directly assertable.

**Tests:** Exact occurrence order and identity including repetition; unchanged
and changed results; processed versus changed callable counts; deterministic
metric owner order and aggregation; trace event rendering; details/trace/off
observer behavior; failure cut-off; no dump text or pass logging; fixed-duration
renderer fixtures rather than live timing assertions.

**Gates:** `cargo test --locked -p skald-compiler reporting`;
`cargo test --locked -p skald-compiler passes`; `make compiler-test`;
`make cli-test`; `make fmt-check`; `make lint`; `make docs-check`; and
`git diff --check`.

**Exit criteria:** The runner emits typed occurrence data and deterministic
aggregates with clear processed/changed vocabulary, reporting renders it at
the intended levels, and observation cannot affect pipeline products.

**Completed:**

### MPR4 — Add verified pipeline inspection checkpoints

**Purpose:** Provide deterministic optimized-MIR inspection without exposing
malformed intermediate state or turning dumps into report messages.

- [ ] Define a request-local optional inspection service separate from
      `CompilationRequest` and `ReportObserver`.
- [ ] Expose borrowed verified products at `input`, after every successfully
      completed occurrence, and `final`, including unchanged occurrences when
      requested.
- [ ] Define deterministic labels containing schedule position, stable pass
      name, and occurrence number so repetitions cannot collide.
- [ ] Ensure changed raw MIR is resealed before an after-pass callback and no
      callback runs after pass, rewrite, or verification failure.
- [ ] Permit the service to invoke the existing phase-owned `mir::dump_mir`
      renderer or collect in-memory statistics without granting mutation.
- [ ] Make the disabled path avoid checkpoint labels, dump rendering,
      allocation, and report events.
- [ ] Add a narrow measured/inspected pipeline composition surface for tests
      and compiler tools; defer general filesystem publication, retention, and
      CLI dump policy.
- [ ] Verify independent-process stability for checkpoint order, labels, and
      bytes under identical target-independent inputs.

**Tests:** Empty, one-pass, repeated, unchanged, changed, and failed schedules;
input/after/final order; repeated labels; verified-only callback type; disabled
zero-work path; no report event contamination; no-op exact dump parity;
independent-process checkpoint determinism.

**Gates:** `cargo test --locked -p skald-compiler passes`;
`cargo test --locked -p skald-compiler reporting`; `make compiler-test`;
`make fmt-check`; `make lint`; `make docs-check`; and `git diff --check`.

**Exit criteria:** Optional observers can inspect every frozen schedule
boundary deterministically, never see sparse or unverified MIR, and remain
independent from semantic request identity and operational reporting.

**Completed:**

### MPR5 — Publish an exhaustive value-use census

**Purpose:** Reuse the single MIR identity traversal to provide the canary and
later scalar passes with exact definition/use data without introducing a
general analysis manager.

- [ ] Add a narrow read-only value census behind the MIR rewrite or analysis
      facade, derived from the existing exhaustive callable-local identity
      mapper rather than a second hand-maintained MIR walk.
- [ ] Distinguish value declarations, definition sites, and actual uses;
      declarations and their defining result position must not count as uses.
- [ ] Count uses in instructions, rvalues, calls, places, projections,
      terminators, path conditions, logical records, proof metadata, callable
      attachments, and every other value-bearing site owned by the mapper.
- [ ] Return deterministic value-indexed counts and enough definition-site
      information for paired assignment/declaration deletion without exposing
      mutable MIR.
- [ ] Reject malformed foreign, unknown, or duplicate definition identities
      through existing structured traversal/error vocabulary rather than
      panicking or guessing.
- [ ] Document analysis lifetime: a rewrite invalidates the census and the
      canary recomputes it for every fixed-point wave.
- [ ] Consolidate adjacent value-use scanning only when the new census can
      become its complete authoritative replacement.
- [ ] Keep the API deliberately smaller than liveness, effects, aliasing,
      dominance, or an analysis cache.

**Tests:** One declaration and one definition with zero uses; each reference
site family; multiple uses; values used only by another dead definition;
path/logical/proof uses; calls and terminators; every executable definition
kind; malformed identities; deterministic indexing; exhaustive coverage that
forces review for new value-bearing MIR fields and variants.

**Gates:** `cargo test --locked -p skald-compiler mir::rewrite`;
`cargo test --locked -p skald-compiler mir::verify`;
`cargo test --locked -p skald-compiler passes`; `make compiler-test`;
`make fmt-check`; `make lint`; `make docs-check`; and `git diff --check`.

**Exit criteria:** One exhaustive read-only census reports actual uses and
definition sites for every callable value, shares the authoritative traversal,
and has an explicit post-rewrite invalidation rule.

**Completed:**

### MPR6 — Implement the dead-pure-definition canary

**Purpose:** Prove real modular final-MIR transformation through the registry,
analysis, atomic rewrite, measurement, and verification boundaries without
silently defining a general effect system.

- [ ] Add one responsibility-oriented pass module named
      `dead-pure-definition-elimination` and register it as the first
      production pass identity without placing it in `default` yet.
- [ ] Match every `MirRvalueKind` explicitly and whitelist only scalar
      constants, exact unary operations, exact binary operations, primitive
      comparisons, and non-checked primitive casts.
- [ ] Explicitly reject callable addresses, path conditions, loads, integer
      division, shifts, checked binary64-to-integer conversion, type tests,
      optional-presence operations, array length, and all future unreviewed
      rvalue families.
- [ ] Treat every non-`Assign` instruction as ineligible, including calls,
      I/O, allocation, initialization, copying, stores, cleanup, ownership,
      checked views, and array operations even when it defines a result.
- [ ] For each executable callable package, compute the value-use census,
      select unused eligible assignments in stable block/instruction order,
      functionally remove instructions and matching value declarations, and
      repeat by deterministic waves to a fixed point.
- [ ] Commit each changed callable once through the whole-program atomic
      coordinator; make no CFG, storage, path, logical, guard, lifecycle,
      ownership, metadata, folding, replacement, or instruction-order edit.
- [ ] Return exact removed-assignment, removed-value-declaration,
      processed-callable, and changed-callable measurements without logging or
      rendering.
- [ ] Exercise the pass through crate-private exact schedules and central
      changed-result verification; keep supported `default` behavior unchanged
      until final activation.

**Tests:** Used and unused examples for every eligible family; explicit test
for every excluded rvalue and non-assignment result producer; cascading trees
requiring multiple waves; mixed live/dead siblings and stable retained order;
paired declaration deletion and dense remapping; no-op equality; exact metrics;
function, instance/static method, initializer, copy constructor, copy
assignment, finalizer, and static initializer coverage; logical, path,
checked-operation, function-value, optional, array, shared, I/O, and lifecycle
fixtures; verification after every changed exact-schedule run.

**Gates:** `cargo test --locked -p skald-compiler passes`;
`cargo test --locked -p skald-compiler mir::rewrite`;
`cargo test --locked -p skald-compiler mir::verify`; `make compiler-test`;
`make fmt-check`; `make lint`; `make docs-check`; and `git diff --check`.

**Exit criteria:** The registered canary reaches its maximal conservative
fixed point through the supported census and atomic rewrite path, reports exact
work, re-verifies every change, and remains inactive in the supported default
profile pending broad parity validation.

**Completed:**

### MPR7 — Activate, harden, and close the canary pipeline

**Purpose:** Make the canary the supported default production optimization,
prove selection and unoptimized parity across the repository, and close the
roadmap without carrying transitional paths or documentation.

- [ ] Place `dead-pure-definition-elimination` exactly once in `default`; keep
      `none` empty, and prove disabling the canary from `default` resolves to
      the same schedule and product as `none`.
- [ ] Remove temporary empty-default allowances, inactive-registration
      scaffolding, duplicate coordinators, and compatibility helpers that are
      no longer needed after production activation.
- [ ] Prove `none` retains exact unoptimized MIR and assembly behavior apart
      from intended reporting/checkpoint selection, with one central final
      verification and zero pass executions.
- [ ] Prove default, none, disabled, duplicate-disabled, and crate-private
      repeated schedules across request, singleton, quiet, observed, CLI,
      backend, and native execution paths.
- [ ] Compare optimized and unoptimized source diagnostics, checked failures,
      panic behavior, output, ownership/lifecycle observations, and native
      behavior on representative and full-corpus programs.
- [ ] Add independent-process determinism coverage for schedule resolution,
      pass errors, measurements, checkpoint labels and bytes, MIR dumps,
      assembly, and golden products where applicable.
- [ ] Audit public and crate-private visibility, module facades, error wording,
      exhaustive matches, request help, living contracts, and tests; remove
      stale claims that the production pipeline is empty.
- [ ] Review the dedicated discoveries record. Implement small cohesive
      maintainability findings, retain larger actionable follow-ups, and do not
      schedule whole-world reachability or other optimization passes here.
- [ ] Run the complete repository, golden determinism, and MSRV gates and
      record exact completion evidence before marking every roadmap checkbox.

**Tests:** Full canary focused suite; request/CLI selection matrix; exact
none/default/disabled MIR and assembly comparisons; full compiler and CLI
suites; native observation parity; golden fixtures and determinism; reporting
and checkpoint order; failure injection; all executable kinds and broad MIR
feature corpus.

**Gates:** `make check`; `make golden-determinism-test`; `make msrv-check`;
`make docs-check`; `git diff --check`; and a final clean-artifact/status review.

**Exit criteria:** `default` runs the canary exactly once, `none` and selective
disabling prove the verification-only baseline, all changed products are
centrally verified, selection/measurement/checkpoint behavior is deterministic,
all quality gates pass, living documentation describes implemented behavior,
and any remaining discoveries are bounded follow-up work rather than missing
pipeline scope.

**Completed:**

## Roadmap completion criteria

This roadmap is complete only when all of the following are true:

- every progress and task checkbox is marked complete with concise evidence;
- the static registry, profiles, request/CLI policy, verified runner,
  structured failures, measurements, reporting, and checkpoints use one
  production path;
- `default` contains the canary exactly once and `none` preserves exact
  unoptimized behavior;
- the canary's exhaustive whitelist and fixed point cover every executable
  definition kind without widening language semantics;
- the final compiler, driver, reporting, and roadmap documentation contains no
  stale transitional or future-tense statements about implemented behavior;
- the discoveries record contains only actionable follow-ups with evidence,
  owner, priority, and bounded next step, or is removed if empty; and
- the complete repository, determinism, MSRV, documentation, formatting, lint,
  and diff gates pass.

After completion, move the frozen design proposal and this roadmap to
`docs/archive/`, update archive and active indexes, and leave the promoted
living compiler contracts as the authoritative description of current
behavior.
