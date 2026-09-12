# MIR Structural CFG Analysis Roadmap

Status: planned; C01 is next.

This roadmap implements
[cleanup finding A18](CODEBASE_CLEANUP_AUDIT.md#a18--reuse-structural-cfg-and-dominance-queries)
by giving callable-local control-flow structure one neutral MIR owner. The
result will let verification, rewrite planning, and optimization consume the
same deterministic edge and predecessor facts, then answer repeated dominance
queries from one per-callable analysis instead of traversing the graph twice
for every query.

## Current state and selected design

The current graph knowledge is split across responsibilities:

- [`mir/verify/checked_scalar.rs`](../../crates/skald-compiler/src/mir/verify/checked_scalar.rs)
  exports a predecessor map through the verifier facade and implements
  dominance as two reachability searches per query.
- Logical verification, path-condition verification, shift verification, and
  logical-topology optimization build additional predecessor maps.
- Checked-integer and checked-floating-point topology reuse the verifier's
  map, while their rewrite helpers derive another map from rewrite CFG facts.
- [`mir/rewrite/cfg.rs`](../../crates/skald-compiler/src/mir/rewrite/cfg.rs)
  already preserves successor-edge occurrences, but its snapshot also
  validates terminators, local identities, values, and protected roots. That
  stronger contract cannot be a prerequisite for initial MIR verification.

The neutral owner will be a private `mir::analysis` facade with a
`MirCfgTopology` query object and an occurrence-preserving `MirCfgEdge`. A
topology is a short-lived immutable callable snapshot. It records blocks in
input snapshot order and edges in block order followed by
`MirTerminator::successors` order. Its block query exposes two deliberately
different views:

- predecessor blocks as a deterministic set for shape checks; and
- predecessor and successor edges as ordered occurrences, including two
  distinct edges when both arms of a conditional select the same block.

Construction is tolerant. A missing terminator contributes no outgoing edge;
foreign, unknown, duplicate, and misindexed block identities cannot panic the
analysis. Raw edge occurrences remain inspectable, while traversal and
block-oriented queries enter only unambiguous local declarations. Existing
verifier owners continue to diagnose malformed declarations, missing
terminators, and invalid targets in their current order. Rewrite planning
continues to reject them through `MirRewriteError` before applying edits.

`MirLocalCfgFacts` will build its structural edge and reachability portions
through this neutral owner while retaining rewrite-only instruction, value,
terminator-kind, protected-root, and permanent-attachment facts. The entry is
the sole root for ordinary reachability and dominance. Protected roots remain
a rewrite policy layered over topology; they do not make an entry-unreachable
block dominated by the entry.

`MirDominators` will be derived once from `MirCfgTopology` for each analysis
scope. It will use deterministic internal block ordinals and predecessor
intersection to store dominator membership. A known block dominates itself,
including in a disconnected component. Other dominance is defined only for
entry-reachable targets. Unknown, foreign, ambiguous, or misindexed IDs return
`false`. This preserves useful current behavior for valid and disconnected
MIR while making malformed-ID queries fail closed.

## Scope and invariants

- Establish one crate-private MIR owner for callable-local edges,
  predecessor sets, reachability, and dominance.
- Preserve terminator successor order and edge occurrence identity.
- Preserve MIR verification diagnostics, their ordering, and the distinction
  between proof-rich and normalized verification.
- Preserve rewrite protection, value-use, root-reachability, and atomic edit
  contracts.
- Build facts from both immutable `MirDefinitionRef` bodies and the ordered
  sparse snapshots used by callable edits without introducing a public graph
  trait.
- Keep analysis lifetime local to one immutable callable or edit snapshot;
  callers recompute after mutation.
- Keep the `mir` facade as the only path used by later compiler phases. The
  analysis module itself remains private.
- Do not add cross-pass caches, revision keys, invalidation summaries, loop
  trees, liveness, post-dominance, or a general-purpose graph framework.
  Cross-snapshot reuse remains the separate A19 design problem.
- Do not change MIR format, block IDs, dumps, pass scheduling, optimization
  eligibility, or source-visible behavior.

## Progress

- [ ] C01 — Establish neutral structural CFG facts
- [ ] C02 — Migrate predecessor and reachability consumers
- [ ] C03 — Compute and reuse callable-local dominance

## PR-sized implementation sequence

### C01 — Establish neutral structural CFG facts

**Purpose:** Settle the ownership and malformed-input contract before any
verifier or pass begins depending on the new analysis.

- [ ] Add a concise `mir::analysis` facade with responsibility-specific CFG
  implementation and colocated tests.
- [ ] Add `MirCfgTopology`, `MirCfgBlockTopology`, and `MirCfgEdge` with
  explicit block-set and edge-occurrence APIs. Keep lookup deterministic and
  return `None` for block-oriented queries whose identity is not one
  unambiguous local declaration.
- [ ] Provide one definition constructor and one MIR-private ordered-snapshot
  constructor so rewrite edits can reuse the implementation without exposing
  their sparse representation or adding a graph trait.
- [ ] Centralize entry-rooted and caller-rooted reachability over known local
  nodes. Invalid targets remain in raw edges but never become traversal nodes.
- [ ] Move rewrite CFG edge construction and closure traversal onto the
  neutral implementation. Retain all strict rewrite validation and
  rewrite-only facts in `mir::rewrite`.
- [ ] Remove the rewrite-owned edge type rather than maintaining aliases or
  two structural edge vocabularies.
- [ ] Document the neutral analysis owner, tolerant verifier input, strict
  rewrite layer, edge identity, root policy, and snapshot lifetime in
  [`PHASES_AND_IR.md`](../compiler/PHASES_AND_IR.md).

**Tests:** Add focused `mir::analysis::cfg` tests for a straight-line graph,
branch, loop, self-edge, disconnected block, and a conditional whose two edge
occurrences have one predecessor block. Cover missing terminators, foreign and
unknown targets, duplicate or misindexed declarations, invalid entry IDs, and
deterministic repeated construction without panics. Extend rewrite CFG tests
to prove unchanged protected-root closure, sparse block order, invalid-ID
errors, and edge ordering. Run `cargo test --locked -p skald-compiler
mir::analysis::cfg`, `cargo test --locked -p skald-compiler mir::rewrite::cfg`,
`make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** The neutral module is the sole shared implementation of
callable-local edge occurrence indexing and structural root closure for this
boundary. Rewrite facts use it without weakening validation, verifier code
does not depend on a rewrite snapshot, all existing rewrite behavior is
preserved, and the ownership contract is documented.

### C02 — Migrate predecessor and reachability consumers

**Purpose:** Replace the repeated predecessor builders only after their shared
set-versus-edge semantics are explicit and tested.

- [ ] Construct one `MirCfgTopology` at the start of each definition's
  verification and thread an immutable reference to checked integer division,
  checked primitive casts, checked shifts, logical expressions, and path
  conditions.
- [ ] Replace the predecessor export in `mir::verify::checked_scalar` and the
  crate-level `checked_scalar_predecessors` path with the neutral MIR facade.
  Keep carrier load/write helpers owned by checked-scalar verification.
- [ ] Migrate checked-integer topology, checked floating-to-integer topology,
  logical-topology optimization, and checked-scalar rewrite topology to the
  neutral predecessor-set query.
- [ ] Delete every callable-local MIR predecessor-map builder and the
  associated `HashMap`/`HashSet` plumbing. Component-graph predecessors in
  static-lifecycle analysis remain separate because they describe a different
  graph.
- [ ] Preserve exact-predecessor checks as set comparisons. Do not infer edge
  count from the unique predecessor set when a transformation needs edge
  occurrence identity.
- [ ] Keep verifier error collection and ordering unchanged on malformed MIR;
  the topology supplies facts and never emits diagnostics.

**Tests:** Run the focused verifier suites for logical expressions, path
conditions, shifts, integer division, primitive casts, and malformed MIR, plus
the logical, checked-integer, and checked floating-to-integer optimization
tests. Add direct regressions where one predecessor contributes duplicate
edges and where an invalid target coexists with a valid shape check. Run
`cargo test --locked -p skald-compiler mir::verify`, `cargo test --locked -p
skald-compiler passes::pipeline::optimizations`, `make check`, `make
msrv-check`, and `git diff --check`.

**Exit criteria:** Every callable-local predecessor-set consumer reads the
neutral topology built once for its analysis scope, transformations that need
edges use occurrence APIs, no predecessor helper remains under verification
or optimization, and diagnostic and optimization regression suites are
unchanged except for the new edge-identity coverage.

### C03 — Compute and reuse callable-local dominance

**Purpose:** Replace repeated reachability searches with one clear,
entry-rooted dominance result and complete the cleanup finding.

- [ ] Add `MirDominators` under the neutral CFG analysis owner. Compute it once
  with deterministic predecessor intersections over entry-reachable known
  blocks, using internal ordinals rather than trusting `BlockId::index()`.
- [ ] Freeze tests for known reflexivity, disconnected non-dominance, loops,
  self-edges, diamonds, multiple paths, duplicate edges, and fail-closed
  malformed-ID queries.
- [ ] Build one dominance result per definition verification and share it
  across primitive-alias, checked-integer-division, and primitive-cast checks.
- [ ] Thread one dominance result through local constant-carrier analysis and
  scalar-spill redundancy analysis instead of rebuilding or searching from
  individual site predicates.
- [ ] Remove `checked_scalar_dominates`, its reachability searches, and all
  verifier-owned graph exports. Keep same-block instruction ordering in its
  existing instruction-site owners.
- [ ] Confirm no caller stores facts across a MIR mutation. Record any need
  for cross-pass or cross-mutation reuse under A19 rather than expanding this
  roadmap.
- [ ] Mark A18 complete with delivered validation, update A19's prerequisite,
  archive this roadmap, and repair the active and archive indexes.

**Tests:** Add focused dominance unit tests and retain the verifier and pass
regressions that exercise instruction-site ordering across blocks. Test a
protected but entry-unreachable rewrite root to prove it does not affect
dominance. Run `cargo test --locked -p skald-compiler mir::analysis::cfg`,
`cargo test --locked -p skald-compiler mir::verify`, the local-constant and
scalar-spill pass suites, `make check`, `make msrv-check`, and `git diff
--check`.

**Exit criteria:** Repeated dominance queries are constant-time membership
lookups over one result per callable analysis scope; no dominance query
performs graph traversal; verifier and pass consumers depend only on the
neutral MIR facade; A18 is complete; and no analysis survives mutation.

## Ordering and dependencies

C01 comes first because edge identity, malformed-input handling, root policy,
and rewrite layering determine every later API. C02 migrates simpler
predecessor consumers and exposes contract mistakes before dominance depends
on the topology. C03 then adds the derived analysis and changes its consumers
without mixing that algorithm review with the larger predecessor migration.

This roadmap is independent of unfinished frontend cleanup. It satisfies the
structural-analysis prerequisite for A19, but it deliberately provides no
cross-pass cache or invalidation contract. New analysis opportunities found
during implementation should be recorded in a separately indexed discovery
document unless they are a small correction required by these exit criteria.
