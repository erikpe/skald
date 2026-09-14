# Copy-Capability Materialization Roadmap

Status: planned; CP01 is next.

This roadmap implements
[cleanup finding A17](CODEBASE_CLEANUP_AUDIT.md#a17--reduce-copy-capability-fixed-point-reconstruction)
through the accepted
[copy-capability materialization design](COPY_CAPABILITY_MATERIALIZATION_DESIGN_PROPOSAL.md).
It measures the current type-check reconstruction first, establishes one
request-local neutral availability authority, and replaces the duplicate HIR
availability solver with one-pass concrete plan materialization only when the
recorded evidence supports that migration.

The roadmap has two valid endpoints. A successful experiment leaves the
phase-neutral service responsible for availability and failure paths while
type checking constructs each concrete HIR plan once. An unsuccessful
experiment removes migration-only machinery, retains independently useful
measurements and regressions, and closes A17 with the rejected reason. Reaching
the implementation checkpoint does not itself justify retaining the change.

## Scope and invariants

- Compute lifecycle availability from the final selected `ResolvedProgram`
  through one immutable `ResolvedLifecycleCapabilities` result per type-check
  request.
- Keep resolution able to run the neutral service over candidate publication
  views without depending on HIR or type checking.
- Keep concrete class copy operations, base and field order, final-field
  assignment permissions, array element operations, and HIR types in type
  checking.
- Materialize constructors before assignments and preserve the constructor
  prerequisite for optional class assignment.
- Build arrays once in canonical identity order after class plans are
  complete, retaining separate default, copy, assignment, and destruction
  semantics.
- Move the final HIR array table into `HirProgram` after checking rather than
  cloning it for publication.
- Preserve exact availability, outer-to-inner failure paths, diagnostics,
  resolved/HIR/MIR dumps, selected operation identities, assembly, and native
  behavior.
- Treat a neutral-available operation that cannot be materialized as a
  compiler consistency defect. Do not convert it into a source diagnostic or
  silently mark it unavailable.
- Do not cache lifecycle facts in `ResolvedProgram`, carry candidate facts
  across publication, introduce shared mutable state, or reuse results across
  compilation requests.
- Do not add a general query framework, dependency graph, worklist,
  incremental invalidation, arbitrary iteration cap, or phase-neutral HIR
  plan representation.
- Keep A15 optional-place restructuring, A23 effect/alias analysis, and
  broader optimizer work outside this roadmap.

## Decision gates

### Gate 1 — Is redundant HIR reconstruction demonstrated and migration safe to attempt?

Evaluate this gate after CP01 against deterministic structural counts and
exact neutral/HIR parity fixtures.

**Go only when all of these hold:**

- a maintained nontrivial class/array fixture clones constructor or assignment
  capability records into a provisional view;
- a maintained array fixture constructs provisional HIR array entries in
  addition to its final published table;
- the baseline records every constructor and assignment convergence round
  without changing the result;
- neutral availability and failure paths exactly match the independently
  constructed HIR capabilities for every reviewed class and array identity;
  and
- recursive, unavailable, inheritance, optional, and class-array dependency
  fixtures terminate without an arbitrary cap.

Multiple convergence rounds strengthen the efficiency evidence but are not
required. Even a single round constructs full provisional HIR products and
maintains a second semantic algorithm.

**No-go:** any count cannot be observed without changing production behavior,
no maintained fixture constructs redundant provisional HIR state, neutral and
HIR results disagree, or the adversarial dependency corpus exposes an
unresolved semantic question. Do not implement CP02 or CP03. Proceed to CP04's
Gate 1 no-go branch, retain only independently useful regression coverage and
measurement evidence, document the discrepancy or absent payoff, and leave
the current type-check solver authoritative until a new design resolves it.

### Gate 2 — Does neutral-guided one-pass materialization earn retention?

Evaluate this gate after CP03. Correctness and ownership conditions are
mandatory. Supporting operational measurements use at least two paired
before/after runs with identical compiler profile, source inventory, warmups,
repetitions, host conditions, MIR profile, and runtime-trace policy. If a
wall-time or RSS result crosses a threshold in only one pair, capture a third
pair and use the two agreeing outcomes.

**Go only when all of these hold:**

- one immutable neutral result is the type-check request's sole availability
  and failure-path authority;
- provisional capability snapshots clone zero constructor or assignment plan
  records because those snapshots no longer exist;
- convergence builds zero provisional `HirArrayTypeTable` entries;
- each concrete class operation is materialized at most once per family and
  exactly one final HIR array table is built and moved into `HirProgram`;
- the neutral service remains free of HIR, MIR, diagnostic-rendering, pass,
  backend, cache, and publication dependencies;
- exact facts, failure paths, HIR plans and order, diagnostics, dumps,
  assembly, and native observations match the baseline;
- the old type-check availability solver is deleted rather than retained as a
  fallback or oracle;
- the final implementation is smaller or has a materially clearer ownership
  path than the duplicate solvers and convergence-time HIR construction; and
- no reviewed workload shows a repeatable median compiler-time or peak-RSS
  regression greater than five percent. A measurable speedup is not required.

**No-go:** any mandatory correctness or ownership condition fails, the target
construction counts are missed, old availability logic must remain, the
materializer needs phase-neutral HIR types or a general dependency framework,
or a repeatable operational regression crosses the threshold. Proceed to
CP04's Gate 2 no-go branch. Revert CP03 and any CP02 wiring that computes or
stores unused neutral facts. Restore the original type-check solver and final
array-table publication. Keep only independently useful measurements,
fixtures, neutral accessors, or dependency guards, record the failed condition,
and do not substitute a borrow-only optimization without a separate bounded
assessment.

## Progress

- [ ] CP01 — Measure reconstruction and freeze capability behavior
- [ ] CP02 — Establish the request-local neutral authority
- [ ] CP03 — Materialize concrete HIR plans once and decide Gate 2
- [ ] CP04 — Retain or remove the migration and close A17

## PR-sized implementation sequence

### CP01 — Measure reconstruction and freeze capability behavior

**Purpose:** demonstrate the actual reconstruction cost and preserve the
semantic boundary before changing either solver.

- [ ] Add a temporary test-only computation report around the current
  `CopyCapabilities::compute`. Count constructor and assignment convergence
  rounds, cloned capability records, provisional and final HIR array-table
  builds and entries, final publication clones, and class plan constructions.
- [ ] Add equivalent compact counts for neutral availability rounds and array
  entry evaluations without changing production query behavior.
- [ ] Keep instrumentation request-local, deterministic, saturating where
  counts can accumulate, and absent from public APIs, driver reporting, dumps,
  diagnostics, and release behavior.
- [ ] Establish maintained fixtures for empty, direct, inherited, nested
  class, optional, nested optional, class-array, nested-array, recursive, and
  independently unavailable constructor/assignment shapes.
- [ ] Freeze exact neutral availability, failure paths, HIR selected
  operations, base/field/final-field order, and array lifecycle slots for those
  fixtures. Retain explicit expectations rather than only comparing the two
  implementations with each other.
- [ ] Add
  `docs/development/COPY_CAPABILITY_MATERIALIZATION_MEASUREMENTS.md` with the
  commands, revision and dirty state, fixture inventory, structural counts,
  cleanup-measurement context, and Gate 1 decision.
- [ ] Run the measurement corpus and apply Gate 1 exactly as written. Record
  the decision in this roadmap before starting another task.

**Tests:** Run focused neutral lifecycle and type-check capability suites,
generic class/interface requirement tests, affected array/optional/inheritance
tests, and exact fixture determinism. Then run `make check`, `make msrv-check`,
documentation validation, and `git diff --check`.

**Exit criteria:** the baseline can be reproduced; exact behavior is protected
independently of implementation parity; Gate 1 records go or no-go with its
evidence; and work continues only along the selected branch.

### CP02 — Establish the request-local neutral authority

**Purpose:** make the accepted availability owner explicit at the type-check
boundary before deleting reconstruction logic.

This task is performed only after a Gate 1 go decision.

- [ ] Expose only the immutable class/array availability and failure-path
  queries needed by type checking. Keep vectors, mutation, convergence state,
  and operation-kind internals private to `type_capabilities`.
- [ ] Make `CopyCapabilities::compute` obtain one neutral result from the final
  selected `ResolvedProgram` and retain it for the type-check request.
- [ ] Delegate capability failure-path queries to the neutral result, removing
  the type checker's duplicate failure-path ownership while preserving exact
  diagnostic paths.
- [ ] During this transition, keep the existing HIR constructor/assignment and
  array construction intact and assert exhaustive availability parity at the
  completed `CopyCapabilities` boundary.
- [ ] Ensure generic publication queries continue to compute against their
  borrowed candidate view. Do not expose, move, or reuse their lazy cache.
- [ ] Extend the phase-boundary guard if needed so neutral lifecycle code may
  depend on resolved products but cannot acquire HIR, MIR, pass, backend, or
  driver dependencies.
- [ ] Update the measurement report with the temporary cost of the explicit
  neutral authority and confirm CP03, rather than CP02 alone, is the intended
  retained state.

**Tests:** Cover exhaustive identity-indexed parity, exact failure-path
borrowing, incomplete or rejected generic candidate views, final publication
selection, and direct public type-check entry behavior. Run focused capability,
generic specialization, phase-boundary, diagnostic-determinism, and HIR-dump
tests, followed by `make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** each type-check request owns one immutable neutral result;
failure diagnostics use that result; the existing HIR construction remains
behaviorally unchanged under exhaustive consistency assertions; no fact cache
crosses resolver publication; and CP03 has one explicit authority to consume.

### CP03 — Materialize concrete HIR plans once and decide Gate 2

**Purpose:** remove convergence-time HIR construction and determine whether the
selected ownership split is simpler and operationally acceptable.

This task is performed only after CP02 and a Gate 1 go decision.

- [ ] Replace the recursive availability-and-plan `CapabilitySet::compute`
  with an identity-indexed HIR materializer guided exclusively by neutral
  availability facts.
- [ ] Materialize unavailable, user, and synthesized constructor plans once,
  preserving direct-base-first and declaration-ordered field operations.
- [ ] Materialize assignment plans once after constructors, preserving
  optional-class constructor requirements and direct final-field order.
- [ ] Treat an available synthesized dependency cycle or missing selected
  operation as a compiler consistency defect. Do not add a second availability
  fallback.
- [ ] Adapt array lifecycle construction to consult neutral array facts and
  completed class plans, then build the final table once in canonical identity
  order. Preserve default and destruction construction independently.
- [ ] Consume `CopyCapabilities` after checking and move its final array table
  into `HirProgram`.
- [ ] Remove provisional `CopyCapabilities` values, set clones, per-round HIR
  array builds, invalidation loops, duplicate failure paths, and obsolete
  helpers. Organize the remaining private materializer behind a concise
  type-check capability facade if the responsibility warrants a submodule.
- [ ] Replace implementation-parity-only tests with explicit expected neutral
  facts and HIR plans plus exhaustive boundary consistency checks. Do not keep
  the old solver as test-only code.
- [ ] Capture post-change structural counts and paired cleanup measurements,
  compare them with CP01, and apply Gate 2 exactly as written. Record the
  decision and selected CP04 branch immediately.

**Tests:** Run the complete neutral lifecycle and type-check capability suites;
generic class/interface validation; array, optional, inheritance, copy,
final-field, and shared-owner type checking; relevant MIR and backend lifecycle
tests; process-determinism cases; and matching golden suites. Then run
`make check`, `make msrv-check`, documentation validation, and
`git diff --check`.

**Exit criteria:** Gate 2 has an evidence-backed go or no-go decision. A go
meets every construction-count, ownership, behavior, complexity, and
operational condition. A no-go identifies the failed condition and authorizes
reversion rather than further generalization.

### CP04 — Retain or remove the migration and close A17

**Purpose:** leave one coherent repository state and durable evidence after the
experiment.

- [ ] Follow exactly one closure branch:
  - **Gate 2 go:** retain neutral-guided one-pass materialization; remove
    temporary probes with no continuing regression value; retain concise
    structural assertions or counters only where they protect the boundary.
  - **Gate 1 no-go:** do not implement CP02 or CP03; remove measurement hooks
    without continuing value and retain exact fixtures and evidence that
    explain why migration was not attempted.
  - **Gate 2 no-go:** revert CP03 and migration-only CP02 authority wiring;
    restore the original solver and publication path; retain only independently
    useful facts, tests, guards, and measurements.
- [ ] Audit production lifecycle availability decisions, failure-path owners,
  provisional snapshots, array-table construction, public products, and phase
  dependencies. Record any remaining intentional duplication precisely.
- [ ] Update the accepted design with the delivered or rejected outcome and
  link the measurement evidence. Put any narrower actionable follow-up in a
  separate indexed discoveries document rather than expanding A17.
- [ ] Update living compiler and testing documentation to describe only the
  final retained behavior.
- [ ] Mark A17 complete with its bounded outcome and validation. Do not claim a
  runtime or compiler speedup beyond the recorded workloads.
- [ ] Mark this roadmap complete, archive it with the design when no task
  remains, update active/archive indexes, and repair every incoming link.

**Tests:** On the final retained repository state, rerun all focused affected
suites, documentation validation, `make check`, `make msrv-check`, and
`git diff --check`. Perform final acceptance from an artifact-free snapshot or
clean checkout and capture the final measurement report named by the design.

**Exit criteria:** the repository contains either one measured, neutral-guided
HIR materialization path or the restored original solver with a documented
no-go result. Experimental machinery without continuing value is absent; all
observable behavior and phase boundaries are preserved; the evidence and
decision are durable; and A17 is closed accurately.

## Ordering and dependencies

CP01 precedes ownership changes because static inspection alone does not
quantify reconstructed records or protect exact failure paths and plan order.
CP02 establishes one immutable authority while the old construction remains
available for exhaustive transition assertions. CP03 then removes the old
availability decisions and applies the decisive retention gate. CP04 is
mandatory for every outcome so an unsuccessful experiment cannot leave
dormant migration code.

A08 is the enabling dependency: its neutral resolved capability service and
candidate view boundary are complete. A15 and A23 are explicitly independent
and must not enter this work. A later attempt to share resolver-local results,
introduce dependency-driven convergence, or optimize another lifecycle family
requires fresh evidence and a separate design.
