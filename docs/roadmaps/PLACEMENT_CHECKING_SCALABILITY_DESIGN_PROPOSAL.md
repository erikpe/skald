# Placement Checking Scalability Design Proposal

Status: draft for review, 2026-09-20. Source assessment: `0ec22432`, after
complete private low-level lowering parity. Parent:
[Low-Level Compiler Architecture](LOW_LEVEL_COMPILER_ARCHITECTURE_DESIGN_PROPOSAL.md).
Promoted inputs: [complete lowering migration discoveries D01 and D02](COMPLETE_LOW_LEVEL_LOWERING_MIGRATION_DISCOVERIES.md).
If accepted, freeze this proposal and create one focused implementation roadmap
before starting LA05 architecture consolidation and adoption.

## Purpose and completion boundary

Make independent placement checking scale with the complete low-level programs
it now verifies. Recursive optional lifecycle and live array-element lowering
produce valid but comparatively large selected CFGs. The current checker proves
them correctly, but several tiny source fixtures take tens or hundreds of
seconds in an ordinary debug test run. That cost has increased `make check` from
about one minute to several minutes and would make the new pipeline unsuitable
as the production default.

This work keeps the complete semantic expansions and all existing end-to-end
witnesses. It changes the internal representation and convergence schedule of
the shared placement checker while preserving the accepted placement language,
independent authority, finite must-analysis, exact fixed point and deterministic
strict diagnostics.

Completion requires:

- phase measurements to confirm the cost distribution and record stable
  structural work metrics before optimization;
- the checker to compute the same greatest fixed point as the accepted
  deterministic-round analysis without trusting placement-producer facts;
- all existing accepted and rejected placement cases to retain their outcomes,
  first failure locations, reasons and source attribution;
- the six excessive native-pipeline tests to remain ordinary tests and become
  fast enough that `make check` returns to a practical feedback time;
- the shared algorithm to remain target-independent and suitable for future
  register placements and AArch64 target facts; and
- temporary profiling, comparison implementations and diagnostic output to be
  removed before the roadmap closes.

This proposal does not change lowering semantics, compact or hide lifecycle
CFGs, add opaque aggregate operations, weaken placement checks, introduce
register allocation, alter an ABI, or adopt the new pipeline in production.
General LIR optimization and CFG simplification remain separate work unless the
checker improvements fail their measured checkpoint.

## Evidence and present cost

The ordinary validation graph did not change during the lowering migration:
`make check` still runs `cargo test --locked --workspace`. A recent cached
workspace test ran 3,516 compiler tests in about 239 seconds. The following
tests account for the regression when measured individually on the same debug
build and host:

| Migration | Native-pipeline witness | Elapsed | Peak RSS |
| --- | --- | ---: | ---: |
| LM10 | Primitive and shared optionals | 47.7 s | 118 MiB |
| LM12 | Primitive-array allocation positions and length | 40.3 s | 225 MiB |
| LM13 | Nontrivial array-element lifecycle | 196.3 s | 892 MiB |
| LM14 | Indexed arrays and aliases | 88.9 s | 640 MiB |
| LM14 | Copied array slices | 91.3 s | 550 MiB |
| LM14 | Primitive slice assignment | 118.0 s | 766 MiB |

The empty-inline-array control took 0.85 seconds and 61 MiB. CPU time closely
matched elapsed time for every slow witness. Cargo runs the witnesses in
parallel during the workspace suite, so they contend for CPU and memory rather
than adding their isolated durations directly.

These timings are observational baselines, not portable test assertions. The
implementation roadmap must capture the command, repository identity, profile,
host identity, warm-up policy and repeated samples used for its decision.

## Root cause and optimization surface

Placement checking reconstructs all authority from a `PlacementDraft`, its
exact `VerifiedSelectedCallable` and immutable target facts. Structural and
legality validation precede content availability. The availability checker is
a forward must-analysis: each location contains the identities whose complete
bits are definitely present, joins intersect those sets, and states only lose
facts from an initial top state until convergence.

The current implementation is correct but has compounding costs:

1. A state is `Vec<BTreeSet<TransferValue>>`. Top materializes every token in
   every location as separately allocated tree nodes.
2. Every global round creates a fresh top state for every reachable block.
3. Every block and every outgoing edge clones a full state on every round,
   including blocks whose input did not change.
4. Equality compares whole maps and states after each round.
5. Writes, definition epochs, transfer-scratch expiry and resource kills scan
   broad location, token or storage collections repeatedly.
6. Alias and target facts are queried in inner transfer loops even when they are
   immutable for the complete check.

The finite bound `blocks * locations * tokens + 1` limits changing rounds, but
it is not a useful work bound for repeated full-CFG sweeps and tree-set clones.
Recursive lifecycle CFGs increase blocks, values and placement-created storage
together, while live array loads retain more identities across joins. D01 and
D02 therefore amplify the same checker behavior.

Instrumentation must verify this diagnosis before choosing the final data
structure. Required structural observations are reachable blocks and edge
occurrences, locations by kind, tokens, state bits, selected events, transfers,
worklist pops, successful fact removals and peak queued blocks. Required phase
timings distinguish planning, discovery lowering, executable lowering,
selection, placement production, placement checking, frame planning, physical
realization/checking, assembly publication and native execution.

Elapsed timing must remain outside correctness output and deterministic dumps.
Prefer an owner-private measurement entry or test-only observer over production
logging. No timing-dependent branch may affect acceptance or diagnostics.

## Accepted semantic contract

The optimized checker preserves the living
[placement checking contract](../compiler/PLACEMENT_CHECKING.md):

- drafts remain untrusted and only the checker constructs `CheckedPlacement`;
- the checker consumes no producer availability map, dominance claim or
  successful-check flag;
- the finite universe consists of exact locations and identities reconstructed
  from the selected snapshot, draft and target facts;
- state means definite complete-bit contents, with top used only as a
  convergence initializer;
- writes, clobbers, definitions, transfers, call slots, widths, aliases,
  preserved contents, scratch lifetimes and simultaneous edge rebinding retain
  their current meanings;
- entry retains its fixed seed as an additional predecessor across backedges;
- every reachable predecessor edge occurrence contributes to a join;
- states descend monotonically and acceptance requires the greatest fixed point;
- unreachable blocks contribute no availability authority, while their
  structural and static legality remains checked;
- strict replay after convergence checks every reachable use and selects the
  first failure in stable block, instruction/event and edge-occurrence order;
  and
- capacity or checker-invariant failure rejects structurally instead of timing
  out, falling back or accepting optimistically.

The current living contract additionally prescribes Jacobi-style rounds reading
the complete previous round. This proposal amends that implementation rule to
permit a deterministic worklist. The semantic fixed point and strict replay
order remain frozen. A test-only round oracle must demonstrate equivalence on
focused counterexamples and generated finite cases before the production solver
changes.

## Proposed convergence architecture

Use a deterministic successor worklist over block-entry states:

1. Compute reachability and immutable block/edge facts in stable selected order.
2. Initialize every reachable block entry to top and intersect the entry block
   with the fixed seed.
3. Queue the entry block. Process queued blocks in deterministic FIFO order,
   preserving stable successor occurrence order.
4. Interpret a block from its current entry state and compute each outgoing edge
   state with non-strict reads, as today.
5. Intersect that edge state into the successor entry. Queue the successor when
   it is reached for the first time or at least one fact is removed, unless it
   is already queued. First-reach scheduling covers empty-token lattices and an
   edge whose first propagated state is still top.
6. Intersect backedges into the entry state without replacing its fixed seed.
7. Stop when the queue is empty, then run the unchanged strict replay in stable
   selected order.

Starting at top and applying only intersections means each block-entry fact can
be removed at most once. A successor may be evaluated more than once, but only
after its input loses information. Track removals with checked arithmetic and
retain `blocks * locations * tokens` as a semantic termination bound. First
reach schedules each reachable block at most once; later queue additions require
a successful state change. Queue processing is therefore bounded by reachable
blocks plus successful fact-removal updates. Exceeding either bound is an
invariant failure.

This schedule computes the same greatest fixed point because transfer functions
remain deterministic and monotone over the finite descending lattice. Worklist
order can affect intermediate states but cannot affect the final solution.
Strict replay, rather than speculative convergence visits, remains the sole
owner of availability diagnostics, so first-error determinism does not depend on
the convergence schedule.

## Compact state and immutable indices

Represent each location's identity set with a compact finite bitset indexed by
the checker-owned, deterministically sorted token universe. A state remains a
location-indexed collection, but top initialization, cloning, intersection,
membership, equality and removal operate on machine words instead of tree nodes.
Mask unused bits in the final word so equality and fact-removal counts remain
canonical.

The representation is private to placement checking. `TransferValue`,
`Location`, public inspection and failure data remain unchanged. Conversion to
identity iterators is limited to operations that must capture compatible equal
contents; no bit index escapes as semantic identity.

Precompute immutable indices once per check where measurements justify them:

- location-to-index and token-to-bit mappings;
- the overlapping-location closure for each location;
- locations killed by each resource unit;
- ABI locations invalidated by calls;
- transfer-lifetime storage locations by transfer point; and
- representation-compatible token masks where repeated filtering is material.

Precomputation must derive solely from the same checked target and draft facts
used today. It grants no new authority and must not cache across snapshots or
targets. Avoid target-specific branches in the shared solver.

The implementation roadmap should introduce compact state and immutable indices
before or with the worklist, using measurements to keep PR boundaries
reviewable. If one change already meets the acceptance threshold, later
complexity needs separate evidence; do not add dormant optimization machinery.

## Correctness proof and comparison strategy

Retain a simple deterministic-round implementation as a test-only specification
oracle during the roadmap. It may use compact state, but it must not share the
production worklist control flow. Compare:

- converged block-entry states or a canonical digest;
- accepted versus rejected result;
- first strict failure location, reason, origin and structural detail; and
- capacity/invariant disposition for deliberately bounded synthetic inputs.

Focused cases must include loops, entry backedges, duplicate successor
occurrences, self-edges, simultaneous parameter swaps, definition epochs,
caller clobbers and results, preserved views, overlapping widths, transfer
scratch, unreachable blocks and partially preserved synthetic resources.

Add deterministic generated small graphs whose placements are checked by both
solvers. Bound generation by case count and graph size so it stays in ordinary
tests. This comparison is stronger than relying on native execution: native
runs sample paths, while the checker proves all reachable paths.

The round oracle and comparison instrumentation are roadmap artifacts. Remove
them at closure unless a compact, independent oracle has a documented permanent
role and acceptable runtime. Existing placement specification-oracle tests
remain independent and permanent.

## Performance acceptance and stop conditions

Do not put wall-clock assertions in ordinary Rust tests. Use repeated external
measurements on the same host/profile and deterministic structural counters.
The implementation is accepted only when all correctness gates pass and both
discovery shapes improve.

Required performance checkpoint:

- each of the six native witnesses remains enabled in the ordinary workspace
  suite and completes successfully;
- the median isolated elapsed time of every witness improves by at least 5x
  against the recorded baseline on the same host, with the worst witness below
  30 seconds;
- the median cached `cargo test --locked --workspace` time returns below
  90 seconds on that host;
- checker peak RSS for the worst witness falls by at least 50%; and
- no representative small placement case regresses by more than 20% beyond
  measurement noise.

These are adoption thresholds for this focused effort, not permanent CI timing
assertions. Record raw samples and medians in a maintained development note;
generated reports remain ignored build artifacts.

There are two explicit checkpoints:

1. **Diagnosis checkpoint.** If phase timings do not identify placement
   checking as the dominant cost, stop before rewriting the solver and amend
   the proposal around the measured owner.
2. **Optimization checkpoint.** If compact state, immutable indices and the
   worklist preserve correctness but miss the thresholds, retain independently
   useful low-complexity improvements and profile again. Promote CFG compaction
   or another owner only through a design amendment. Do not move the witnesses
   out of `make check`, weaken lifecycle lowering, or waive the cost for LA05.

An improvement that passes only optional or only array witnesses is incomplete.
D01 and D02 remain separate acceptance rows until both pass.

## Testing and validation

Owner-local placement tests prove:

- bitset top, empty, intersection, membership, canonical masking and removal
  counts;
- deterministic queue order, deduplication and termination accounting;
- worklist/round-oracle fixed-point equivalence;
- unchanged strict diagnostic selection and provenance;
- entry/backedge, duplicate-edge, epoch, alias, ABI, preservation and scratch
  counterexamples; and
- native and synthetic target facts use the same shared algorithm.

Native-pipeline tests retain the six slow witnesses without semantic reduction.
The empty-array control and representative scalar/call cases guard against
fixed overhead regressions. Existing LIR, selection, placement, frame, physical,
artifact and golden tests continue to prove phase integration.

Each implementation task runs focused placement and native-pipeline tests. The
closing task runs `make check`, `make check-long`, the supported-toolchain gate
when Rust targets change, and the documented repeated measurement protocol from
an artifact-free snapshot or clean checkout.

## Documentation and ownership

`backend/placement` owns the solver, state representation, immutable derived
indices and owner-local tests. Target implementations continue to own resource,
alias, preservation, ABI-footprint and transfer-recipe facts. Baseline placement
continues to produce drafts and gains no checking authority.

Update the living placement contract to describe semantic fixed-point
requirements and the deterministic worklist without encoding incidental queue
containers. Update backend and testing documentation with the measurement entry
and accepted bounds. When the roadmap closes, resolve D01 and D02, update the
low-level architecture and migration handoff, and leave LA05 responsible for
broader foundation cost comparison and production adoption.

## Alternatives considered

| Alternative | Disposition |
| --- | --- |
| Move the six witnesses to `check-long` | Rejected as the solution: it hides a production-path compiler cost and weakens ordinary end-to-end coverage |
| Trust baseline placement or skip independent checking | Rejected: future register allocation requires the checker more strongly, not less |
| Keep full rounds and only run tests in release mode | Rejected: it masks algorithmic scaling and leaves ordinary development feedback broken |
| Add opaque optional/array lifecycle operations | Rejected: it weakens explicit LIR semantics and independent verification |
| Compact lifecycle CFGs first | Deferred: broader semantic transformation with more attribution and correctness risk; consider only after measured checker work |
| Parallelize the checker or add more test threads | Rejected: current witnesses already contend for CPU and memory; this does not reduce work |
| Use a general third-party bitset/dataflow framework | Not selected: the lattice and transition semantics are small and placement-specific; reconsider only with a concrete maintained dependency benefit |

## Decisions and review questions

| Question | Proposed decision |
| --- | --- |
| Are D01 and D02 separate implementations? | No. They are separate witnesses for one shared placement-checking scalability effort. |
| May convergence order change? | Yes, after round-oracle equivalence proves the same greatest fixed point; strict replay order remains unchanged. |
| May the checker consume producer analysis? | No. All facts continue to be reconstructed independently. |
| Are elapsed thresholds correctness tests? | No. Correctness uses deterministic tests; repeated timings are roadmap acceptance evidence. |
| Do the slow tests remain in `make check`? | Yes. Their excessive cost is part of the defect being fixed. |
| Is CFG compaction included? | No. It requires an explicit amendment if checker-local changes miss the checkpoint. |
| Does completion clear LA05 cost acceptance? | It clears D01/D02 only. LA05 still owns full-foundation comparison and adoption. |

Review should focus on whether the semantic-equivalence proof and performance
thresholds are strong enough to amend the frozen deterministic-round contract.
After acceptance, the implementation roadmap should divide measurement,
compact state/indices, deterministic worklist conversion, witness validation,
and cumulative cleanup into separate reviewable tasks.
