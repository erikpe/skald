# MIR Snapshot Analysis Reuse Design Proposal

Status: frozen delivered decision record. The design was accepted and its
bounded Gate 2 go outcome was delivered on 2026-09-12 through the completed
[MIR snapshot analysis reuse roadmap](MIR_SNAPSHOT_ANALYSIS_REUSE_ROADMAP.md).

This proposal defines how Skald may reuse expensive read-only MIR analyses
while the exact verified MIR snapshot remains unchanged. It follows the
completed
[structural CFG analysis roadmap](MIR_STRUCTURAL_CFG_ANALYSIS_ROADMAP.md),
which established callable-local topology and dominance but deliberately kept
their lifetime inside one verifier or optimization invocation.

The immediate candidate is proof-rich local constant analysis. Several passes
solve the same executable callables, and the default schedule repeats primitive
constant folding. Delivery remains conditional on reproducible evidence that
multiple requests occur on the same snapshot and that avoiding those
computations improves compiler cost without materially increasing retained
memory. The design does not assume that every MIR analysis belongs in a cache.

## Delivered outcome

Skald retains one runner-owned, typed proof-snapshot cache for successful local
constant solutions. The production pipeline always uses memoization. Complete
session invalidation occurs before verification of every changed proof-rich
outcome and at proof normalization; no cached fact reaches a final seal,
checkpoint, pass outcome, or backend input. The uncached policy remains
test-only because it provides direct behavior, failure, output, and lifecycle
equivalence coverage.

Three paired comparisons over the maintained 19-workload matrix preserved the
deterministic compiler and native projections. Memoization reduced local
constant computations from 12,040 to 6,399 in each run, a workload range of
25–87.5 percent. Multiple nontrivial workloads improved beyond combined
dispersion in all three pairs, no workload had a repeatable adjusted
regression, and the largest median peak-RSS increase was 2.34 percent. The
[measurement evidence](../development/MIR_ANALYSIS_REUSE_MEASUREMENTS.md)
records the raw reports, calculations, and bounded Gate 2 decision. This
result does not justify caching another analysis kind, partial invalidation,
schedule changes, skipped verification, or a general analysis manager.

## Intended outcome

- Measure analysis requests, computations, reuse, invalidation, and retained
  results by analysis kind and pass occurrence.
- Reuse one successful callable-local analysis result between pass occurrences
  only while the runner still owns the same verified MIR product.
- Make invalidation structural: a changed pass consumes the old verified
  product and causes its complete analysis session to be dropped before fresh
  verification or another pass.
- Keep analysis construction lazy and typed, with one result per callable and
  snapshot.
- Preserve mandatory verification, proof normalization, final resealing,
  checkpoints, deterministic reports, and backend trust products.
- Establish a narrow extension point for another measured analysis without a
  type-erased analysis manager, dependency graph, or preservation framework.

## Current architecture and evidence

The [pipeline runner](../../crates/skald-compiler/src/passes/pipeline/execution/runner.rs)
owns an opaque verified product between pass occurrences. A proof-rich or
final-stage pass receives a capability containing that product. An unchanged
outcome returns the product intact. A changed outcome consumes it through a
supported rewrite or retention operation, and the runner performs mandatory
verification before the next occurrence.

The [seal owner](../../crates/skald-compiler/src/passes/pipeline/seal.rs)
already demonstrates one safe form of analysis ownership. A
`VerifiedFinalMirProgram` stores reachability computed from its exact normalized
program. Final-stage invalidation consumes both together, and resealing builds
fresh reachability. Proof-rich verification computes reachability for its
checks but does not retain it because proof normalization will create another
representation.

The default schedule contains multiple consumers of
[`solve_local_constants`](../../crates/skald-compiler/src/passes/pipeline/optimizations/local_constant/solve.rs):

- primitive constant folding;
- primitive algebraic simplification;
- checked integer folding;
- checked floating-to-integer folding;
- constant short-circuit folding; and
- conservative CFG cleanup.

Primitive constant folding occurs three times in the default schedule. Many of
these passes change MIR and therefore require fresh facts. When a pass is
unchanged, however, the next consumer currently solves every executable
callable again even though the verified product is identical. Static call-site
inventory establishes an opportunity, not a performance result.

The local constant solution is already an immutable, callable-owned value. Its
facade states that the result belongs to one verified callable snapshot. The
solution carries constants, provenance, logical selections, and retained
checked failures, and its builder validates the identities it consumes. It is
a suitable first candidate because consumers already depend on the same typed
product rather than merely having similar private algorithms.

The implemented `MirCfgTopology` and `MirDominators` are intentionally not the
first cross-pass candidate. Their C03 consumers currently build them once per
verification or analysis scope. They may join a later session only if
measurements show repeated construction on unchanged pass snapshots and their
callable-local memory cost is acceptable.

The [cleanup measurement procedure](../development/CLEANUP_MEASUREMENTS.md)
already provides reproducible compiler wall-time, peak-RSS, artifact, and
semantic comparisons. Pipeline occurrence records already identify exact pass
positions, outcomes, durations, processed callables, rewrite counts, and
verification executions. A19 should extend this evidence with analysis-specific
counts before changing reuse behavior.

The sibling Niflheim compiler has pass-local dataflow invalidation but no
general cross-pass analysis cache worth copying. Skald's consuming capabilities
and verified seals provide a stronger invalidation boundary, so the design
should build on them directly.

## Scope and invariants

- Analysis reuse is compiler-internal and target-independent.
- A cached result belongs to exactly one verified proof-rich or normalized
  program snapshot and one callable identity.
- The pipeline runner owns session lifetime. Pass implementations request typed
  facts but cannot construct, detach, replace, or return a session.
- An unchanged outcome preserves the session because it preserves the exact
  verified product.
- Every changed outcome invalidates the complete session before reverification,
  regardless of the reported changed-callable set.
- Proof normalization ends the proof-rich session. Facts never cross into the
  normalized stage.
- Final-stage reachability remains seal-owned and authoritative. A session may
  borrow it through the verified product but must not duplicate or replace it.
- Verification never trusts pass-analysis cache entries. Initial verification
  and every reseal compute all required verification facts from their input.
- Analysis errors remain attributed to the pass occurrence that requested the
  fact. Only successful results are retained.
- Callable lookup uses stable identities from the exact verified program.
  Numeric identity payloads are never treated as proof of snapshot membership.
- Cache state does not affect diagnostics, MIR dumps, checkpoints, assembly,
  runtime behavior, or optimization selection.
- Deterministic measurements may expose cache use; operational timings remain
  non-deterministic observations under the existing reporting contract.

## Non-goals

- No global, thread-local, process-wide, or cross-compilation cache.
- No reuse across a changed MIR snapshot, even when a change report names only
  unrelated callables.
- No revision numbers, structural hashes, dirty-bit propagation, incremental
  dependency graph, or fine-grained invalidation summary.
- No pass-declared analysis preservation set.
- No skipped verification, reduced verifier coverage, or reuse of verifier
  diagnostics.
- No cached mutable graph, edit object, rewrite plan, or reference into a MIR
  vector.
- No type-erased `Any` map, open registration API, generic graph framework, or
  public analysis service.
- No automatic caching of every available topology, census, reachability,
  liveness, lifecycle, or alias query.
- No pass fusion, schedule reordering, repeated-pass removal, or optimization
  semantics change.
- No concurrent pass execution. The design must not prevent a later parallel
  implementation, but A19 does not introduce one.

## Selected design

### The runner owns a stage-specific analysis session

The runner will keep a private proof-rich analysis session beside the current
`VerifiedProofMirProgram`. A pass receives one pipeline-owned context containing
its existing capability and a temporary mutable borrow of that session. The
context exposes the verified program and narrow typed analysis queries; it does
not expose the session representation.

The initial session contains only the local constant result table and its
measurement state. There is no empty generic final-stage counterpart. A
separate final-stage session should be introduced only when a measured final
analysis has at least two consumers on an unchanged normalized snapshot.

The pass callback receives a stage-specific context rather than two unrelated
arguments. Conceptually, `MirProofPassContext<'session>` owns the existing
`MirProofPassCapability` and borrows `MirProofAnalysisSession` for that callback.
It mirrors the capability's unchanged and rewrite operations and adds the
typed query. The registry callback type becomes higher-ranked over the private
session lifetime, so neither a pass registration nor an outcome can name or
retain a particular runner session.

Conceptually, runner state is:

```text
proof state = verified proof-rich MIR + proof analysis session
final state = verified normalized MIR
```

The session is not a field of `VerifiedProofMirProgram`. Verified products are
trust tokens and inspection products; demand-driven optimization memoization is
pipeline execution state. Keeping them separate preserves the seal's current
`Clone`, equality, debug, inspection, and public borrowing behavior.

### Ownership performs complete invalidation

The pass outcome continues to decide validity:

```text
unchanged(verified snapshot)
    -> retain the same verified product and analysis session

changed(raw rewritten program)
    -> drop the complete old session
    -> verify the rewritten program
    -> begin with an empty session for the new verified snapshot

proof normalization
    -> drop the complete proof-rich session
    -> normalize and verify
    -> expose no proof-rich fact to final passes
```

The changed-callable list is reporting and rewrite evidence, not an
invalidation authority. CFG edits, identity compaction, static effects,
callable retention, and permanent attachments can alter facts outside the most
obvious edited body. Complete invalidation is cheap to reason about and matches
the current complete reseal.

### Queries remain typed and lazy

The first query accepts a `CallableId`, resolves that identity through the
exact program owned by the pass context, and returns a shared immutable local
constant solution. The implementation uses a typed callable-keyed table; it
does not accept arbitrary analysis keys or constructors from callers.

The cache stores at most one successful result per executable callable for its
snapshot. The first request computes and inserts the solution. Later requests
return the stored solution. An unknown, non-executable, or mismatched callable
fails through a deterministic pipeline analysis error rather than falling back
to another definition.

The query returns an `Arc<LocalConstantSolution>` so existing immutable plans
can cheaply retain a result while applying their atomic rewrite. The handle may
be retained only inside the current pass invocation,
including while that pass applies an atomic rewrite plan derived from the old
snapshot. Pass outcome types contain no analysis-session or result handle, so
facts cannot enter the next occurrence. A reference-counted immutable handle is
private lifetime plumbing rather than cross-snapshot reuse. Consuming the pass
context to begin an unchanged or rewrite outcome also prevents further queries
through that context.

Failed computations are not inserted. The first analysis failure terminates
the requesting pass under the existing `MirPassFailure::Execution` path, so
there is no later same-run consumer to benefit from caching that error.

### Measurement precedes reuse

The implementation roadmap must first add a typed analysis-use observation at
the pipeline boundary. For each pass occurrence and analysis kind, record:

- requests;
- computations;
- cache hits;
- results present before the occurrence;
- results inserted by the occurrence; and
- results discarded when the occurrence changes MIR.

Counts saturate through the repository's existing conventions. Their ordering
follows schedule occurrence and a closed compiler-owned analysis-kind order.
They must be identical across independent processes for identical inputs.
Disabled occurrence reporting does not require retaining per-occurrence data,
but aggregate deterministic counts remain available to the measurement owner.

The baseline first routes local constant requests through the typed context
with memoization disabled, making `requests == computations`. The cache is
enabled only after reviewed workloads demonstrate multiple requests for at
least one unchanged snapshot. Compare the pre-reuse and post-reuse revisions
with the existing cleanup procedure, including compiler wall time, peak RSS,
MIR schedule, pass outcomes, assembly hashes, and native result digests. A
test-only policy runs the same schedule with memoization disabled and enabled
for direct product comparison; this is not a request or CLI option.

The initial delivery is accepted only if:

- computations are lower than requests on a maintained workload;
- all saved computations occur on snapshots preserved by unchanged outcomes;
- assembly and semantic observations are unchanged between cache modes;
- peak retained results and peak RSS are reported; and
- timing results are presented with dispersion, without becoming a correctness
  threshold.

If the baseline finds no same-snapshot reuse or an unacceptable memory cost,
A19 should close as measured and unjustified rather than ship dormant cache
infrastructure.

### Local constant analysis is the only initial cached fact

The initial migration changes the local constant facade from a direct solver
used independently by each pass into a session query used by selected
proof-rich consumers. The solver and its result type remain owned by the
optimization analysis module. The pipeline session owns only memoization and
usage accounting.

Consumers migrate only when they use the complete existing result contract.
Pass-specific topology observations, carrier evidence, candidate scans, and
rewrite plans remain with their pass. Similar inputs do not justify combining
those responsibilities.

All default-schedule local constant consumers should eventually request the
session result so an unchanged chain can reuse it. Migration can proceed one
consumer at a time behind a test-only cache-disabled mode. A changed consumer
must still receive a fresh result on its next scheduled occurrence.

### Module ownership stays narrow

Add a private `pipeline::snapshot_analysis` facade with a session
implementation, typed usage records, and colocated tests. The runner and pass
context import only that facade. Keep the local constant solver with its
existing cohesive optimization analysis implementation; selectively expose its
constructor and immutable result to the enclosing pipeline so the session can
memoize it. Individual passes stop calling the constructor directly as they
migrate and obtain results only through their context.

This avoids moving the solver together with all of its checked-operation,
logical-topology, carrier, and primitive-evaluation dependencies merely to
change lifetime ownership. The new facade does not re-export a public module,
and no session API crosses the crate-private pipeline boundary.

### Verification and seal-owned reachability stay independent

MIR verification continues constructing its own structural facts and
dominators from untrusted input. A cache is available only after verification
has produced the capability. This preserves malformed-MIR diagnostics and
their ordering and avoids making pass facts as proof of validity.

Normalized reachability remains inside `VerifiedFinalMirProgram`. The
whole-world retention pass continues using that exact seal-owned result.
Moving reachability into a general session would weaken a useful invariant and
create two owners for backend completeness.

### Inspection sees products, not memoization state

Existing checkpoint inspectors continue borrowing only verified MIR and
seal-owned facts. They do not receive an analysis session or force lazy
queries. Enabling an inspector therefore cannot warm the cache, change reuse
counts, retain results, or influence a later pass.

Analysis-use records belong to pipeline measurement and reporting. They are
not embedded in MIR dumps or checkpoint identity. Quiet compilation performs
the same queries and transformations even when detailed records are disabled.

## Alternatives considered

### Store every analysis in the verified seal

Rejected for optimization memoization. The final seal appropriately owns
reachability because backend validity depends on it. Local constants and other
pass conveniences are optional, lazy, and schedule-dependent. Embedding them
would complicate seal equality, cloning, inspection, and trust semantics.

### Use a global or thread-local cache keyed by hashes

Rejected. It would cross compilation requests, require collision-resistant
snapshot identity and eviction, obscure memory ownership, and complicate
future parallel compilation. Request-local ownership is already available in
the runner.

### Add revision keys and invalidate only changed callables

Deferred. A single rewrite may compact identities or change whole-program
reachability and static effects. Correct dependency summaries would be a
larger incremental-compilation design. Complete invalidation provides a clear
first boundary and lets measurements quantify the value left on the table.

### Let passes declare preserved analyses

Deferred. The current rewrite capabilities expose several mutation families,
and a false preservation claim could silently miscompile. The unchanged versus
changed outcome already supplies one mechanically trustworthy preservation
fact.

### Cache rewrite plans instead of analyses

Rejected. Plans encode pass policy, expected old instructions, and exact edit
sites. Sharing them would couple pass implementations and risk applying a plan
under the wrong schedule context. Only phase-neutral immutable facts are
candidates for reuse.

### Eagerly analyze every callable at verification time

Rejected. Optimization-off and unrelated programs should not pay for local
constant analysis. Lazy queries retain the current pay-for-use boundary and
make request/computation counts observable.

### Remove repeated passes or fuse consumers

Rejected as an A19 mechanism. Repetition intentionally exposes new
opportunities after transformations. Schedule changes require separate
semantic and performance evidence; they do not solve general immutable-
snapshot ownership.

## Failure and diagnostic contract

- Invalid initial or rewritten MIR fails at the existing verification owner
  before any later pass can query cached facts.
- A local constant analysis failure remains an internal pass-execution failure
  attributed to the requesting occurrence and retains its current message.
- The cache never converts an error into absence, an empty solution, or a
  fallback recomputation under another identity.
- Counter overflow saturates and is reported consistently with other pipeline
  measurements.
- Allocation failure follows ordinary Rust process behavior; A19 does not add
  a recoverable memory-error protocol.
- A cache implementation defect must not be masked by recomputing after a hit.
  Test mode compares cached and uncached typed results directly.

## Validation strategy

### Ownership and invalidation tests

- Two analysis-consuming passes separated only by unchanged outcomes observe
  two requests and one computation for the same callable.
- A changed proof-rich pass discards all prior callable results; the next
  consumer computes from the newly verified program.
- A change in one callable invalidates cached results for every callable.
- Repeated uses of one pass identity obey occurrence order rather than sharing
  through registry identity.
- Proof normalization drops the proof-rich session even when every preceding
  pass was unchanged.
- Disabled passes and the `none` profile issue no hidden analysis requests.
- An analysis failure is attributed to the exact requesting occurrence and
  publishes no later cache or checkpoint observation.

### Result-equivalence tests

- Cached and direct local constant solutions are equal for literals, primitive
  chains, checked integer and floating protocols, logical selections, loops,
  disconnected blocks, and carrier-mediated facts.
- Unknown, foreign, sparse, and malformed callable identities fail through the
  selected owner without cross-callable lookup.
- Cached results do not alter edit candidates, changed-callable counts,
  verification counts, MIR dumps, or pass measurements unrelated to reuse.
- Inspection enabled and disabled produce identical cache usage and compiler
  products.

### Repository evidence

- Add focused pipeline schedules with synthetic unchanged and changed
  consumers to pin invalidation independently of production-pass heuristics.
- Run the complete local constant and affected optimization suites.
- Compare cache-disabled and cache-enabled detailed occurrence records across
  independent processes.
- Capture the reviewed cleanup workloads with identical compiler profile,
  repetition policy, host conditions, and input inventory.
- Run `make check`, `make msrv-check`, and `git diff --check` for each delivery
  task that changes Rust code.

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| A stale fact survives mutation | Runner drops the complete session on every changed outcome before resealing. Outcome types cannot carry session state. |
| A cache is paired with another program | The runner constructs and owns the verified-product/session pair; typed queries resolve callable IDs through that exact product. |
| Memory retention outweighs saved work | Lazy successful entries only, peak-entry and RSS evidence, complete drop on change, and a cache-disabled comparison mode. |
| Generic infrastructure obscures ownership | Start with one typed local constant table and a closed analysis-kind measurement vocabulary. |
| Verification begins trusting optimization facts | Sessions exist only after verification and are never inputs to verification or seal construction. |
| Reports become order-dependent | Record deltas in schedule order and analysis kinds in a closed deterministic order; test independent processes. |
| Existing plans need facts during atomic rewrite | Permit immutable handles only inside the current transform; no outcome or checkpoint type can retain them. |
| Fine-grained reuse is added prematurely | Treat changed-callable data as reporting only and defer revisions, preservation, and partial invalidation. |

## Roadmap boundary

The implementation roadmap should have four reviewable outcomes:

1. add analysis-use instrumentation and capture the uncached baseline;
2. establish the runner-owned proof-rich session and mechanically complete
   invalidation tests with reuse still disabled;
3. enable local constant reuse, migrate its proof-rich consumers, and compare
   enabled and disabled products and measurements; and
4. document the accepted result, remove temporary measurement scaffolding that
   has no continuing value, and either mark A19 complete for this bounded scope
   or record a measured follow-up.

The roadmap must not schedule topology, dominance, value-use census,
reachability, lifecycle, or partial-invalidation caching without new evidence.
If local constant reuse fails the acceptance threshold, the final task should
record that result and close the experiment without leaving an unused session
abstraction.

## Decision summary

| Question | Decision |
| --- | --- |
| Who owns reuse? | The pass runner owns a private stage-specific session beside the verified product. |
| What is cached first? | Successful proof-rich local constant solutions, gated by baseline evidence. |
| What preserves a session? | Only an unchanged pass outcome retaining the exact verified product. |
| What invalidates it? | Every changed outcome and the proof-normalization boundary, completely. |
| Are unaffected callables retained after a change? | No. Changed-callable summaries are not invalidation authority. |
| Does verification consume cached facts? | No. Verification and resealing always derive their own facts. |
| How is reuse observed? | Deterministic typed request/computation/hit/insert/discard counts plus existing operational measurements. |
| Are revision keys or preservation sets introduced? | No; both require separate measured justification and design. |
| What happens if reuse is not beneficial? | Close A19 as measured and unjustified, without shipping dormant cache infrastructure. |
