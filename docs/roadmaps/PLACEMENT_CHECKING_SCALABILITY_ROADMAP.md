# Placement Checking Scalability Roadmap

Status: planned; PS01 is next.
Planning baseline: `b3b109c7`, the committed design draft accepted by the user.
Implementation baseline: record the committed roadmap revision immediately
before PS01 changes code or measurement support.
Accepted design:
[Placement Checking Scalability Design Proposal](PLACEMENT_CHECKING_SCALABILITY_DESIGN_PROPOSAL.md).
Parent program:
[Low-Level Compiler Architecture](LOW_LEVEL_COMPILER_ARCHITECTURE_DESIGN_PROPOSAL.md).

This roadmap resolves the recursive-optional and live-array placement-cost
discoveries before LA05 production adoption. It keeps independent placement
checking and every existing native witness, while making the finite must-analysis
compact and incremental enough for ordinary development validation.

## Scope and invariants

- `PlacementDraft` remains untrusted. Only the independent checker can publish
  `CheckedPlacement`, and it consumes no producer availability or dominance
  claims.
- The location/token universe, definite-content lattice, epoch invalidation,
  aliases, calls, transfers, scratch lifetime, simultaneous edge rebinding,
  entry seed and greatest fixed point retain their accepted meanings.
- Strict replay remains separate from convergence and owns stable first-failure
  location, reason and source attribution.
- Structural and static legality continue to cover unreachable blocks;
  unreachable content states grant no authority.
- The implementation remains shared and target-independent. Native x86 and
  synthetic portability targets exercise the same solver; no x86 register or
  ABI assumption enters compact state or scheduling.
- All six slow optional/array native tests stay enabled in the ordinary
  workspace suite. Timing determines roadmap acceptance but never test success.
- Lowering, lifecycle expansion, selected CFGs, ABIs, physical realization,
  assembly and production backend selection are unchanged.
- CFG compaction, opaque lifecycle operations, register allocation, LA05
  adoption and general LIR optimization are outside this roadmap.
- Wall-clock reports are observational artifacts under `build/measurements/`.
  Deterministic structural work metrics and correctness results remain the
  reviewable evidence.

## Checkpoints

1. **Diagnosis after PS01.** Proceed only if phase measurements identify
   placement checking as the dominant cost for both a recursive-optional and a
   live-array witness. Otherwise stop, amend the accepted design and assign the
   measured owner before changing the solver.
2. **Compact-state checkpoint after PS03.** Re-measure all witnesses. PS04 adds
   only immutable indices shown material by the measurements. If the final
   performance threshold is already met, an index with no measured benefit is
   skipped and its disposition recorded.
3. **Schedule checkpoint before PS05.** Proceed with the worklist only when
   structural counters show repeated evaluation of unchanged blocks remains
   material or the final threshold is still unmet. A skipped worklist leaves
   the living deterministic-round contract unchanged and requires documented
   evidence that the accepted performance boundary is already met.
4. **Acceptance after PS06.** Both D01 and D02 must pass correctness and
   performance thresholds. If checker-local changes miss them, keep only
   independently useful low-complexity improvements, leave the discoveries
   open and amend the design before considering CFG compaction or another owner.

No checkpoint permits moving witnesses to `check-long`, weakening validation or
accepting a timing-dependent result.

## Progress

- [ ] PS01 — Establish placement phase measurements and pass the diagnosis checkpoint
- [ ] PS02 — Establish fixed-point equivalence evidence and migration scaffolding
- [ ] PS03 — Replace tree-set lattice states with compact finite bitsets
- [ ] PS04 — Precompute measured immutable placement relations
- [ ] PS05 — Replace full convergence rounds with a deterministic worklist
- [ ] PS06 — Pass the optional/array performance acceptance checkpoint
- [ ] PS07 — Cumulative review, artifact cleanup and closure

## PR-sized implementation sequence

### PS01 — Establish placement phase measurements and pass the diagnosis checkpoint

**Purpose:** turn the observed suite regression into reproducible phase evidence
before optimizing the checker.

- [ ] Record the actual implementation baseline from current history before the
  first code or measurement-support change.
- [ ] Add a narrow owner-private or test-only observation path that reports
  planning, both lowering passes, selection, placement production, placement
  checking, frame planning, realization/checking, publication and native
  execution separately.
- [ ] Report deterministic placement dimensions and work: reachable blocks,
  edge occurrences, locations by kind, tokens, state bits, selected events,
  transfers, convergence visits, fact removals and peak pending work.
- [ ] Add a reproducible measurement command following existing repository
  conventions. Record repository/profile/host identity, warm-up and repeated
  samples; write raw JSON below an ignored `build/measurements/placement-checking/`
  directory.
- [ ] Create a maintained development note containing commands, witness IDs,
  baseline medians, peak RSS observations and interpretation limits.
- [ ] Measure all six witnesses and the empty-array/small-scalar controls. Record
  whether placement checking dominates D01 and D02 independently.
- [ ] Update the artifact ledger with every observation seam, counter and report
  helper introduced here.

**Tests:** measurement-support unit tests; focused pilot observation tests;
determinism checks proving elapsed timing does not enter compiler dumps or
results; `make static-check`.

**Exit criteria:** another developer can reproduce the structural and timing
report, placement-checking time is separated from placement production, and the
diagnosis checkpoint explicitly says go or no-go. A no-go blocks PS02 and
requires a design amendment.

### PS02 — Establish fixed-point equivalence evidence and migration scaffolding

**Purpose:** make the forthcoming representation and schedule changes
independently reviewable against the accepted round semantics.

- [ ] Extend the existing owner-local placement specification oracle to model
  the accepted deterministic-round join and strict replay cases needed for
  solver comparison; do not import production solver control flow.
- [ ] Add a test-only canonical fixed-point digest or equivalent comparison view
  without exposing mutable state or widening `CheckedPlacement` authority.
- [ ] Compare production and oracle acceptance, converged state, first strict
  failure location/reason/origin and capacity disposition on the maintained
  loop, entry-backedge, duplicate-edge, epoch, call, alias, preservation,
  scratch and unreachable counterexamples.
- [ ] Add bounded deterministic generation of small selected graphs and manual
  placements, including empty-token and first-propagation-top cases.
- [ ] Isolate production state operations behind a small cohesive private
  interface so PS03 can change representation without changing transfer
  semantics.
- [ ] Record the round oracle, digest and generated comparison runner in the
  artifact ledger with PS07 disposition.

**Tests:** all placement owner tests; generated equivalence cases with fixed
seeds and bounded sizes; repeated runs proving deterministic comparison and
failure selection; `make compiler-test`.

**Exit criteria:** the current production round solver and independent oracle
agree on the complete comparison corpus, state representation is privately
encapsulated, and no testing seam is reachable from ordinary compilation.

### PS03 — Replace tree-set lattice states with compact finite bitsets

**Purpose:** remove the dominant dense-top allocation and cloning cost while
retaining the existing round schedule for a clean representation comparison.

- [ ] Assign deterministic checker-private token bits and store each location's
  definite contents as canonically masked machine words.
- [ ] Implement top, empty, membership, iteration, insertion, removal,
  intersection-with-removal-count, equality and compatible-token capture without
  exposing bit identities outside placement checking.
- [ ] Preserve definition epochs, writes, clobbers, transfers, parameter
  rebinding and strict replay through the private state interface.
- [ ] Keep checked arithmetic for state dimensions and allocation sizes;
  unrepresentable capacity rejects with the existing structured reason.
- [ ] Compare every focused and generated case with the independent round
  oracle, including zero tokens and a partially used final word.
- [ ] Re-run the complete witness measurement and record elapsed, phase, RSS and
  structural changes against PS01.

**Tests:** compact-set unit tests at zero, word and cross-word boundaries;
placement equivalence/counterexample suites; all six native witnesses;
`make compiler-test` and `make static-check`.

**Exit criteria:** production uses compact state with unchanged round semantics,
all oracle comparisons and diagnostics match, and the compact-state checkpoint
records which remaining costs justify PS04 and PS05.

### PS04 — Precompute measured immutable placement relations

**Purpose:** remove repeated target/draft relation scans that remain material
after compact state, without creating speculative caches.

- [ ] From PS03 evidence, select only material relations among location lookup,
  overlap closures, resource-unit kills, call-invalidated ABI slots,
  transfer-point lifetime expiry and representation-compatible token masks.
- [ ] Build selected indices once per check from the same exact draft, selected
  snapshot and immutable target facts used by current validation.
- [ ] Keep indices inside the check invocation; reject stale/cross-target reuse
  by construction rather than adding cache invalidation machinery.
- [ ] Prove indexed and direct interpretations agree for native and synthetic
  targets, overlapping widths, ABI signature aliases and partial preservation.
- [ ] Measure each retained index independently where practical. Remove an
  index whose complexity has no demonstrated benefit.
- [ ] Re-run structural and timing reports and make the schedule checkpoint
  decision explicit.

**Tests:** index-construction and equivalence tests; complete placement owner
suite; native/synthetic target cases; six native witnesses; `make compiler-test`.

**Exit criteria:** every retained index has a measured hot-path consumer and
identical semantics, unhelpful candidates are absent, and the roadmap records
whether PS05 proceeds or is skipped under the accepted stop condition.

### PS05 — Replace full convergence rounds with a deterministic worklist

**Purpose:** avoid interpreting unchanged blocks and rebuilding global states
while computing the same greatest fixed point.

- [ ] Implement stable reachability and deterministic FIFO successor scheduling,
  preserving selected block and edge-occurrence order.
- [ ] Schedule each reachable block on first propagation even when the lattice
  has no tokens or the first edge state is top; later scheduling requires a
  successful input-state change.
- [ ] Preserve the fixed entry seed as an additional predecessor across
  backedges and intersect every reachable predecessor edge occurrence.
- [ ] Track fact removals and queue processing with checked arithmetic. Enforce
  the finite lattice bound plus first-reach scheduling without wall-clock or
  arbitrary iteration limits.
- [ ] Keep speculative visits non-strict. After convergence, run strict replay
  in the existing stable selected order so diagnostics remain schedule-independent.
- [ ] Compare fixed-point digests and strict results against the round oracle on
  every focused/generated case and both target profiles.
- [ ] Update the living placement contract only after equivalence passes,
  describing semantic convergence and deterministic scheduling rather than a
  particular queue container.

If PS04 records a justified skip, mark every checklist item with that disposition
and leave the living round contract unchanged; do not land dormant worklist code.

**Tests:** worklist first-reach/deduplication/termination unit tests; oracle
equivalence over loops and counterexamples; stable-failure repeated runs;
placement, native pilot and portability suites; `make compiler-test`,
`make static-check`, and `make msrv-check`.

**Exit criteria:** when implemented, the worklist matches the round oracle and
all existing diagnostics while structural counters show reduced redundant
visits. When skipped, PS04 evidence already meets final thresholds and documents
why schedule complexity has no present consumer.

### PS06 — Pass the optional/array performance acceptance checkpoint

**Purpose:** decide the focused effort using the retained production-path
witnesses rather than inferred algorithmic improvement.

- [ ] Run repeated isolated measurements for all six witnesses and the controls
  from the same host/profile protocol as PS01.
- [ ] Demonstrate at least a 5x median elapsed improvement for every witness,
  with the worst witness below 30 seconds.
- [ ] Demonstrate a median cached `cargo test --locked --workspace` below
  90 seconds on the baseline host and at least 50% lower peak RSS for the worst
  witness.
- [ ] Confirm representative small placements do not regress by more than 20%
  beyond recorded measurement noise.
- [ ] Run every placement correctness/equivalence test and confirm the six
  witnesses remain ordinary, enabled tests with unchanged source semantics.
- [ ] Update the development measurement note with raw-report locations,
  before/after summaries and the D01/D02 decision.
- [ ] If either discovery misses its threshold, stop before closure, retain only
  justified improvements and amend the design; do not reclassify the tests.

**Tests:** measurement-support tests, exact witness tests, complete placement and
native pilot suites, cached workspace repetitions, `make check`.

**Exit criteria:** all correctness gates pass, every quantitative threshold is
met for both discovery shapes, and D01/D02 have objective closure evidence.

### PS07 — Cumulative review, artifact cleanup and closure

**Purpose:** review the whole scalability change as one correctness-boundary
revision, remove temporary machinery and hand a maintainable checker to LA05.

- [ ] Review `git diff <implementation-baseline>..HEAD` in stat, name-status and
  full forms, plus staged, unstaged and untracked work. Use commit history to
  include scaffolding introduced and removed across manually committed tasks.
- [ ] Reconcile every artifact-ledger entry. Remove temporary timing seams,
  duplicate round implementations, comparison-only exports, allowances and
  diagnostic output; retain only measurement/oracle support with a continuing
  documented LA05 or regression role.
- [ ] Review state, indices and solver modules for cohesive ownership, narrow
  visibility, clear invariants and absence of target-specific assumptions.
- [ ] Confirm the accepted design, living placement/backend/testing contracts,
  measurements and implementation agree; remove roadmap codes from living code
  and comments.
- [ ] Mark D01 and D02 resolved with links to retained evidence, update the
  low-level architecture and migration handoff, and make LA05 design the next
  active program step.
- [ ] Run the complete repository gates from an artifact-free snapshot or clean
  checkout and repeat the final measurement protocol.
- [ ] Record baseline, endpoint, validation and residual artifact dispositions;
  mark the roadmap complete, archive it and its frozen design, and repair all
  indexes and relative links.

**Tests:** focused placement/native suites, deterministic comparison tests,
measurement-support tests, `make check`, `make check-long`, `make msrv-check`,
documentation validation and the final repeated performance report.

**Exit criteria:** the cumulative implementation preserves independent placement
authority and deterministic failures, all accepted performance bounds still
pass, no temporary bridge or hidden alternate solver remains, D01/D02 are
closed, and LA05 receives one maintained scalable baseline checker.

## Artifact ledger

| Artifact | Introduced | Intended disposition | Retention criterion |
| --- | --- | --- | --- |
| Phase timing and placement structural-metric observation seam | PS01 | Review and narrow/remove in PS07 | Retain only if the owner-private interface is deterministic, has no normal-compilation cost and is used by LA05 measurement |
| Placement-checking measurement command and development note | PS01 | Retain through LA05 | Reproducible reports, ignored generated artifacts and maintained witness identifiers |
| Canonical fixed-point digest/comparison view | PS02 | Remove or keep test-only in PS07 | Retain only if it materially protects solver equivalence without exposing authority or adding material suite cost |
| Extended deterministic-round oracle | PS02 | Reconcile in PS07 | Retain as independent specification coverage only if bounded ordinary tests remain fast; never compile into production |
| Generated solver-equivalence cases | PS02 | Prefer permanent bounded tests | Fixed seeds, stable size bound, independent semantics and low ordinary-suite cost |
| Direct relation paths superseded by immutable indices | PS04 | Remove with each accepted index | Keep only a test oracle where it provides independent equivalence evidence |
| Full-round production solver superseded by worklist | PS05 | Remove in PS05 after equivalence | A test-only round oracle may remain under the separate oracle criterion; no production alternative |

Update this ledger as commits introduce concrete symbols. Record the introducing
task and commit, removal task and final disposition. New discoveries that do not
block this roadmap belong in
`docs/roadmaps/PLACEMENT_CHECKING_SCALABILITY_DISCOVERIES.md`, created only when
there is an actionable finding.

## Ordering and dependencies

PS01 is a hard diagnosis gate. PS02 establishes independent equivalence before
production representation changes. PS03 changes representation while retaining
the current schedule, isolating semantic and performance effects. PS04 admits
only measured indices. PS05 changes convergence order only after compact-state
equivalence and only when the schedule checkpoint justifies it. PS06 applies the
same-host acceptance protocol to both discoveries. PS07 reviews all commits and
temporary artifacts together before clearing the LA05 dependency.

PS02 may prepare bounded oracle cases while PS01 measurements run, but no
production solver change starts before the diagnosis is a go. Later tasks are
sequential because each checkpoint determines whether the next complexity has a
demonstrated consumer.
