# Dead Normalized Path-Activation Cleanup Roadmap

Status: in progress; NAC0 through NAC3 are complete and NAC4 is next.

This roadmap implements FMM-13 as one exact, target-independent final-MIR
cleanup. It removes a `NormalizedPathActivation` declaration only when every
remaining reference belongs to a complete deletable condition-carrier
protocol. The durable result is a shared read-only analysis, a narrow
final-stage storage-cleanup capability, and an independently selectable pass
which removes dead activation loads, stores, lifetime markers, load-result
values, and frame homes without becoming general dead-store elimination.

The optimization is intentionally narrow. Its expected direct payoff is
modest stack traffic, MIR, assembly, and frame-size reduction in control-flow
and cleanup-heavy code. Its larger architectural value is establishing the
first reviewed final-stage storage-deletion boundary without introducing
general alias, effect, escape, or ownership analysis prematurely.

## Dependencies

- The completed
  [proof-provenance normalization](../archive/PROOF_PROVENANCE_NORMALIZATION_ROADMAP.md)
  supplies the proof-rich/final seal transition and normalized verification.
- The frozen
  [normalization-stable path-activation provenance design](../archive/NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_DESIGN_PROPOSAL.md)
  and completed
  [roadmap](../archive/NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_ROADMAP.md)
  supply the final-only `NormalizedPathActivation` role, its structural
  contract, current CFG mutation guard, backend equivalence, and test matrix.
- The completed
  [dense callable-local MIR identity rewriting](../archive/DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md)
  supplies sparse storage/value/block edits, exhaustive identity traversal,
  deterministic dense commit, and structured stale-plan failures.
- The completed
  [selectable final-MIR pipeline](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
  supplies pass registration, exact schedules, exclusions, metrics,
  checkpoints, and changed-result reverification.
- The completed
  [post-proof CFG canonicalization](../archive/POST_PROOF_CFG_CANONICALIZATION_ROADMAP.md)
  supplies final CFG facts and the empty-block and merge consumers which may
  benefit when carrier instructions disappear.
- The completed
  [local redundancy measurement](../archive/LOCAL_MIR_REDUNDANCY_MEASUREMENT_ROADMAP.md)
  supplies the reviewed corpus and stable human/JSON opportunity projections.

## Scope and invariants

- Analyze only source-free boolean storage whose exact snapshot kind is
  `NormalizedPathActivation`; `PathCondition`, `ScalarSpill`, user locals,
  checked-protocol carriers, statics, parameters, return storage, and every
  other storage kind are barriers.
- Define a removable protocol from exhaustive same-snapshot storage and value
  uses. Deletable references are declaration ownership, `StorageLive` and
  `StorageDead`, exact base-place unauthorized ordinary stores, and exact
  base-place ordinary loads whose result value has no semantic uses.
- Treat attachments, projected or alias places, authorized writes, material
  load results, checked protocols, proof metadata, calls, ownership/lifecycle
  operations other than storage lifetime markers, I/O, unknown executable
  roles, malformed identities, and stale observations as barriers.
- Remove a candidate only as one complete transaction: all of its exact load
  assignments and unused result declarations, all exact stores and lifetime
  markers, and the storage declaration. Never leave a dangling reference or
  partially deleted carrier protocol.
- Preserve evaluation count and order by retaining every producer of a stored
  value. Removing the non-failing local boolean store does not authorize
  removal, movement, or recomputation of its source expression.
- Do not chase newly dead producer values. Existing or later dead-pure value
  cleanup owns that concern; this pass deletes only the unused values defined
  by removable carrier loads.
- Keep analysis target-independent. Backend frame size and instruction
  reduction are observations, not safety evidence.
- Build plans from one immutable verified final-MIR seal, retain exact expected
  declarations/instructions/use sites, revalidate the whole callable plan
  before mutation, commit atomically, and reverify every changed result.
- Add a dedicated final storage-cleanup capability. Do not expose raw mutable
  final MIR or weaken the CFG capability's invariant that it cannot create,
  delete, or reclassify storage.
- Permit that capability to delete only certified normalized activations and
  their owned protocol instructions/values. Creation, replacement,
  reclassification, mutation of surviving declarations, or deletion of any
  other storage remains unsupported.
- Apply all candidates in deterministic callable, storage, block, and
  instruction order, batch block rewrites to avoid shifting analyzed indices,
  and rely on dense commit for deterministic surviving identity remapping.
- Place one default occurrence after post-proof unreachable block/value
  elimination and before empty-block forwarding and basic-block merging, so
  unreachable uses disappear first and newly empty blocks can still be
  canonicalized. Whole-world definition retention remains last.
- Keep mandatory normalization representation-only and keep the `none`
  profile unoptimized. The cleanup remains independently selectable and may be
  excluded without disabling normalization or other final passes.
- Preserve diagnostics, failure and evaluation behavior, proof-consumption
  authority, ownership and destruction order, runtime traces, ABI, native
  output, deterministic dumps/reports, and all live activation behavior.
- General scalar dead-store elimination, store-to-load forwarding, redundant
  load elimination, lifetime shortening, storage coalescing, producer DCE,
  alias/effect inference, and changes to the language or runtime are non-goals.
- Keep the semantic analysis, final-stage capability, pass plan/rewrite, and
  measurement projection behind concise owner-oriented facades. Do not add a
  generalized provenance or memory-optimization framework for this one role.

## Progress

- [x] NAC0 — Establish exact dead-carrier analysis and evidence
- [x] NAC1 — Add narrow final-stage storage deletion authority
- [x] NAC2 — Implement the selectable cleanup pass
- [x] NAC3 — Activate and compose cleanup in the final pipeline
- [ ] NAC4 — Prove end-to-end value and observable equivalence
- [ ] NAC5 — Harden the boundary and close the roadmap

## PR-sized implementation sequence

### NAC0 — Establish exact dead-carrier analysis and evidence

**Purpose:** Define one auditable candidate boundary and establish its corpus
incidence before any production mutation depends on it.

- [x] Add one read-only callable analysis over verified MIR which selects only
  `NormalizedPathActivation` declarations and consumes the existing exhaustive
  storage-use census plus value-use index.
- [x] Classify exact base loads, exact unauthorized base stores, lifetime
  markers, declarations, and every barrier role without inferring from names,
  spans, boolean type, CFG shape, or lowering history.
- [x] Require each removable load to be an exact ordinary load assignment with
  the expected boolean result declaration and no semantic result uses.
- [x] Produce an immutable deterministic candidate containing the exact
  declaration, removable instruction sites, removable load-result values, and
  stable removal bounds; retain structured blockers for non-candidates.
- [x] Extend the local redundancy observation and measurement projections with
  dead normalized path activations, including stable human/JSON fields,
  aggregate counts, bounded examples, blockers, and saturation behavior.
- [x] Record the frozen corpus baseline in living measurement documentation
  without rewriting the archived historical study.

**Tests:** Complete storage-use-role and place-shape matrix; zero, one, and
multiple exact loads/stores/lifetime sites; dead and materially used load
results; wrong kinds/types/sources; attachments and every protected role;
malformed and stale identities; deterministic candidate order; measurement
model, aggregation, rendering, and real-driver projections; `make
mir-redundancy-measure`, `make mir-measure-test`, `make compiler-test`, and
`make static-check`.

**Exit criteria:** The observer and measurement tool agree on one exact
candidate boundary, the current corpus incidence and possible removal bounds
are recorded, and no executable MIR behavior has changed.

### NAC1 — Add narrow final-stage storage deletion authority

**Purpose:** Make complete certified carrier deletion possible without
weakening the existing final CFG-only mutation boundary.

- [x] Add a private final-stage cleanup capability which consumes a prepared
  exact carrier-deletion plan and exposes no raw storage or instruction edits
  to optimization implementations.
- [x] Validate that every removed storage is a live
  `NormalizedPathActivation`, every removed instruction/value belongs to its
  certified protocol, and every expected site still matches before the first
  mutation.
- [x] Remove planned instructions once per block, then remove their load-result
  values and storage declarations through the existing sparse edit owner so
  dense commit remaps surviving identities once.
- [x] Add a capability invariant which rejects storage creation,
  reclassification, surviving-declaration mutation, deletion of unrelated
  storage, unplanned instruction/value changes, and partial protocol deletion.
- [x] Preserve consumed-proof authority through invalidation and require the
  ordinary normalized verifier and fresh reachability analysis to reseal every
  changed transaction.
- [x] Keep `MirFinalCfgEdit` and its exact storage-declaration invariant
  unchanged.

**Tests:** Successful single/multiple carrier deletion; candidates sharing a
block; dense storage/value identity remapping; exact change summaries; stale
declaration/instruction/use plans; attempted deletion of every excluded kind;
partial and extra mutations; atomic multi-callable rollback; normalized
verification and reachability resealing; compile-time API separation between
CFG and storage-cleanup capabilities; `make compiler-test` and `make
static-check`.

**Exit criteria:** A final pass can delete only an exact certified normalized
activation protocol, all other storage mutations fail before publication, and
existing final CFG passes retain their narrower authority unchanged.

### NAC2 — Implement the selectable cleanup pass

**Purpose:** Materialize the shared analysis through the narrow capability
while leaving normal compilation unchanged until focused behavior is proven.

- [x] Add a proof-normalized final-MIR pass with a unique private identity,
  stable name and description, private plan/rewrite ownership, and registry
  entry outside the default schedule.
- [x] Prepare all callable candidates from one verified seal, revalidate every
  expected declaration, instruction, value, and use decision, and apply the
  complete program transaction through the dedicated cleanup capability.
- [x] Batch overlapping block edits and storage/value removals without repeated
  whole-callable scans, mutation-time rediscovery, or index-shift dependence.
- [x] Report processed/changed callables, inspected/removable/protected
  carriers, removed storages, loads, stores, lifetime markers and values, and
  maximum protocol size in stable order.
- [x] Return unchanged with the existing seal when no candidate exists. A
  repeated occurrence over cleaned MIR must be a deterministic no-op.
- [x] Keep the production pass and measurement observer on the same semantic
  analysis so blocker and candidate decisions cannot drift.

**Tests:** Exact opt-in schedules; every removable protocol shape; mixed live
and dead activations; multiple candidates and blocks; every protected-use
rejection; source-producer preservation; exact spans and surviving
instructions; stale-plan failures; exact commit statistics and metrics;
unchanged seal reuse; changed-result verification; repetition and dump
determinism; `make compiler-test` and `make static-check`.

**Exit criteria:** Explicit selection safely removes every analyzed protocol,
leaves all blocked and unrelated storage byte-for-byte unchanged, reports exact
deterministic outcomes, and verifies after each changed occurrence.

### NAC3 — Activate and compose cleanup in the final pipeline

**Purpose:** Enable the proven pass where unreachable uses have disappeared and
its instruction deletion can expose additional CFG canonicalization.

- [x] Insert exactly one default-profile occurrence after
  `post-proof-unreachable-block-elimination` and before
  `post-proof-empty-block-forwarding`, basic-block merging, and final
  whole-world retention.
- [x] Update default/all-disabled schedules, exclusions, pass discovery,
  known-name diagnostics, checkpoint numbering, schedule fingerprints,
  reporting order, and stable pass lists.
- [x] Add constructed-MIR composition cases where unreachable deletion exposes
  a dead carrier and carrier cleanup in turn exposes an empty or mergeable
  block.
- [x] Prove that disabling unreachable deletion can conservatively retain a
  carrier, disabling cleanup retains only the removable protocol, and
  disabling later CFG passes does not undo carrier deletion.
- [x] Confirm that the `none` profile still performs mandatory normalization
  but never performs carrier cleanup.
- [x] Update living phase, driver/selection, reporting, testing, and debugging
  documentation with current scheduling, safety boundaries, metrics, and
  exclusions.

**Tests:** Registry and policy tests; exact default and exclusion schedules;
driver listing and diagnostics; constructed final-MIR composition; checkpoint,
metric, and dump fingerprints across repeated runs; focused golden filter;
`make compiler-test`, `make golden-runner-test`, and `make static-check`.

**Exit criteria:** Normal compilation runs cleanup once at the documented
position, every profile and exclusion composes deterministically, and later CFG
passes consume any newly exposed structure without broadening storage
authority.

### NAC4 — Prove end-to-end value and observable equivalence

**Purpose:** Demonstrate what the pass removes in real source programs while
pinning all semantic and target-facing invariants.

- [ ] Add focused source fixtures with removable and live activations across
  conditionals, short-circuit selection, loops, optional/array/shared cleanup,
  methods, initializers, destructors, static lifecycle, selected/skipped
  failures, and runtime-trace attribution.
- [ ] Add golden variants for `default`, `none`, cleanup disabled, unreachable
  deletion disabled, later CFG cleanup disabled, reachability disabled, and all
  passes disabled; require exact stdout/stderr/status and runtime-trace
  equivalence.
- [ ] Pin final-MIR and structured-report differences to the intended complete
  protocol deletions and stable metrics, with no partial carrier remnants.
- [ ] Add backend tests proving removed candidates no longer receive frame
  slots or load/store instructions, live activations remain identical, and
  assembly remains deterministic and accepted by the system assembler.
- [ ] Rerun the reviewed redundancy corpus after default activation, record
  before/after candidate and entity evidence in living measurement
  documentation, and keep the archived study historical.
- [ ] Update FMM-13 in the optimization catalog from planned to implemented,
  stating measured value without generalizing to FMM-03 through FMM-05.

**Tests:** Focused debug and release goldens; native backend and frame-layout
tests; full source/profile matrix; `make mir-redundancy-measure`, `make
compiler-test`, relevant golden filters, `make golden-determinism-test`, and
`make golden-release-test`.

**Exit criteria:** Real programs demonstrate complete removable protocols and
target reductions, optimized and excluded variants are observably equivalent,
live activations are untouched, and the documented corpus evidence matches
pass observations.

### NAC5 — Harden the boundary and close the roadmap

**Purpose:** Audit completeness, scaling, and ownership after activation, then
publish the implementation as stable current behavior.

- [ ] Add generated large-callable tests with many interleaved dead/live
  activations, loads, stores, lifetime markers, and blockers; require bounded
  iterative analysis and batched rewrite behavior.
- [ ] Audit analysis, capability, rewriting, verification, measurement, and
  reporting for repeated scans, quadratic block edits, recursion,
  nondeterministic maps, duplicated role semantics, stale snapshots, large-file
  ownership, and unclear facades; resolve small roadmap-local issues and record
  unrelated findings separately.
- [ ] Confirm the pass cannot delete an activation through material use,
  aliasing, attachment, authorization, malformed topology, or an out-of-date
  census, and that no broader memory optimization entered scope.
- [ ] Confirm no task codes or rollout language remain in living code, tests,
  compiler/development documentation, pass names, metrics, or diagnostics.
- [ ] Run the full repository gate from an artifact-free snapshot, the
  supported-toolchain and long robustness gates, full golden determinism, and
  release goldens.
- [ ] Mark every completed item, set the roadmap status to complete, move it to
  `docs/archive/`, update active/archive indexes and incoming links, and leave
  only current behavior in living documentation.

**Tests:** Focused compiler and measurement tests; `make check`; `make
msrv-check`; `make robustness-long`; `make golden-determinism-test`; `make
golden-release-test`; and `make docs-check` after archival.

**Exit criteria:** Every safe candidate in the deliberately narrow domain is
removed, protected and malformed shapes fail closed, long candidate sets have
bounded deterministic cost, all repository gates pass, living documentation
is current, actionable unrelated findings are indexed separately, and the
completed roadmap is archived.

## Ordering and dependencies

NAC0 freezes the semantic candidate boundary and measures its incidence before
mutation. NAC1 then introduces only the final-stage authority that boundary
requires. NAC2 consumes both pieces in an opt-in pass, keeping default output
unchanged while transaction and metric behavior are proven. NAC3 activates the
pass after unreachable deletion and before later CFG canonicalization. NAC4
adds broad source, target, and corpus evidence only after the schedule is
stable. NAC5 closes scaling and maintainability gaps before archival.

The exact normalized activation role, dense rewrite owner, and final resealing
path are prerequisites for all tasks and are already implemented. Measurement
projection work may proceed alongside focused analysis tests in NAC0, but no
production mutation should precede the shared analysis. Source fixtures may be
prepared alongside NAC2, but default activation and living behavior
documentation wait for the pass and capability contracts. General scalar
storage optimization, effect/alias analysis, producer DCE, and lifetime
shortening require separate reviewed designs and cannot expand this roadmap.
