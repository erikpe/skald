# Optimization Architecture Discoveries

Status: four architectural constraints remain without implementation plans.
The reachability constraint now has a frozen
[target-independent whole-world reachability design](TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_DESIGN_PROPOSAL.md),
planned
[implementation roadmap](TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_ROADMAP.md),
and active
[discoveries record](TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_DISCOVERIES.md).
Its dependency vocabulary, shared target/lifecycle extraction, explicit roots,
deterministic closure, analysis-query foundation, and coherent final-MIR seal
binding are implemented through WRR3; sparse definitions, backend consumption,
and pruning remain in progress.
The static-lifecycle
and dense callable-local identity constraints are resolved by their completed
[static-lifecycle certificate](../archive/STATIC_LIFECYCLE_CERTIFICATE_ROADMAP.md)
and
[MIR identity rewriting](../archive/DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md)
roadmaps. The next enabling layer is also resolved by the completed
[selectable final-MIR optimization pipeline roadmap](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
and its frozen
[design record](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_DESIGN_PROPOSAL.md).
The pipeline now includes typed selection, deterministic verified execution,
reporting and inspection, shared value-use analysis, and one default
dead-pure-definition elimination pass.

This document records the compiler-architecture constraints that currently
limit target-independent and target-specific optimization in Skald. It
separates foundational changes from later high-effort opportunities so that a
future implementation roadmap can settle contracts and representation
boundaries before scheduling individual optimization passes.

## Scope and fixed assumptions

- Every Skald program is compiled as one closed world. There is no separate or
  incremental compilation boundary whose unknown future definitions must be
  preserved.
- The resulting Skald program is single-threaded. Optimizations need not account
  for concurrent source-level mutation, data races, atomic memory ordering, or
  synchronization. This does not constrain whether the compiler implementation
  performs independent work in parallel, provided its phase products remain
  deterministic.
- Current source evaluation order, checked failures, panic behavior,
  deterministic destruction, non-exclusive aliases, and mutable access through
  shared pointees remain language semantics. Potential language-contract
  relaxations are explicitly deferred.
- Compiler correctness, source acceptance, and source diagnostics must not
  depend on optimization being enabled.
- Target-independent optimization belongs after semantic correctness has been
  established. Target-specific optimization remains behind the backend
  boundary.

The implemented pipeline lowers typed HIR to preliminary MIR, plans and
synthesizes static lifecycle work, runs the selected final-MIR pass schedule,
and then enters target legality and emission. `none` is the empty unoptimized
schedule and `default` runs dead-pure-definition elimination once. The
authoritative current sequence is documented in
[Compiler Phases and Intermediate Representations](../compiler/PHASES_AND_IR.md#pipeline-contract).

## Assessment scale

Impact describes the optimization capability unlocked rather than promising a
particular benchmark result:

- **Very high:** removes a broad architectural ceiling or enables an entire
  optimization level.
- **High:** enables several important transformations or whole-program
  analyses.
- **Medium:** materially improves a narrower class of transformations.

Effort is a relative planning estimate that must be refined by a future
roadmap:

- **Medium:** likely two to four focused pull requests.
- **Large:** a staged change likely requiring several coordinated pull
  requests across representation, verification, tests, dumps, and
  documentation.
- **Extra large:** a new maintained compiler layer or a multi-milestone
  architectural program.
- **Large to extra large:** useful baseline results can arrive at large effort,
  while increasing precision remains open-ended.

## Summary

| Area | Limitation | Nature | Potential impact | Estimated effort | Recommended timing |
|---|---|---|---|---|---|
| Static-lifecycle optimization boundary | Resolved: exact verified baseline authority now permits monotone final-MIR realization | Implemented compiler proof and sealed phase-product contract | Very high architectural unlock delivered | Completed (large) | Foundation available |
| Dense index-coupled MIR identities | Resolved: private sparse transactions support coordinated deletion, replacement, CFG rewriting, import, and deterministic dense commit | Implemented editing and verification infrastructure | Very high architectural unlock delivered | Completed (large) | Foundation available |
| Block-local non-SSA values | Limits global scalar propagation, value numbering, code motion, and loop optimization | Deliberate initial representation with an eventual optimization ceiling | High for advanced portable optimization | Extra large | Defer until simpler MIR passes demonstrate the need |
| Proof provenance mixed with executable MIR | Couples CFG transformations to exact lowering shapes and derived metadata | Awkward IR layering | High for CFG and loop work | Large | Normalize incrementally as CFG passes require it |
| Direct physical-register backend lowering | Forces every MIR value and storage through a stack home and leaves no natural register-allocation layer | Deliberate bootstrap backend and the largest target-code ceiling | Very high eventual runtime value | Extra large | Largest eventual performance project |
| Reachability after machine lowering | Retains unreachable work through legality, layout, frame planning, and instruction selection | Phase-placement debt with extraction, root, closure, query, and verified-seal foundations implemented through WRR3 | High for size, compile time, and whole-world follow-ons | Large | Continue active foundational implementation |
| Conservative alias, effect, and ownership knowledge | Prevents memory and ownership optimizations unless each pass proves safety independently | Analysis-infrastructure gap under intentionally permissive language semantics | High, with precision improving incrementally | Large to extra large | Build a conservative shared analysis after the first MIR passes |

The static-lifecycle, dense-identity rewriting, and selectable-pipeline
foundations are implemented. The pipeline provides a typed static registry,
profiles, deterministic schedule resolution, a verified atomic multi-pass
runner, and default dead-pure-definition elimination. Together those
boundaries support later constant folding, algebraic simplification, copy
propagation, and conservative CFG simplification. The planned reachability
foundation is the next step chosen to exploit permanent whole-world
compilation before broadening the optimization suite. A
virtual-register backend is likely to provide the largest eventual improvement
in generated scalar code, but it is also the largest single investment and
should not be the first optimizer change.

## 1. Implemented static-lifecycle optimization boundary

### Implemented state

Static-lifecycle planning still selects diagnostics and deterministic lifecycle
order from the unoptimized whole program. It now issues compact baseline
authority over normalized lifecycle-root facts instead of retaining exact
cross-phase graph shape. Planned verification consumes draft
`PlannedMirProgram`, independently proves exact authority issuance, and returns
an opaque `VerifiedPlannedMirProgram`; synthesis accepts only that sealed
product.

Final ordinary-MIR and lifecycle-realization verification re-extracts effects
from the actual final program and enforces:

```text
effects(final MIR, lifecycle root)
    subset-of
baseline authority[lifecycle root]
```

Fact identity retains target field, access kind, root phase, and
lifecycle-owned status while excluding direct call-graph shape, spans, and
witnesses. Analysis evidence is planning-only; final executable MIR retains the
canonical lifecycle plan, structured coordinator, and immutable compact
authority.

`passes::verify_final_mir` is the central invalidation target and constructs a
read-only `VerifiedFinalMirProgram`. `BackendInput` accepts only this sealed
product, so no backend path can consume unchecked MIR or repeat
target-independent verification. The measured pipeline reports one honest final
verification execution and currently registers no production transformation.

Future passes are classified by their lifecycle effect behavior. A pass proven
to preserve static accesses, reachability, lifecycle operations, and possible
callees may use a preserving API when one is justified. Effect removal,
target narrowing, inlining, and other call-graph or CFG reshaping invalidate the
seal and must return raw MIR to `verify_final_mir`. Test-only transformations
exercise removal, narrowing, and inlining-shaped rewriting through this same
checker before backend emission. Compile-fail public API tests prove that draft
planned MIR cannot enter synthesis and raw final MIR cannot construct backend
input.

The authoritative current contract is documented in
[Compiler Phases and Intermediate Representations](../compiler/PHASES_AND_IR.md#frozen-static-lifecycle-certificate-direction).
The rationale and implementation history are preserved in the
[frozen design proposal](../archive/STATIC_LIFECYCLE_CERTIFICATE_DESIGN_PROPOSAL.md)
and
[completed roadmap](../archive/STATIC_LIFECYCLE_CERTIFICATE_ROADMAP.md).

### Optimization possibilities enabled

- dead call and unreachable-block elimination;
- target-set narrowing and devirtualization;
- inlining and its subsequent cleanup;
- target-independent whole-program reachability pruning;
- removal of effect-free or no-longer-reachable function-value targets; and
- more aggressive CFG simplification without making lifecycle diagnostics
  optimization-dependent.

### Effort

**Completed (large).** Delivery included the proof representation, independent
issuance and realization checkers, canonical planned and coordinator schemas,
sealed public phase products, driver/reporting integration, malformed-product
coverage, transformed-shape tests, documentation, and full repository,
determinism, and MSRV validation.

## 2. Implemented dense callable-local MIR identity rewriting

### Implemented state

Callable-local `StorageId`, `ValueId`, and `BlockId` values contain vector
indices. Definitions look them up by indexing `storage`, `values`, and `blocks`,
and verification requires IDs to occupy their matching positions. Every
declared transient value must retain exactly one definition, and unreachable
blocks are still structurally verified. Committed MIR therefore retains its
compact, deterministic, directly indexed representation.

Transformations now open a private callable-owned sparse transaction with
stable slots, tombstones, and explicit block order. One exhaustive mapper owns
every storage, value, block, path-condition, and optional-guard occurrence in
instructions, terminators, places, callable headers, proof metadata, and static
publication attachments. Commit validates all retained references, compacts in
canonical order, returns complete typed maps and measured change counts, and
publishes no partial program on failure.

The supported crate-private facade provides typed lookup, allocation,
substitution, instruction rewriting, edge redirection, explicit deletion, and
cross-callable rehoming. Function, member, and static-initializer bodies use the
same atomic program coordinator. A pipeline-private capability exposes only
borrowed verified MIR and consumes its seal through that coordinator. The
runner immediately invokes central ordinary and static-lifecycle realization
verification for every changed result. Backends accept only the resealed
product, and structured failures stop without exposing raw or partial MIR. The
production pass sequence remains empty.

The authoritative implemented contract is documented in
[Compiler Phases and Intermediate Representations](../compiler/PHASES_AND_IR.md#dense-callable-local-mir-identity-rewriting).
The rationale and delivery history are preserved in the archived
[design proposal](../archive/DENSE_MIR_IDENTITY_REWRITING_DESIGN_PROPOSAL.md)
and
[completed roadmap](../archive/DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md).

### Optimization possibilities unlocked

- dead definition and dead storage removal;
- copy propagation followed by actual copy deletion;
- branch folding, empty-block forwarding, and unreachable-block removal;
- callable inlining and specialization;
- removal of obsolete proof metadata; and
- deterministic optimized MIR dumps with compact rewritten identities.

### Effort

**Completed (large).** Delivery included exhaustive identity traversal, sparse
editing, deterministic dense commit, all-definition ownership transfer,
supported structural operations, cross-callable import, verified pipeline
invalidation and resealing, adversarial malformed coverage, corpus parity, and
independent-process determinism. The same exhaustive structural kernel now
also supplies immutable identity observation, allowing analyses such as the
value-use census to borrow dense or sparse MIR without callable snapshots.

## 3. Block-local non-SSA values

### Current constraint

MIR transient values are deliberately block-local. State crossing a control-flow
edge uses addressable storage rather than phi nodes or block parameters. This
keeps ownership, object places, cleanup, and initial lowering explicit, but it
also inserts storage boundaries between scalar computations in different
blocks.

Local constant folding and algebraic simplification remain straightforward.
Global scalar reasoning must instead analyze loads and stores, initialization,
aliasing, and path state.

### Nature and impact

This was a reasonable initial representation decision, not an immediate defect.
It becomes a ceiling for sparse conditional constant propagation, global value
numbering, common-subexpression elimination, loop-invariant code motion,
induction-variable simplification, and scalar promotion.

Whole-world compilation improves interprocedural knowledge but does not by
itself turn mutable storage into values. Single-threaded execution removes
concurrent interference, making promotion easier once ordinary aliasing and
calls have been accounted for.

### Resolution direction

Do not convert all MIR ownership and aggregate state to SSA. First implement
the useful transformations supported by current MIR. If measurements then show
that mutable scalar storage is the limiting factor, introduce either:

- scalar promotion and block parameters within MIR; or
- a normalized target-independent optimization IR with SSA scalar values and
  explicit places for addressable aggregates, aliases, ownership, and
  lifecycle operations.

Construction of the SSA form would use dominance frontiers or an equivalent
algorithm, with a verifier for dominance, phi/block-argument agreement, types,
and effect ordering. Lowering to the target backend need not reconstruct
source-shaped MIR provenance.

### Optimization possibilities unlocked

- sparse conditional constant propagation;
- global value numbering and common-subexpression elimination;
- scalar load/store promotion;
- stronger dead-code elimination;
- loop-invariant code motion and induction analysis; and
- cleaner inputs for inlining and target lowering.

### Effort

**Extra large.** This adds or substantially changes a maintained IR contract,
its verifier, dumps, analyses, lowering, and test fixtures. It should be driven
by demonstrated limitations in the initial MIR optimizer rather than treated
as prerequisite work.

## 4. Proof provenance mixed with executable MIR

### Current constraint

Final MIR contains executable operations together with proof and lowering
provenance. Path conditions, structured logical-expression records, explicit
checked-operation diamonds, storage epochs, ownership protocols, and static
lifecycle metadata can name exact blocks, values, storage, and predecessor
relationships.

This enables strong producer verification, but a generic CFG transformation
must preserve or rewrite more than executable successors. Folding one branch
may invalidate the canonical logical-expression or checked-operation shape
that originally justified the MIR.

### Nature and impact

This is awkward IR layering. Proof-carrying MIR is valuable at the boundary
where HIR lowering establishes correctness; some of its provenance is no
longer needed after every consuming verifier and analysis has run. Retaining
all of it as an exact invariant throughout optimization makes otherwise local
CFG rewrites cross-cutting.

### Resolution direction

Classify metadata explicitly as one of:

- semantic and required through target lowering;
- proof provenance consumed at a named verification boundary; or
- derived analysis that may be invalidated and recomputed.

Initially, metadata-aware MIR rewriting can preserve the current verifier.
When CFG transformations become complicated, add a named normalization stage
after semantic MIR verification. That stage should consume removable
provenance, retain executable ownership and failure semantics, and produce an
optimizer-facing product with its own verifier. Analysis invalidation and
recomputation should be pipeline responsibilities rather than implicit pass
behavior.

### Optimization possibilities unlocked

- branch folding across previously canonical checked shapes;
- block merging and jump threading;
- unreachable-region deletion;
- simplification of short-circuit and checked-operation CFG;
- loop canonicalization; and
- a smaller invariant surface for later optimization passes.

### Effort

**Large.** Metadata classification and the first normalization boundary require
careful verifier work. A complete separate optimization IR would move toward
extra-large effort and should be coordinated with, rather than duplicated by,
any later SSA work.

## 5. Direct physical-register backend lowering

### Current constraint

The x86-64 backend intentionally gives every MIR storage and transient value a
fixed stack home. Instruction selection immediately uses concrete scratch and
ABI registers in the private assembly model. There is no target-private CFG of
virtual registers between semantic MIR and physical machine instructions. The
current ordering is specified in
[Backend input and legality](../compiler/BACKEND.md#input-and-legality-boundary).

Assembly peepholes can remove local redundancies, but they cannot recover the
information and freedom expected by a real register allocator. Values are
already repeatedly loaded into and stored from fixed homes, and physical
register choices have already constrained scheduling.

### Nature and impact

This was an intentional bootstrap backend. It is now the clearest hard ceiling
on generated scalar code and likely the largest eventual source of runtime
improvement. It is independent of the source program being single-threaded,
although single-threaded semantics simplify motion around ordinary memory. The
whole-world call graph can improve inlining and calling decisions, but register
allocation remains primarily callable-local target work.

### Resolution direction

Introduce a target-private low-level IR between MIR and the current physical
assembly model. It should provide:

- typed virtual integer and floating-point registers;
- explicit CFG and call-clobber information;
- target instructions before final physical-register assignment;
- use/definition and side-effect descriptions;
- liveness and live intervals or interference information;
- register allocation, coalescing, and spilling;
- stack homes only for addressable values and actual spills; and
- post-allocation peepholes and branch cleanup.

Runtime-trace updates, failure attribution, ABI constraints, ownership calls,
and hard-trap behavior should be represented as ordered pseudo-instructions or
explicit effects so target passes cannot move across them accidentally.

### Optimization possibilities unlocked

- register allocation and copy coalescing;
- elimination of most transient stack traffic;
- target-aware instruction combining and addressing-mode selection;
- instruction scheduling;
- better call argument/result placement;
- spill-cost decisions informed by loops; and
- effective machine-level peepholes after allocation.

### Effort

**Extra large.** This is a new maintained backend layer with lowering,
verification, analyses, allocation, ABI integration, tracing integration,
dumps, and native tests. It likely offers the largest eventual runtime payoff,
but it should build on a stable target-independent optimization contract.

## 6. Reachability after machine lowering

### Current constraint

The backend currently lowers executable definitions and generated helpers into
the assembly model before retaining only artifacts reachable from exported
machine symbols. This produces a pruned artifact, but unreachable definitions
have already participated in target legality, layout, trace planning, frame
planning, and instruction selection. The current target-private pass is
described in
[closed-world artifact retention](../compiler/BACKEND.md#assembly-emission-and-artifact-retention).

### Nature and impact

This is phase-placement debt. Permanent whole-world compilation means Skald can
know the complete semantic root set before backend lowering. Deferring all
pruning loses compile-time and code-generation benefits and prevents
reachability from simplifying MIR, call-target sets, and dispatch metadata.

### Resolution direction

Add target-independent reachability over final MIR using the implemented
monotone lifecycle certificate and final invalidation boundary. The root model
must conservatively include:

- the program entry and every contract-required exported artifact;
- static activation and shutdown work;
- exact callable addresses and indirect-call candidate sets;
- virtual and interface dispatch targets;
- copy, assignment, destruction, array, optional, and shared lifecycle work;
- static data and literal dependencies; and
- any opaque external boundary that can make a definition reachable.

Preserve target-private artifact retention after lowering. It still owns
generated helpers, target symbols, trace metadata, panic messages, and data
that do not have target-independent identities.

The frozen
[whole-world reachability design](TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_DESIGN_PROPOSAL.md)
develops this direction as reusable seal-bound dependency and reachability
infrastructure. Its first pruning client removes executable definitions while
preserving dense semantic declarations and global identities; later passes may
reuse the same possible-target and closure queries for devirtualization,
inlining, effect analysis, specialization, and metadata pruning.
Delivery is divided by the planned
[whole-world reachability roadmap](TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_ROADMAP.md).

### Optimization possibilities unlocked

- whole-program dead-function and dead-method elimination;
- smaller dispatch and metadata inventories;
- reduced backend legality and layout work;
- fewer runtime-trace records and emitted symbols;
- stronger devirtualization after unused implementations disappear; and
- lower compilation time and output size.

### Effort

**Large.** The graph traversal itself is moderate. The difficult part
is specifying complete roots across function values, dispatch, static
lifecycle, ownership, arrays, and generated operations, plus coordinating the
result with stable semantic declarations, sparse executable definition tables,
lifecycle certification, final verification, and backend planning.

## 7. Conservative alias, effect, and ownership knowledge

### Current constraint

Skald aliases are intentionally non-exclusive, and mutable access through
shared pointees is legal. A transformation may not assume that two mutable
parameters designate different places. Retains and releases are individually
unobservable, but changing their last-owner point can change observable
destructor timing.

Without shared analysis infrastructure, each pass must either treat calls and
mutable places as broad barriers or independently reconstruct the facts needed
to prove a narrower transformation safe.

### Whole-world and single-thread consequences

This area benefits especially from Skald's fixed execution model:

- no other source thread can mutate storage between two operations;
- aliases are call-scoped and cannot escape into stored program state;
- the complete set of direct, virtual, interface, and function-value targets
  is available for fixed-point analysis;
- every internal callable body is available unless it is an external
  declaration; and
- call sites may be specialized for their actual alias relationships even
  though the general callable must accept overlapping aliases.

An external call or otherwise opaque operation must remain a conservative
barrier according to its ABI contract. Parallelizing the compiler itself would
not change these source-program facts; parallel analyses would merely need
deterministic joins and publication.

### Nature and impact

The permissive alias rules and observable destruction are language semantics,
but the broad loss of optimization is primarily an analysis-infrastructure gap.
The compiler can prove non-aliasing, non-escape, non-mutation, or non-last-owner
facts at particular program points without changing the language.

### Resolution direction

Build a reusable, conservative whole-program analysis service in stages:

1. Represent abstract regions for local storage, statics, allocation sites,
   fields and projections, array backings, and unknown external memory.
2. Compute callable summaries for reads, writes, captures, allocation,
   ownership operations, possible destruction, failure, and call targets.
3. Propagate summaries to a fixed point over the complete call graph, expanding
   dynamic and indirect calls through their finite target sets.
4. Track flow-sensitive points-to and escape facts where profitable, widening
   conservatively at joins and recursive components.
5. Expose common queries such as `may_alias`, `may_read`, `may_write`,
   `may_escape`, `may_destroy`, and `owner_must_remain_live`.
6. Let the pipeline invalidate or recompute summaries explicitly when a pass
   changes calls, storage, or control flow.

Later precision can include context-sensitive call-site specialization and
runtime alias versioning, selecting an optimized path after an internal overlap
check while retaining the general overlapping-alias path. Neither technique
requires a source-visible address or exclusive-reference contract.

### Optimization possibilities unlocked

- load forwarding and redundant-load elimination;
- dead-store elimination through calls proven not to observe the destination;
- loop-invariant load motion;
- scalar replacement of eligible addressable state where required allocation,
  failure, and destruction behavior remains unchanged;
- call-site specialization for proven-distinct actual arguments;
- redundant anchor and retain/release elimination when last-owner timing is
  unchanged; and
- stronger inlining, devirtualization, and interprocedural constant
  propagation.

### Effort

**Large to extra large.** A conservative mod/ref summary and region model are a
large but bounded first milestone. Precise points-to, escape, context-sensitive
specialization, and ownership-count reasoning are independently valuable later
milestones rather than prerequisites for the first optimizer.

Escape analysis may identify shared allocations that could otherwise be placed
on the stack or eliminated. Performing that transformation when it removes a
required heap-allocation failure would change the current language contract, so
allocation elision is not included in this assessment.

## Recommended starting sequence

### Foundation

1. Settle the lifecycle proof relation for effect-removing transformations.
2. Introduce exhaustive MIR traversal, rewriting, and deterministic compaction.
3. Add the named pass registry, selection profiles, per-pass verification, and
   optimized MIR dumps around those contracts. This is implemented by the
   [selectable final-MIR optimization pipeline](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md).

These steps should be proven by one deliberately small transformation, such as
local constant folding or dead pure definition elimination. Building a large
pass manager without a safe rewrite and verification contract would not remove
the actual architectural constraints.

### Next foundational layer

4. Implement the frozen target-independent whole-world reachability design and
   retain machine-artifact pruning as the final target safety net.
5. Generalize callable effects and alias queries as real passes demonstrate
   where conservative barriers cost useful transformations.

### First broader optimization layer

6. Extend the implemented dead-pure-definition elimination layer with
   conservative constant folding, algebraic simplification, copy propagation,
   and CFG cleanup in final MIR.

This layer offers the best balance of moderate-to-large effort, broad coverage,
and low semantic risk. It also creates measurements that can justify the later
architectural investments.

### Larger performance investments

7. Add a virtual-register target LIR and register allocation. This is likely
   the largest eventual improvement because the current backend gives every MIR
   value a stack home.
8. Normalize proof-heavy MIR metadata as increasingly aggressive CFG passes
   require it.
9. Introduce scalar SSA or a separate optimization IR only when global scalar
   and loop optimization benefits justify the extra maintained boundary.

The last two decisions should be coordinated: one normalized SSA-capable
optimization IR is preferable to independently adding overlapping
normalization and SSA layers.

## Expected return on effort

The likely qualitative ranking is:

1. **Largest architectural unlock:** lifecycle-certificate relaxation plus the
   MIR rewrite boundary. These do not directly make programs faster, but nearly
   every safe final-MIR optimization depends on them.
2. **Best early delivered value:** earlier whole-world reachability followed by
   simple final-MIR passes. They improve code quality and size while exercising
   the complete framework at manageable risk.
3. **Largest eventual runtime impact:** a virtual-register backend with
   register allocation, because it removes the systematic stack traffic in the
   current bootstrap target.
4. **Largest advanced whole-world opportunity:** shared alias, effect, escape,
   and ownership analysis. Closed-world, single-threaded execution makes this
   substantially more tractable than in an open-world concurrent language.
5. **Most deferrable high-cost change:** SSA. It enables powerful global and
   loop transformations, but is unnecessary for validating the initial pass
   framework and basic MIR optimizer.

These rankings are architectural expectations, not benchmark results. A future
roadmap should establish representative compile-time, assembly-size, stack-use,
and native-runtime measurements before committing to the extra-large backend or
SSA programs.

## Dependencies and open design decisions

- The lifecycle certificate relation is resolved and available for
  effect-removing reachability, devirtualization, and later inlining.
- MIR rewriting covers callable-local reference-bearing operations and
  metadata; whole-definition retention deliberately adds a separate
  stable-identity program-level boundary.
- The boundary between metadata-aware final MIR and a normalized optimization
  product should be chosen before both CFG normalization and SSA work expand.
- Whole-program roots are frozen by the target-independent reachability design
  and must remain explicit as the roadmap moves retention ahead of the backend.
- The target LIR must retain explicit ABI, runtime-trace, failure, and ownership
  barriers before register allocation or instruction scheduling is implemented.
- Alias and effect analysis should preserve current permissive language
  semantics and infer facts at program points; source annotations and language
  restrictions are out of scope for this discovery record.
