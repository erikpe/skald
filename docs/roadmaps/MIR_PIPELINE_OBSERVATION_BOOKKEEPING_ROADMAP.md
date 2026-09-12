# MIR Pipeline Observation Bookkeeping Roadmap

Status: planned; O01 is next.

This roadmap implements
[cleanup finding A20](CODEBASE_CLEANUP_AUDIT.md#a20--factor-pipeline-observation-bookkeeping)
by giving pass-attempt timing and occurrence-record publication one private
owner while preserving the MIR runner's stage-specific semantic authority.
The resulting runner should make verification, proof-snapshot invalidation,
proof normalization, final resealing, and checkpoint publication easy to
follow without duplicating observation mechanics around every outcome.

The current ownership split is sound. `execution::runner` controls verified
state and stage order, `execution::measurement` defines the typed occurrence
product, `execution::statistics` accumulates deterministic totals, and
`execution::inspection` publishes optional borrowed checkpoints. This work
clarifies how those owners cooperate; it does not merge them into a general
event framework.

## Scope and invariants

- Centralize optional pass timing and construction of completed and failed
  `MirPassOccurrenceRecord` values behind one private execution helper.
- Keep record collection disabled below trace detail. Disabled collection must
  allocate no occurrence vector and start no per-pass timer.
- Preserve the distinction between pass execution or rewrite failure, where
  pass data is unavailable, and changed-output verification failure, where
  the attempted pass data, rewrite summary, and verification execution remain
  observable.
- Keep deterministic aggregate statistics independent of trace occurrence
  collection. Details reporting must not require occurrence records.
- Keep proof-rich, proof-transition, mandatory normalization, and final-stage
  execution as explicit typed paths. Their capabilities, outcomes, errors,
  seals, and verification functions must remain distinct.
- Preserve complete proof-snapshot analysis invalidation before changed-output
  verification and at both proof-normalization routes. Final occurrences must
  never report proof-snapshot analysis usage.
- Preserve schedule position, stable pass identity and name, stage, occurrence
  number, outcome, pass-owned measurements, rewrite totals, verification
  counts, analysis usage, and record order.
- Preserve checkpoint order and publication rules: input first, an after-pass
  checkpoint only after a verified product exists, the mandatory normalization
  checkpoint exactly once, and no later or final checkpoint after failure.
- Preserve pass selection, pass order, diagnostics, MIR dumps, final MIR,
  assembly, artifacts, native behavior, and public Rust paths.
- Keep durations operational and nondeterministic. They must not enter
  deterministic fingerprints, cache keys, diagnostics, dumps, or artifacts.
- Do not introduce a generic stage state machine, type-erased pass result,
  observer calls from pass implementations, selectable normalization event,
  concurrency, async execution, a new report schema, or a CLI option.
- Record any broader reporting or pipeline redesign discovered during
  implementation in an indexed discoveries document rather than expanding
  this roadmap.

## Progress

- [ ] O01 — Centralize occurrence recording
- [ ] O02 — Expose stage-specific execution boundaries and close A20

## PR-sized implementation sequence

### O01 — Centralize occurrence recording

**Purpose:** remove repeated timer and occurrence-vector mechanics without
changing the runner's semantic control flow.

- [ ] Add one private, cohesively named execution module for occurrence
  recording. Keep `execution::mod.rs` as the facade and expose nothing outside
  the existing execution boundary.
- [ ] Give the recorder ownership of whether trace collection is enabled, the
  optional start time for one attempted occurrence, and ordered insertion into
  the occurrence vector. Allocate capacity only when collection is enabled.
- [ ] Provide narrow completed and failed recording operations over the
  existing typed `MirPassOccurrenceRecord` constructors. Callers continue to
  supply the semantic outcome, pass data, rewrite summary, verification count,
  and already-accounted analysis usage.
- [ ] Migrate proof-rich, selected-transition, and final-stage record creation
  to the recorder. Remove the repeated `record_occurrences.then(Instant::now)`,
  conditional pushes, and standalone failure-record helper from the runner.
- [ ] Keep `execution::measurement` responsible for the immutable record model
  and `execution::statistics` responsible for aggregate counts. Do not move
  verification, snapshot reset, resealing, failure classification, or
  checkpoint callbacks into the recorder.
- [ ] Add or refine focused tests only where existing coverage does not pin the
  recorder boundary. Require empty occurrence output when disabled; exact
  schedule order and identity when enabled; unavailable data for execution and
  rewrite failures; retained pass data for output-verification failure; and
  unchanged analysis usage and verification counts.

**Tests:** Run the execution-module unit tests, pipeline occurrence and
analysis-usage tests, trace text-rendering tests, and driver reporting tests.
Compare deterministic occurrence fingerprints before and after the refactor,
excluding elapsed durations. Run `cargo fmt --all -- --check`,
`cargo clippy --locked --workspace --all-targets -- -D warnings`, `make check`,
`make msrv-check`, and `git diff --check`.

**Exit criteria:** all three selectable MIR stages publish records through one
private recorder; the runner contains no direct timer start, conditional
occurrence push, or duplicate failed-record construction; aggregate-only and
trace-enabled execution retain their exact observable contracts.

### O02 — Expose stage-specific execution boundaries and close A20

**Purpose:** make semantic stage ownership readable after observation mechanics
have a stable private interface, while removing the remaining duplicated
accounting and ordinary pass-failure plumbing.

- [ ] Organize runner execution into explicit proof-rich, optional transition,
  and final-stage functions or cohesive private sections with typed inputs and
  outputs. The top-level orchestration must visibly remain input verification,
  proof-rich execution, proof normalization, final execution, and final
  checkpoint publication in that order.
- [ ] Centralize the small common operation that maps ordinary
  `MirPassFailure::Execution` and `MirPassFailure::Rewrite` values to the exact
  pass-attributed `MirPipelineError`. Keep transition-boundary failures and
  changed-output verification failures in their stage-specific owners.
- [ ] Centralize per-occurrence aggregate updates only where their semantics
  are identical. Keep rewrite-summary extraction, proof-snapshot reset,
  proof verification, normalized resealing, normalization counters, and
  definition-retention handling explicit in the responsible stage path.
- [ ] Preserve the sequencing around changed results: collect/reset analysis
  state where applicable, record pass data and rewrite totals, count
  verification, verify or reseal, publish the record with the correct outcome,
  then publish an after-pass checkpoint only for success.
- [ ] Preserve selected-transition and implicit-normalization differences. The
  implicit boundary creates no pass occurrence, while a selected transition
  retains its pass identity and owns only failures and data attributable to
  that occurrence.
- [ ] Audit the resulting files by responsibility. Keep private helper types
  with their owner, avoid tiny pass-through modules, retain explicit facade
  re-exports, and remove superseded helpers and imports.
- [ ] Update the living MIR phase and reporting documentation only if the
  refactor reveals a missing current contract; do not turn implementation
  structure into a public API promise.
- [ ] Mark A20 complete with the delivered boundary and validation, mark this
  roadmap complete, archive it, update both roadmap indexes, and repair every
  incoming link. Put any excluded larger opportunity in an indexed discoveries
  document.

**Tests:** Run the complete pipeline tests covering changed, unchanged,
execution-failed, rewrite-failed, normalization-failed, and
output-verification-failed paths in all applicable stages. Run pipeline
composition, source-equivalence, independent-process determinism, inspection,
reporting detail-level, and observer-independent artifact tests. From a clean
checkout or artifact-free snapshot, run `make check`, `make msrv-check`, the
documentation checker, `cargo fmt --all -- --check`, and `git diff --check`.

**Exit criteria:** the top-level runner reads as explicit stage orchestration;
one private owner handles occurrence timing and publication; common accounting
and ordinary failure adaptation are not duplicated; stage authority and every
observable compiler result remain unchanged; A20 is complete and the roadmap
is archived with no untracked follow-up hidden in its scope.

## Ordering and dependencies

O01 comes first because timing and record publication are stage-neutral only
after their callers have supplied typed semantic data. Extracting that narrow
boundary exposes the true remaining duplication without first abstracting
verification or ownership transfer. O02 may then clarify the three stage paths
using the recorder's stable interface and can reject any helper that would
erase their different capabilities or failure rules.

The completed structural CFG and snapshot-analysis work supplies the current
verified-product, invalidation, and reporting contracts. This roadmap depends
on those contracts and must preserve them. It is independent of frontend
object-view planning, copy-capability reconstruction, shared MIR traversal
organization, and future optimization work.
