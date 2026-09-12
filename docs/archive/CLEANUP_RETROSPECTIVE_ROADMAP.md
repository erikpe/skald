# Cleanup Architecture Retrospective Roadmap

Status: complete (2026-09-12); R01–R04 are complete. Final acceptance,
readiness and clean-snapshot validation are recorded in the
[review](CLEANUP_RETROSPECTIVE_REVIEW.md#r04--final-acceptance-and-readiness).
Publication ownership remains in its separately indexed implementation roadmap.

Created: 2026-09-12.

The first cleanup batches were implemented directly from the
[cleanup audit](../roadmaps/CODEBASE_CLEANUP_AUDIT.md), with delivery records and regression
checks but without separate design and implementation roadmaps. This bounded
retrospective will establish which architectural outcomes are actually complete,
which were deliberately narrowed, and what must precede further dependent work.
It preserves useful delivered changes while making their acceptance reviewable.

## Scope and invariants

- Review A08–A12 against their original goals, current implementation, living
  contracts, and named tests. Inspect A07, A34, and A43 only as supporting
  ownership, measurement, and dependency-check foundations.
- Produce evidence and design decisions before proposing implementation that
  depends on them. Test success establishes observed behavior, not architectural
  sufficiency or a measured speedup.
- Preserve identity allocation, diagnostic ownership and order, deterministic
  dumps, declaration-wide checking, specialization rejection semantics, and
  ordinary declarations surviving errors. Preserve evaluation/cleanup order,
  lifecycle behavior, ABI, and lower-phase consumption of identities and plans.
- Keep structural language-item validators separate and authoritative type
  checking in its current phase. Do not add another specialization engine.
- Exclude a new repository-wide audit, redesign of IRs, per-body incremental
  discovery, indexed binding implementation, and unrelated bug/tooling fixes.
  Do not recreate historical roadmaps for the early narrow fixes.
- Any necessary production redesign receives its own design and PR-sized
  roadmap. This roadmap permits focused regression tests and documentation
  corrections; it does not absorb open-ended architectural implementation.

## Progress

- [x] R01 — Establish outcome and evidence traceability
- [x] R02 — Decide the candidate-publication acceptance boundary
- [x] R03 — Close targeted verification gaps
- [x] R04 — Reconcile statuses and release dependent work

## PR-sized implementation sequence

### R01 — Establish outcome and evidence traceability

**Purpose:** distinguish delivered behavior from intended architecture before
deciding whether further implementation is needed.

- [x] Record the reviewed revision and working-tree state. Inspect relevant
  history to separate pre-existing behavior from cleanup changes.
- [x] Create a companion `CLEANUP_RETROSPECTIVE_REVIEW.md` in this directory and
  index it as in progress. For each A08–A12 outcome, record the original promise,
  current owner/product, source links, exact test names, evidence limitations,
  residual risk, and proposed disposition: fulfilled, deliberately narrowed,
  or outstanding. Record command results separately from historical reports.
- [x] Review A08's neutral capability ownership, recursive lifecycle facts,
  ordinary/generic parity, and HIR plan boundary. Review the actual scope and
  limitations of A43's dependency checks; absence of a forbidden import alone
  does not establish correct semantic ownership.
- [x] Review A09's collection/body/publication products and remaining ordering
  coupling. Review A10's delta/termination invariant, provisional-state
  isolation, counters, and no-range fast path. Identify whether evidence is
  structural, behavioral, or a comparable before/after performance measurement.
- [x] Review A11's known/unknown/invalid distinctions and diagnostic consumers;
  explicitly trace indexed binding lookup to A25. Review A12's canonical catalog,
  provenance ordering, validator separation, and shared body environment.
- [x] Correct A12's claim about replacing repeated graph scans: collection
  implementation is shared, but the current collector still scans separately
  for different requirement families. Do not infer a need to fuse scans.

**Tests:** inspect and map existing capability parity, publication, range,
operator, module graph, determinism, and phase-boundary tests to claims. Run
`make docs-check` and `git diff --check` for this documentation task; do not
claim unexecuted suites passed on the reviewed revision.

**Exit criteria:** every original A08–A12 outcome has a disposition proposal
and evidence or an explicit gap; architectural and performance claims are
separated from regression evidence. No item is accepted solely because its
audit inventory says Complete.

### R02 — Decide the candidate-publication acceptance boundary

**Purpose:** resolve the principal remaining design question before changing
publication representation or allowing dependent changes to assume it is settled.

- [x] In the review record, inventory every `ResolvedProgram` field and related
  specialization state. Identify ordinary/candidate ownership, dependencies,
  and required preservation, restoration, clearing, or failure marking for each
  rejection path. Include bodies, hierarchy, dispatch, interface dependencies,
  interned identities, and retained language-item references where applicable.
- [x] Trace successful publication, class rejection, interface rejection, and
  combined/dependent rejection. Determine which independent valid products
  survive and which rejection effects are intentional existing policy.
- [x] Evaluate at least two concrete alternatives: retain centralized rollback
  with an explicit dependency contract, or select coherent owned ordinary and
  candidate products. Explain extension cost when a new dependent table is
  added, invalid intermediate states, cloning cost, and migration risk.
- [x] Record the chosen endpoint and rationale. If centralized rollback is
  accepted, state how its remaining manual field maintenance satisfies a
  deliberately narrowed goal. If stronger ownership is necessary, define its
  product boundary, invariants, migration steps, and acceptance tests in a
  separate focused implementation roadmap before scheduling its consumers.

**Tests:** map the field/rejection matrix to exact existing assertions. Inspect
failed dependent specializations, ordinary declaration restoration, dispatch
clearing, independent class/interface survival, and source-order determinism.
Validate documentation links and diff hygiene.

**Exit criteria:** publication has a recorded design decision; every dependent
product has a justified rejection disposition. Any remaining hazard has an
explicit owner and blocking or nonblocking classification, rather than an
unqualified Complete label.

### R03 — Close targeted verification gaps

**Purpose:** verify the accepted contracts where the retrospective found missing
evidence, without creating tests that merely mirror implementation details.

- [x] Select missing cases from the review matrix and add the narrowest tests
  that observe a meaningful invariant. Reuse existing adequate coverage.
- [x] Cover any uncovered publication failure combination or stale-reference
  risk identified in R02. Verify diagnostic owner/order and deterministic
  products where selection or rejection interacts across modules.
- [x] Close demonstrated gaps in capability parity, range probe isolation and
  termination, provisional query consumers, or language-item provenance only
  when R01 identifies them. Avoid duplicating every scenario across phases.
- [x] Record current focused test results and link new regressions in the review.
  If a test exposes a production defect, retain the reproducer, record a blocker,
  and route the fix through a separately scoped task; do not weaken expectations
  or expand this task into an unreviewed redesign.

**Tests:** follow [test ownership](../development/TESTING.md). Run focused suites
for changed tests, then `make check` and `make msrv-check` for Rust changes.
Use [cleanup measurements](../development/CLEANUP_MEASUREMENTS.md) if making
performance claims; ordinary correctness tests must not use timing thresholds.

**Exit criteria:** accepted contracts have adequate named coverage and current
results. Any unresolved failure is documented and blocks the affected dependent
work. No production redesign is silently bundled into test hardening.

### R04 — Reconcile statuses and release dependent work

**Purpose:** leave an accurate architectural starting point and a finite set of
explicit prerequisites for the next cleanup work.

- [x] Finalize the review dispositions and reconcile the cleanup audit's summary,
  inventory, detailed entries, and sequencing. Preserve historical validation
  records while clearly distinguishing any new acceptance or narrowing.
- [x] Update living contracts only with accepted current behavior. Keep decision
  history in the review and planned implementations in their own roadmaps.
- [x] Put additional actionable findings in a separately indexed
  `CLEANUP_RETROSPECTIVE_DISCOVERIES.md` if needed. Each finding needs evidence,
  owner, priority, scope, and whether it blocks a named next change. Avoid an
  empty discoveries document or copying existing audit items into a new backlog.
- [x] Publish a readiness table for the next candidates: A25 and other resolver
  consumers, A17's capability work, A18 before A19, and the larger representation
  projects. State the prerequisite, disposition, and next design/task for each;
  do not imply independent MIR or narrow bug fixes require publication redesign.
- [x] Specify the future workflow: narrow fixes may use a bounded task with
  invariants and tests; cross-phase, representation, and substantial algorithm
  changes require a short design decision followed by PR-sized roadmap tasks.
- [x] Close and archive this roadmap and the review after acceptance. Keep
  actionable discoveries and prerequisite roadmaps indexed under active work,
  repair links, and update the cleanup audit's dependency statement.

**Tests:** run `make check` from an artifact-free snapshot or clean checkout for
closure, plus `make msrv-check` when Rust was changed during the retrospective.
Run documentation and diff checks after archival/link updates. Record revision,
commands, outcomes, and remaining limitations in the review.

**Exit criteria:** every reviewed outcome has a final disposition; no unresolved
correctness issue is represented as accepted. Further work can identify exactly
which contracts are safe to build on. Larger outstanding work may remain in its
own roadmap, with affected consumers explicitly blocked; its implementation is
not required to finish this bounded retrospective.

## Ordering and dependencies

R01 establishes evidence; R02 settles the publication contract; R03 verifies
the resulting obligations; R04 records acceptance and readiness. No task starts
by assuming that all previous Complete labels are either correct or wrong.

The [architecture](../compiler/README.md),
[phase contracts](../compiler/PHASES_AND_IR.md), and feature contracts linked
there remain authoritative. A07, A34, and A43 supply supporting evidence; they
are not reopened wholesale. Niflheim's separation of identity migration goals
and non-goals is useful precedent, but Skald's current products and contracts
determine the decisions here.

Do not begin further cleanup that depends on unsettled A08–A12 contracts until
its relevant acceptance decision is recorded. Independent narrow fixes and
design investigations may proceed. If the retrospective discovers a need for
a larger change, creating and linking that bounded follow-up is the stopping
point here, not permission to expand this roadmap indefinitely.
