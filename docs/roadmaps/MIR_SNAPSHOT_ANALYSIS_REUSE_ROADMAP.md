# MIR Snapshot Analysis Reuse Roadmap

Status: in progress; S02 is complete and S03 is next.

This roadmap implements
[cleanup finding A19](CODEBASE_CLEANUP_AUDIT.md#a19--reuse-analyses-within-an-immutable-mir-snapshot)
through the accepted
[MIR snapshot analysis reuse design](../archive/MIR_SNAPSHOT_ANALYSIS_REUSE_DESIGN_PROPOSAL.md).
It first measures repeated local constant analysis on unchanged proof-rich MIR,
then builds and enables a runner-owned cache only if two explicit decision
gates justify it.

The roadmap has two valid endpoints. A successful experiment leaves one typed,
measured proof-rich analysis session in production. An unsuccessful experiment
removes cache-specific machinery, preserves the useful evidence, and closes
A19 as measured and unjustified. Completing every planned implementation step
is not itself success; the go/no-go criteria decide which endpoint is correct.

## Scope and invariants

- Use proof-rich local constant analysis as the only initial reuse candidate.
- Associate every result with one runner-owned verified program snapshot and
  one callable identity.
- Preserve a session only across an unchanged pass outcome.
- Drop the complete session after every changed outcome and at proof
  normalization, even when only one callable changed.
- Keep mandatory verification, final resealing, seal-owned reachability,
  checkpoints, pass order, diagnostics, dumps, artifacts, and runtime behavior
  unchanged.
- Measure before enabling memoization and retain a private comparison mode
  until the final decision is recorded.
- Keep local constant semantics and solver implementation under their current
  optimization-analysis owner. The pipeline session owns lifetime and
  memoization only.
- Keep every new facade and callback type crate-private. No request or CLI
  cache option is introduced.
- Do not add revision keys, structural hashes, per-callable invalidation,
  preservation declarations, a type-erased analysis manager, a final-stage
  session, or caching for topology, dominance, reachability, lifecycle,
  liveness, censuses, or rewrite plans.
- Treat changed-callable summaries as reporting evidence, never as authority
  to preserve cached facts.

## Decision gates

### Gate 1 — Is same-snapshot reuse demonstrated?

Evaluate this gate after S01 using memoization-disabled measurements from the
reviewed cleanup workloads under the default schedule.

**Go:** at least one maintained workload requests a local constant solution
for the same callable more than once without an intervening changed outcome or
proof-normalization boundary. Record the exact workload, pass occurrences,
requests, distinct callable-snapshot keys, and potentially avoidable
computations. Mark S01 complete and continue to S02.

**No-go:** no maintained workload contains such a repeated request. Do not
implement S02 or S03. Proceed directly to the no-go branch of S04, remove
measurement-only pass-context or session scaffolding that has no independent
value, restore direct solver calls, retain the evidence document, mark S02 and
S03 as not pursued after Gate 1, and close A19 as measured and unjustified.

### Gate 2 — Is enabled reuse beneficial enough to retain?

Evaluate this gate after the bounded S03 prototype. All correctness conditions
are mandatory. Operational comparisons use at least two paired before/after
runs with identical source inventory, optimized assertion-enabled compiler,
host conditions, warmups, repetitions, MIR profile, and runtime-trace policy.

**Go only when all of these hold:**

- cache-enabled and cache-disabled runs have identical schedules, pass
  outcomes, diagnostics, MIR observations, assembly hashes, and native result
  digests;
- at least one nontrivial maintained workload reduces local constant
  computations by at least 20 percent, with every hit belonging to an unchanged
  callable snapshot;
- in both paired comparisons, at least one reviewed workload improves median
  compiler wall time by more than the sum of its before/after median absolute
  deviations;
- no reviewed workload repeatably regresses by that same dispersion rule; and
- no reviewed workload has a repeatable peak-RSS increase greater than five
  percent.

If a wall-time or RSS result crosses a limit in only one comparison, run a
third pair and use the two agreeing outcomes. Record raw reports and the
calculation, not only the decision.

**No-go:** any correctness condition fails, the deterministic computation
threshold is missed, no repeatable timing benefit is demonstrated, a stable
timing regression remains, or retained memory exceeds the limit. Proceed to
the no-go branch of S04. Remove memoization, the session/cache representation,
reference-counted result plumbing, and cache-driven callback changes. Restore
direct solver ownership. Keep only measurement code or tests with clear
continuing value, record why the threshold failed, mark A19 complete as
measured and unjustified, and do not substitute a different cached analysis
without a new design decision.

## Progress

- [x] S01 — Instrument uncached requests and decide Gate 1
- [x] S02 — Establish snapshot-bound session ownership
- [ ] S03 — Enable bounded reuse and decide Gate 2
- [ ] S04 — Deliver or remove reuse and close A19

## PR-sized implementation sequence

### S01 — Instrument uncached requests and decide Gate 1

**Purpose:** establish whether the default pipeline actually repeats local
constant work on one unchanged snapshot before adding memoization.

- [x] Add a private `pipeline::snapshot_analysis` facade with a closed local-
  constant analysis identity, deterministic saturating usage counts, and
  colocated tests. Keep memoization disabled.
- [x] Introduce the stage-specific proof-pass context selected by the design.
  It owns the existing capability, borrows the runner's measurement state, and
  mirrors unchanged and rewrite operations without widening mutation access.
- [x] Route every production proof-rich local constant request through the
  typed context. Remove direct solver calls from pass implementations after
  confirming that every current consumer is covered.
- [x] Record, by schedule occurrence, requests, computations, repeated
  callable-snapshot requests, results present before the occurrence, inserted
  results, and discarded results. With memoization disabled, requests equal
  computations and result counts remain zero.
- [x] Keep detailed-recording selection observational: enabling reports or an
  inspector must not cause a query or change aggregate counts.
- [x] Add the maintained
  `MIR_ANALYSIS_REUSE_MEASUREMENTS.md` evidence document under
  `docs/development/`. Record commands, compiler revision and dirty state,
  inputs, repetition policy, raw report paths, deterministic counts, timing,
  RSS, and Gate 1's decision.
- [x] Run the reviewed cleanup workloads and apply Gate 1 exactly as written.
  Update this roadmap immediately with the decision and selected next step.

**Gate 1 decision:** go. The reviewed matrix recorded 12,040 uncached requests
and computations across 6,399 callable-snapshot keys, including 5,641 repeated
same-snapshot requests. The exact occurrences, operational context, and raw
report paths are retained in the
[measurement evidence](../development/MIR_ANALYSIS_REUSE_MEASUREMENTS.md).
After S01 validation, continue to S02; do not take the Gate 1 no-go branch.

**Tests:** Cover saturating aggregation, schedule-ordered records, repeated
requests within one measurement epoch, reset after a synthetic changed
outcome, proof-normalization reset, disabled occurrence recording, inspector
independence, exact pass failure attribution, and independent-process
determinism. Run all local constant and affected optimization tests,
`make measurement-support-test`, `make check`, `make msrv-check`, and
`git diff --check`.

**Exit criteria:** every production local constant solver request crosses one
typed measurement boundary with memoization disabled; the baseline is
reproducible; Gate 1 has a recorded go or no-go decision; and the roadmap
continues only along that branch.

### S02 — Establish snapshot-bound session ownership

**Purpose:** prove complete invalidation mechanically before production can
reuse any result.

This task is performed only after a Gate 1 go decision.

- [x] Extend `pipeline::snapshot_analysis` with one lazy proof-rich session
  containing a deterministic callable-keyed table of successful
  `Arc<LocalConstantSolution>` results. Keep solver construction and result
  semantics in the existing local constant module.
- [x] Let the proof-pass context resolve `CallableId` through its exact verified
  program before computing or returning a result. Reject unknown,
  non-executable, foreign, or mismatched identities deterministically.
- [x] Make the runner retain the session beside the verified product after an
  unchanged outcome and drop it before verifying every changed result.
- [x] Drop the proof-rich session unconditionally at proof normalization. Do
  not introduce a final-stage session or duplicate final seal reachability.
- [x] Permit immutable result handles only inside the current transform. Keep
  all outcome and checkpoint types free of session/result handles, and consume
  the pass context before rewrite or unchanged handoff completes.
- [x] Add a private test policy which selects measurement-only or memoized
  behavior. Production remains measurement-only throughout S02.
- [x] Count hits, insertions, entries present before an occurrence, and entries
  discarded on invalidation without changing existing pass-owned metrics.

**Tests:** Use synthetic exact schedules rather than production heuristics to
prove one computation across consecutive unchanged consumers, complete reset
after a change, global reset when one callable changes, reset across proof
normalization, stable repeated pass identities, no query from disabled passes,
no facts exposed to inspectors, and exact failure ownership. Add compile-fail
or visibility tests where they materially prove that outcomes and external
callers cannot retain a session.

Run focused pipeline ownership and local constant tests, then `make check`,
`make msrv-check`, and `git diff --check`.

**Exit criteria:** memoized test execution can reuse a successful result only
on the exact unchanged proof-rich snapshot; every mutation and stage transition
causes complete invalidation; production behavior is still uncached; and no
new public or final-stage analysis surface exists.

**Result:** the private memoized test policy shares one typed immutable result
across unchanged occurrences. The runner clears the complete callable table
before changed-output verification and at proof normalization, and the usage
contract now reports cache hits and table lifecycle counts. Production remains
on the measurement-only policy until S03 evaluates enabled reuse.

### S03 — Enable bounded reuse and decide Gate 2

**Purpose:** determine whether the mechanically safe cache produces enough
real benefit to justify its complexity and retained memory.

This task is performed only after S02 and a Gate 1 go decision.

- [ ] Enable memoization for production proof-rich local constant queries.
  Keep the test-only measurement policy for direct equivalence checks.
- [ ] Migrate all default-schedule local constant consumers to shared handles
  without combining their topology observations, candidate selection, rewrite
  plans, or pass measurements.
- [ ] Preserve analysis failure text and requesting-occurrence attribution.
  Insert only successful complete results and never recompute after a hit.
- [ ] Compare memoized and measurement-only schedules for exact typed local
  constant solutions across primitive chains, checked integer and floating
  protocols, logical selections, carrier facts, loops, and disconnected
  blocks.
- [ ] Capture at least two paired pre-reuse/post-reuse reports for every
  reviewed workload. Include deterministic usage counts, compiler wall time,
  peak RSS, schedule and pass outcomes, artifacts, and native observations in
  the evidence document.
- [ ] Apply Gate 2 exactly as written, record every calculation and the final
  go or no-go decision, and update S04 to name the applicable closure branch.

**Tests:** Compare memoized and measurement-only execution on identical exact
schedules containing unchanged chains, changed passes, repeated pass
identities, failures, and proof normalization. Require identical MIR and
observable outputs plus the expected computation-count difference. Run every
local constant consumer suite, pipeline composition and determinism tests,
`make cleanup-baseline` with the recorded selection and repetition policy,
`make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** Gate 2 has a reproducible evidence-backed decision. A go
proves exact behavioral parity, a meaningful deterministic computation
reduction, repeatable timing benefit, and bounded RSS. A no-go identifies the
failed condition and authorizes removal, not further cache generalization.

### S04 — Deliver or remove reuse and close A19

**Purpose:** leave one coherent repository state after the experiment rather
than treating implemented cache code as automatically permanent.

- [ ] Follow exactly one closure branch:
  - **Gate 2 go:** retain the typed production session, remove comparison-only
    scaffolding with no regression-testing value, keep useful deterministic
    usage reporting, and document the snapshot/invalidation contract in living
    compiler and reporting documentation.
  - **Gate 1 or Gate 2 no-go:** remove the cache/session representation and
    cache-specific pass context, callback, `Arc`, and consumer plumbing;
    restore direct local constant solver calls; keep only independently useful
    measurement vocabulary and tests; and document the measured reason reuse
    was rejected.
- [ ] In either branch, audit direct solver call sites, session/result
  visibility, outcomes, checkpoints, proof normalization, final seals, and
  backend inputs for one unambiguous owner.
- [ ] Update the accepted design record with the delivered or rejected outcome
  and link its evidence. Record any narrower measured follow-up in an indexed
  discoveries document rather than expanding A19.
- [ ] Mark A19 complete with its bounded outcome and validation. Do not claim a
  speedup on the no-go branch or beyond the measured workloads on the go branch.
- [ ] Mark this roadmap complete, archive it, remove it from the active index,
  add it to the archive index, and repair every incoming link.

**Tests:** On the final retained repository state, rerun focused affected
suites, cache-policy equivalence tests only if the cache remains, documentation
validation, `make check`, `make msrv-check`, and `git diff --check`. Capture a
final clean-checkout measurement report for the evidence document.

**Exit criteria:** the repository contains either a measured, beneficial,
fully invalidated typed cache or no cache-specific production machinery. The
decision and evidence are durable, A19 is complete, mandatory verification and
all observable compiler behavior are preserved, and no analysis survives a
changed snapshot.

## Ordering and dependencies

S01 comes first because static call-site repetition does not prove runtime
reuse. Gate 1 prevents a cache prototype when the selected workloads do not
preserve snapshots between requests. S02 separates safety from benefit by
proving ownership and invalidation while production remains uncached. S03 then
enables exactly one typed result and evaluates Gate 2. S04 is mandatory for
both outcomes and removes experimental machinery when the evidence rejects it.

The completed structural CFG work supplies the local snapshot vocabulary but
does not authorize caching its results. Final reachability remains part of the
normalized seal. Work on partial invalidation, other analysis kinds, schedule
changes, or skipped verification requires a separate measured finding and
design after this roadmap.
