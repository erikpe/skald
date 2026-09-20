# Placement Checking Scalability Roadmap

Status: active; PS01 through PS06 are complete, PS04 and PS05 were skipped at
their measured stop conditions, and PS07 cumulative closure is next.
Planning baseline: `b3b109c7`, the committed design draft accepted by the user.
Implementation baseline: `9eafca80`, the committed roadmap revision immediately
before PS01 changed code or measurement support.
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

- [x] PS01 — Establish placement phase measurements and pass the diagnosis checkpoint
- [x] PS02 — Establish fixed-point equivalence evidence and migration scaffolding
- [x] PS03 — Replace tree-set lattice states with compact finite bitsets
- [x] PS04 — Skip immutable relation indices at the compact-state checkpoint
- [x] PS05 — Skip the worklist at the schedule checkpoint
- [x] PS06 — Pass the optional/array performance acceptance checkpoint
- [ ] PS07 — Cumulative review, artifact cleanup and closure

## PR-sized implementation sequence

### PS01 — Establish placement phase measurements and pass the diagnosis checkpoint

**Purpose:** turn the observed suite regression into reproducible phase evidence
before optimizing the checker.

- [x] Record the actual implementation baseline from current history before the
  first code or measurement-support change.
- [x] Add a narrow owner-private or test-only observation path that reports
  planning, both lowering passes, selection, placement production, placement
  checking, frame planning, realization/checking, publication and native
  execution separately.
- [x] Report deterministic placement dimensions and work: reachable blocks,
  edge occurrences, locations by kind, tokens, state bits, selected events,
  transfers, convergence visits, fact removals and peak pending work.
- [x] Add a reproducible measurement command following existing repository
  conventions. Record repository/profile/host identity, warm-up and repeated
  samples; write raw JSON below an ignored `build/measurements/placement-checking/`
  directory.
- [x] Create a maintained development note containing commands, witness IDs,
  baseline medians, peak RSS observations and interpretation limits.
- [x] Measure all six witnesses and the empty-array/small-scalar controls. Record
  whether placement checking dominates D01 and D02 independently.
- [x] Update the artifact ledger with every observation seam, counter and report
  helper introduced here.

**Tests:** measurement-support unit tests; focused pilot observation tests;
determinism checks proving elapsed timing does not enter compiler dumps or
results; `make static-check`.

**Exit criteria:** another developer can reproduce the structural and timing
report, placement-checking time is separated from placement production, and the
diagnosis checkpoint explicitly says go or no-go. A no-go blocks PS02 and
requires a design amendment.

**Completed:** the two-repetition baseline is recorded in
[Placement Checking Performance](../development/PLACEMENT_CHECKING_PERFORMANCE.md).
Placement checking consumed 99.52% of native-pilot time for the D01 witness and
99.87% to 99.97% for all six D02 witnesses, while draft production remained in
the 5.18--28.28 ms range. Structural observations matched exactly between
repetitions. The diagnosis checkpoint is **go for D01 and D02**; PS02 may
proceed without amending the accepted design.

### PS02 — Establish fixed-point equivalence evidence and migration scaffolding

**Purpose:** make the forthcoming representation and schedule changes
independently reviewable against the accepted round semantics.

- [x] Extend the existing owner-local placement specification oracle to model
  the accepted deterministic-round join and strict replay cases needed for
  solver comparison; do not import production solver control flow.
- [x] Add a test-only canonical fixed-point digest or equivalent comparison view
  without exposing mutable state or widening `CheckedPlacement` authority.
- [x] Compare production and oracle acceptance, converged state, first strict
  failure location/reason/origin and capacity disposition on the maintained
  loop, entry-backedge, duplicate-edge, epoch, call, alias, preservation,
  scratch and unreachable counterexamples.
- [x] Add bounded deterministic generation of small selected graphs and manual
  placements, including empty-token and first-propagation-top cases.
- [x] Isolate production state operations behind a small cohesive private
  interface so PS03 can change representation without changing transfer
  semantics.
- [x] Record the round oracle, digest and generated comparison runner in the
  artifact ledger with PS07 disposition.

**Tests:** all placement owner tests; generated equivalence cases with fixed
seeds and bounded sizes; repeated runs proving deterministic comparison and
failure selection; `make compiler-test`.

**Exit criteria:** the current production round solver and independent oracle
agree on the complete comparison corpus, state representation is privately
encapsulated, and no testing seam is reachable from ordinary compilation.

**Completed:** focused hand-written placements now run production convergence
and an independently written Jacobi scheduler over the same validated
transition semantics. They compare canonical block-entry states and the full
strict failure, including location, reason and origin. The
corpus covers loops and entry backedges, duplicate edges, epochs, calls, ABI
aliases, preservation, scratch lifetime and unreachable code on both target
profiles. Thirty-two bounded generated selected diamonds cover accepted and
rejected joins twice, an empty-token graph covers first propagation when top
equals the entry state, and checked dimension cases compare capacity
disposition. The ordinary checker exposes no comparison API.

### PS03 — Replace tree-set lattice states with compact finite bitsets

**Purpose:** remove the dominant dense-top allocation and cloning cost while
retaining the existing round schedule for a clean representation comparison.

- [x] Assign deterministic checker-private token bits and store each location's
  definite contents as canonically masked machine words.
- [x] Implement top, empty, membership, iteration, insertion, removal,
  intersection-with-removal-count, equality and compatible-token capture without
  exposing bit identities outside placement checking.
- [x] Preserve definition epochs, writes, clobbers, transfers, parameter
  rebinding and strict replay through the private state interface.
- [x] Keep checked arithmetic for state dimensions and allocation sizes;
  unrepresentable capacity rejects with the existing structured reason.
- [x] Compare every focused and generated case with the independent round
  oracle, including zero tokens and a partially used final word.
- [x] Re-run the complete witness measurement and record elapsed, phase, RSS and
  structural changes against PS01.

**Tests:** compact-set unit tests at zero, word and cross-word boundaries;
placement equivalence/counterexample suites; all six native witnesses;
`make compiler-test` and `make static-check`.

**Exit criteria:** production uses compact state with unchanged round semantics,
all oracle comparisons and diagnostics match, and the compact-state checkpoint
records which remaining costs justify PS04 and PS05.

**Completed:** production states now share one deterministically sorted token
layout and store location contents in a flat, canonically masked `u64` matrix.
Every semantic operation still accepts `TransferValue`; numeric bit identities
remain inside `state.rs`. Checked dimension arithmetic and fallible reservation
map unrepresentable state storage to `CheckReason::Capacity`. Boundary tests
cover zero tokens, partial and crossed words, multiple locations and removal
counts, while the complete focused/generated corpus still agrees with the
independent round oracle.

The two-repeat PS03 checkpoint is recorded in
[Placement Checking Performance](../development/PLACEMENT_CHECKING_PERFORMANCE.md).
Witness wall time improved by 19.2x--120.8x and placement-checking time by
32.6x--138.1x; the worst measured witness is now 2.43 seconds and the former
891.8 MiB peak is 51.2 MiB. All structural counters, including rounds, visits
and fact removals, are unchanged. The representation alone already exceeds the
final witness and memory thresholds. No immutable relation is shown material,
so PS04 should record its candidates as skipped unless new profile evidence
identifies one. The unchanged round work is measurable but no longer justifies
PS05's scheduling complexity under the accepted stop condition; its formal
skip remains a later checkpoint decision.

### PS04 — Precompute measured immutable placement relations

**Purpose:** remove repeated target/draft relation scans only when they remain
material after compact state, without creating speculative caches.

- [x] Reviewed location lookup, overlap closures, resource-unit kills,
  call-invalidated ABI slots, transfer-point lifetime expiry and
  representation-compatible token masks against the PS03 checkpoint; none has
  a demonstrated material cost.
- [x] Skipped index construction, so no new snapshot lifetime, target binding or
  cache invalidation responsibility was introduced.
- [x] Retained the existing direct interpretations for native and synthetic
  targets, overlapping widths, ABI aliases and partial preservation.
- [x] Retained no index requiring independent measurement or a parallel direct
  implementation as a test oracle.
- [x] Used the complete two-repeat PS03 report for the schedule checkpoint: all
  witnesses already exceed the final speed and memory thresholds with unchanged
  structural work counters.

**Validation for the skip:** the PS03 compact-state and oracle suites, all six
native witnesses, `make compiler-test` and `make static-check` already cover the
unchanged direct relations. This documentation-only disposition adds no new
runtime behavior requiring duplicate tests.

**Exit criteria:** every retained index has a measured hot-path consumer and
identical semantics, unhelpful candidates are absent, and the roadmap records
whether PS05 proceeds or is skipped under the accepted stop condition.

**Disposition:** skipped. No index was retained because the compact state alone
reduced the worst witness to 2.43 seconds, all discovery witnesses improved by
at least 19.2x, and the largest measured RSS fell by 94.3%. The schedule
checkpoint therefore directs PS05 to use its skip path.

### PS05 — Replace full convergence rounds with a deterministic worklist

**Purpose:** avoid interpreting unchanged blocks and rebuilding global states
only when the remaining cost justifies changing convergence scheduling.

- [x] Skipped FIFO successor scheduling; production retains stable reachability
  and deterministic Jacobi rounds in selected block and edge-occurrence order.
- [x] Skipped first-propagation and queue-deduplication machinery, including the
  special empty-token/top scheduling path it would require.
- [x] Retained the existing fixed entry seed, backedge and predecessor-occurrence
  semantics without a second production solver.
- [x] Retained the checked finite round bound; no queue counters, allowances or
  invariant paths were introduced.
- [x] Retained non-strict convergence followed by strict stable replay, so
  diagnostic selection is unchanged.
- [x] Retained the bounded test-only round oracle for PS07 reconciliation; no
  worklist comparison API or dormant implementation was added.
- [x] Left the living placement contract unchanged because production still uses
  its documented deterministic-round schedule.

If PS04 records a justified skip, mark every checklist item with that disposition
and leave the living round contract unchanged; do not land dormant worklist code.

**Validation for the skip:** the PS03 report preserves exactly the same rounds,
block visits, edge visits and removal counts while meeting every current witness
and RSS threshold. No worklist behavior exists to test; PS06 owns the final
workspace, `make check` and repeated acceptance gates.

**Exit criteria:** when implemented, the worklist matches the round oracle and
all existing diagnostics while structural counters show reduced redundant
visits. When skipped, PS04 evidence already meets final thresholds and documents
why schedule complexity has no present consumer.

**Disposition:** skipped. Repeated round work remains visible structurally, but
placement checking now takes 0.58--1.41 seconds across the discovery witnesses.
That absolute cost is not material under the accepted threshold, so a second
convergence schedule would add correctness and maintenance risk without a
current consumer.

### PS06 — Pass the optional/array performance acceptance checkpoint

**Purpose:** decide the focused effort using the retained production-path
witnesses rather than inferred algorithmic improvement.

- [x] Run repeated isolated measurements for all six witnesses and the controls
  from the same host/profile protocol as PS01.
- [x] Demonstrate at least a 5x median elapsed improvement for every witness,
  with the worst witness below 30 seconds.
- [x] Demonstrate a median cached `cargo test --locked --workspace` below
  90 seconds on the baseline host and at least 50% lower peak RSS for the worst
  witness.
- [x] Confirm representative small placements do not regress by more than 20%
  beyond recorded measurement noise.
- [x] Run every placement correctness/equivalence test and confirm the six
  witnesses remain ordinary, enabled tests with unchanged source semantics.
- [x] Update the development measurement note with raw-report locations,
  before/after summaries and the D01/D02 decision.
- [x] Confirm both discoveries pass; no design amendment, test reclassification
  or rollback is required. If either had missed, the roadmap would have stopped
  before closure and retained only independently justified improvements rather
  than weakening the tests.

**Tests:** measurement-support tests, exact witness tests, complete placement and
native pilot suites, cached workspace repetitions, `make check`.

**Exit criteria:** all correctness gates pass, every quantitative threshold is
met for both discovery shapes, and D01/D02 have objective closure evidence.

**Completed:** the committed `c3978118` state was measured with the original
two-repeat, zero-warm-up protocol. D01 improved by 19.3x and every D02 witness by
45.5x--119.9x; the worst wall median is 2.42 seconds. The former worst-memory
witness fell from 891.8 MiB to 51.1 MiB. The controls improved by 4.4x and 6.2x,
so no representative small case regressed.

Two complete cached workspace runs passed in 52.14 and 52.73 seconds, for a
52.44-second median. `make check` passed static checks, the complete workspace
and placement/oracle suites, runtime tests and 650 golden cases. The six
witnesses remain ordinary enabled `#[test]` functions with unchanged source
semantics. The raw report and full before/after decision are recorded in
[Placement Checking Performance](../development/PLACEMENT_CHECKING_PERFORMANCE.md).
D01 and D02 both pass PS06 and are ready for formal closure during PS07.

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
| `compile_native_pilot_profiled`, `NativePilotProfile`, profiled lowering/placement/native-execution helpers and `PlacementCheckMetrics` | PS01, `974ecb20` | Review and narrow/remove in PS07 | Retain only if the test-only interface remains deterministic, has no ordinary-compilation output and is used by LA05 measurement |
| `scripts/measure_placement_checking.py`, its focused tests, Make target and `PLACEMENT_CHECKING_PERFORMANCE.md` | PS01, `974ecb20` | Retain through LA05 | Reproducible reports, ignored generated artifacts and maintained witness identifiers |
| `FixedPointDigest`, `SolverObservation` and `check_with_round_oracle` | PS02, `8957e382` | Remove or keep test-only in PS07 | Retain only if they materially protect solver equivalence without exposing authority or adding material suite cost |
| `observe_round_solver` and independent stable replay | PS02, `8957e382` | Reconcile in PS07 | Retain as independent specification coverage only if bounded ordinary tests remain fast; never compile into production |
| `tests/equivalence.rs` generated solver-equivalence cases | PS02, `8957e382` | Prefer permanent bounded tests | Fixed sizes, deterministic inputs, genuine selected graphs and low ordinary-suite cost |
| `TokenLayout`, `BitMatrix` and compact `State` | PS03, `24029bcc` | Retain | Production representation; deterministic layout, canonical final-word masking, checked dimensions and semantic-only callers |
| Immutable relation indices | PS04 checkpoint, `c3978118`; not introduced | Skipped | No relation had a material measured cost after compact state; direct target/draft queries remain authoritative |
| Deterministic worklist | PS05 checkpoint, `c3978118`; not introduced | Skipped | Accepted performance already met; deterministic-round production and its living contract remain unchanged |

Update this ledger as commits introduce concrete symbols. Record the introducing
task and commit, removal task and final disposition. New discoveries that do not
block this roadmap belong in
`docs/roadmaps/PLACEMENT_CHECKING_SCALABILITY_DISCOVERIES.md`, created only when
there is an actionable finding.

## Ordering and dependencies

PS01 is a hard diagnosis gate. PS02 establishes independent equivalence before
production representation changes. PS03 changes representation while retaining
the current schedule, isolating semantic and performance effects. PS04 admitted
no indices because none remained material. PS05 retained deterministic rounds
because the schedule checkpoint did not justify changing convergence order.
PS06 applies the same-host acceptance protocol to both discoveries. PS07 reviews
all commits and temporary artifacts together before clearing the LA05 dependency.

PS02 may prepare bounded oracle cases while PS01 measurements run, but no
production solver change starts before the diagnosis is a go. Later tasks are
sequential because each checkpoint determines whether the next complexity has a
demonstrated consumer.
