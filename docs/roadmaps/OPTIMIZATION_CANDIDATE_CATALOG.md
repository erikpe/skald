# Optimization Candidate Catalog

Status: living current-and-future optimization register. This document is
intentionally expected to gain, change, merge, and lose entries as
measurements, designs, roadmaps, implementation, and language decisions
change.

This catalog records implemented and plausible future Skald optimizations, the
representation or graph on which each belongs, its pipeline placement,
approximate implementation effort, potential value, and principal correctness
risks. It is not a roadmap, promise, priority queue, or authoritative
specification of implemented behavior.

The
[optimization architecture discoveries](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md)
remain authoritative for broad architectural constraints. Confirmed designs
and implementation roadmaps remain authoritative for work that has entered a
delivery sequence. This file is the lightweight inventory spanning observed
opportunities, reviewed designs, active delivery, and implemented
optimizations.

Every Skald compilation is whole-world and every resulting program is
single-threaded. Candidates may use those guarantees, but must still preserve
sequential mutation, permissive aliases, checked failure, allocation behavior,
observable destructor timing, static activation, diagnostics, and runtime
traces unless a separate language decision changes the relevant contract.

## How to maintain this catalog

### Entry lifecycle

Add a future entry when its likely owner and semantic boundary can be stated,
even if it still needs enabling architecture. Add an implemented entry when it
forms part of the baseline needed to understand placement or composition.
Keep the entry concise and link to a focused discovery, design, roadmap, or
living contract when detailed evidence exists.

Advance an entry's status when:

- a draft design first gives it a concrete semantic boundary;
- a frozen design or concrete roadmap commits to its delivery;
- implementation starts; or
- implementation and living documentation become authoritative.

Remove an entry only when it is rejected, superseded, merged into a better
entry, or determined not to be an optimization Skald intends to track.
Implemented entries remain as the compact baseline against which future work
is placed. They link to living or archived authoritative documentation rather
than duplicating its full contract.

When an entry is removed because of a durable rejection or language decision,
record that reasoning in the relevant design, discovery, or language contract
before removing the row. Git history owns incidental catalog chronology.

Entry identifiers are local handles for discussion. Gaps are expected and
identifiers must not appear in production code, tests, or living semantic
contracts.

### Status vocabulary

| Status | Meaning |
|---|---|
| **Implemented** | Current compiler behavior; the entry links to its authoritative living contract or completed delivery record |
| **In progress** | A concrete roadmap is actively being implemented |
| **Proposed** | A frozen design or concrete planned roadmap defines the optimization, but implementation is not complete |
| **Draft design** | An unfrozen design proposal gives the candidate a concrete boundary that may still change |
| **Follow-up** | Can be designed against implemented architecture, including the local-simplification layer |
| **Foundation needed** | Valuable, but a named analysis or representation must land first |
| **Contract decision** | Sound implementation depends on a language or observable-runtime decision |
| **Research** | Placement or benefit remains uncertain enough to require measurement or prototyping |

### Effort vocabulary

| Effort | Typical scope |
|---|---|
| **Small** | One focused pass or local extension using existing analyses and rewriting |
| **Medium** | A pass plus shared support, verifier changes, or several semantic families |
| **Large** | Multiple compiler owners, a reusable analysis, or proof/lifecycle restructuring |
| **Extra large** | A new maintained IR layer or a broad interprocedural optimization system |

Potential value uses **low**, **medium**, **high**, or **very high** and names
the expected dimension: runtime, code size, compile time, or architecture.
These are hypotheses until measurements exist.

The first deterministic local-suite measurements and their limits are recorded
in the
[local final-MIR simplification discoveries](LOCAL_FINAL_MIR_SIMPLIFICATION_DISCOVERIES.md#initial-measurements-separate-local-wins-from-whole-world-pruning).
They establish a baseline, not a general workload ranking.

## Current baseline and design boundary

The implemented local final-MIR simplification layer follows its
[frozen design](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_DESIGN_PROPOSAL.md)
and [completed roadmap](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md).
It owns conservative integer/boolean constant folding, primitive
algebraic simplification with guarded value forwarding, ordinary constant
branch folding, and unprotected unreachable-block removal. Its concrete
entries appear below as **Implemented** and link to their authoritative living
contracts.

## HIR and MIR-lowering boundary

HIR should not become a second general optimizer. It is the right owner only
when a transformation depends on resolved language constructs that final MIR
intentionally erases, or when it prevents unnecessary proof-heavy lowering
without changing diagnostics or evaluation.

| ID | Candidate | Placement | Status / effort | Potential value | Main pitfalls |
|---|---|---|---|---|---|
| HIR-03 | [Statically selected type tests and cast outcomes](../compiler/PHASES_AND_IR.md#mir) | During typed HIR-to-MIR lowering, after resolution has established exact type relations | Implemented / **Medium** | Medium runtime and avoids unnecessary checked CFG | Applies only to statically proven relations; dynamic tests, checked carriers, access views, and failure behavior remain explicit |
| HIR-01 | Compile-time evaluation of explicitly guaranteed pure language intrinsics | After type checking and overload resolution; before MIR lowering | Foundation needed / **Large** | Medium runtime and code size; medium compile-time reduction for generated MIR | Requires a stable intrinsic-purity and failure contract; must preserve diagnostics, evaluation order, allocation, and target-independent results |
| HIR-02 | Avoid constructing proof-heavy lowering for statically selected optional, cast, or pattern outcomes | After type relations and definite-state checks; before checked MIR diamonds are emitted | Research / **Medium to large** | Medium compile time and MIR size | HIR must retain enough exact path, cleanup, and ownership information; doing this before diagnostic owners run could change accepted programs or error ordering |

Primitive arithmetic should normally remain a final-MIR concern. Source- or
HIR-level folding would duplicate typed MIR semantics and risk making source
diagnostics optimization-dependent.

## Final-MIR local value graph

These candidates consume transient `ValueId` definitions and uses inside one
callable. Unless stated otherwise, they belong after final-MIR verification
and before CFG cleanup, dead-pure cleanup, and whole-world retention.

| ID | Candidate | Placement and ordering | Status / effort | Potential value | Main pitfalls |
|---|---|---|---|---|---|
| FMV-12 | [Dead-pure-definition elimination](../compiler/PHASES_AND_IR.md#selectable-final-mir-optimization-pipeline) | First pass in the current default final-MIR schedule; before whole-world retention | Implemented / **Medium** | Medium MIR/code-size reduction and cleanup foundation | Intentionally limited to unused non-failing scalar definitions; loads, calls, checked operations, ownership, and semantic queries remain |
| FMV-13 | [Primitive integer/boolean constant folding](../compiler/PHASES_AND_IR.md#local-final-mir-simplification) | After an initial dead-pure cleanup; before algebraic simplification, and repeated afterward | Implemented / **Medium** | Medium runtime and code size; high enabling value for CFG cleanup | Exact wrapping/width behavior, unsupported-operation barriers, stable spans/identities, and no checked or floating families initially |
| FMV-14 | [Primitive algebraic simplification with guarded value forwarding](../compiler/PHASES_AND_IR.md#local-final-mir-simplification) | After primitive constant folding; before repeated folding and dead-pure cleanup | Implemented / **Medium to large** | Medium runtime and code size | Every forwarded use must be an allowed ordinary role; operand evaluation, proof metadata, checked protocols, and floating identities are barriers |
| FMV-01 | Raw-bit primitive cast folding | After basic constant folding; before algebraic simplification | Follow-up / **Small** | Low to medium runtime and code size | Only truly bit-preserving `u64`/`f64` reinterpretations are simple; must retain raw NaN payloads and result type exactly |
| FMV-02 | Redundant primitive cast and cast-chain elimination | After constant folding; before dead-pure cleanup | Follow-up / **Medium** | Medium runtime and MIR size | Integer width and boolean canonicalization matter; checked `f64` conversion and proof-coupled casts are barriers |
| FMV-03 | Local common-subexpression elimination for non-failing primitive rvalues | After constant/algebraic simplification; before dead-pure cleanup | Follow-up / **Medium** | Medium runtime and code size | Restrict to exact same-block pure operations; source values must dominate; floating equivalence, spans, runtime traces, and checked operations need exclusions |
| FMV-04 | Wrapping-integer reassociation and constant aggregation | After primitive constant folding; before local CSE | Follow-up / **Medium** | Medium runtime, especially generated arithmetic | Reassociate only exact wrapping integer operations; do not move or suppress operand producers; exclude floating arithmetic and checked protocols |
| FMV-05 | Deterministic constant floating comparison | After an exact IEEE constant model; before branch folding | Foundation needed / **Medium** | Low to medium runtime and CFG value | Must specify NaN, unordered predicates, signed zero, and raw-bit handling independently of the Rust host |
| FMV-06 | Deterministic constant floating arithmetic | After an exact target-independent IEEE evaluator; before algebraic simplification | Foundation needed / **Large** | Medium runtime and code size in numeric programs | Rounding, NaN result/payload policy, infinities, subnormals, signed zero, host independence, and compile-time cost |
| FMV-07 | Exact constant integer-to-floating conversion | With the IEEE evaluator; before floating arithmetic folding | Foundation needed / **Medium to large** | Low to medium runtime | Must implement specified round-to-nearest behavior without inheriting host or target quirks |
| FMV-08 | Stronger dead-definition elimination using callable effect facts | After effect summaries; repeated after inlining and CFG cleanup | Foundation needed / **Large** | High code size and medium runtime | Calls, queries, loads, allocation, failure, cleanup, destruction, and ownership cannot be classified by result use alone |
| FMV-09 | Scalar promotion of eligible local storage to transient values or block parameters | Before global scalar optimization; after alias and lifetime facts | Foundation needed / **Large to extra large** | High runtime and large architectural unlock | Mutable aliases, address-taken storage, initialization, joins, loops, storage epochs, proof records, and debug/runtime identity |
| FMV-10 | Sparse conditional constant propagation | On an SSA-capable MIR or normalized optimization IR; before CFG cleanup | Foundation needed / **Large** after SSA, **extra large** including it | High runtime and code size | Current values are block-local and cross-block state is storage; lattice must include executable edges, checked failure, and semantic barriers |
| FMV-11 | Global value numbering and global common-subexpression elimination | After scalar SSA/promotion and effect analysis; before loop passes | Foundation needed / **Large** after foundations | High runtime | Memory versions, calls, failure, dominance, floating semantics, ownership, and proof-bearing operations |

## Final-MIR checked-operation and control-flow graph

These candidates change `BlockId` edges or exact checked shapes. They belong
after scalar facts have exposed constant conditions and before dead-pure
cleanup and final whole-world retention. Candidates beyond the first two will
probably require proof-provenance normalization.

| ID | Candidate | Placement and ordering | Status / effort | Potential value | Main pitfalls |
|---|---|---|---|---|---|
| FMC-16 | [Ordinary branch folding and unprotected unreachable-block cleanup](../compiler/PHASES_AND_IR.md#local-final-mir-simplification) | After repeated scalar simplification; before final dead-pure cleanup and whole-world retention | Implemented / **Large** | Medium runtime/code size and high proof of CFG-rewrite architecture | Dedicated checked terminators remain unchanged; body entry, static publication, lifecycle, and proof-metadata blocks are protected roots |
| FMC-01 | Fold constant integer division and remainder with a known nonzero divisor | After primitive constant folding; before general CFG cleanup | In progress ([roadmap](CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_ROADMAP.md#cir3--fold-constant-integer-division-and-remainder-protocols)) / **Medium to large** | Medium runtime and code size | Must compute Skald floor-division/divisor-sign remainder including `i64::MIN / -1`, rewrite the divisor-check diamond coherently, preserve spans and evaluation, and remove only safe failure regions |
| FMC-02 | Fold constant shifts with an in-range constant count | After primitive constant folding; before general CFG cleanup | In progress ([roadmap](CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_ROADMAP.md#cir4--fold-constant-integer-shift-protocols)) / **Medium to large** | Medium runtime and code size | Must preserve arithmetic/logical shift flavor and `u8` canonicalization while rewriting the exact shift-count check protocol |
| FMC-03 | Eliminate an always-successful divisor or shift check when only the checked RHS is constant | After checked constant protocol folding; before instruction selection once an unchecked/proven operation representation exists | Foundation needed / **Large** | Medium to high runtime in guarded dynamic arithmetic | The operation remains dynamic, so MIR needs an accepted proof for an unchecked operation or a normalized post-proof representation; simply removing the terminator violates current verification |
| FMC-04 | Fold constant checked `f64`-to-integer conversions | After exact IEEE/range evaluation; before CFG cleanup | Foundation needed / **Large** | Low to medium runtime and size | Range, finiteness, truncation, target-width result, exact failure reason, and cast-range diamond must be rewritten together |
| FMC-05 | Simplify statically decidable checked casts and type tests missed by lowering | After whole-world type/dispatch facts; before CFG cleanup | Foundation needed / **Medium to large** | Medium runtime and code size | Dynamic class sets, access views, checked carriers, failure blocks, ownership, and complete-object provenance |
| FMC-06 | Simplify statically decidable optional presence and unwrap diamonds | After optional-state analysis; before CFG cleanup | Foundation needed / **Large** | Medium runtime and code size | Optional representation, guard counts, pinned mutation, payload lifetime, cleanup, and exact absence/overflow failure behavior |
| FMC-07 | Delete obsolete path-condition and logical-expression proof records | In a named normalization stage after their final semantic verifier; before broad CFG passes | Foundation needed / **Large** | High architectural value; medium MIR/compile-time reduction | Must classify metadata as semantic, consumed proof, or recomputable analysis and establish a new verified post-normalization product |
| FMC-08 | Empty-block forwarding | After FMC-07; before block merging | Foundation needed / **Medium** | Medium code size and compile time | Exact predecessor roles, storage epochs, cleanup joins, static publication endpoints, and runtime trace spans |
| FMC-09 | Basic-block merging | After proof normalization and empty-block forwarding; before jump threading | Foundation needed / **Medium** | Medium code size and target input quality | Lifetime and ownership state, fallthrough spans, terminator semantics, and deterministic block order |
| FMC-10 | Jump threading and branch-to-branch folding | After proof normalization and scalar propagation; before unreachable-region deletion | Foundation needed / **Large** | Medium to high runtime and code size | Path predicates, duplicated predecessors, cleanup/ownership joins, loop edges, and code-size growth |
| FMC-11 | Short-circuit logical CFG simplification | After logical proof records are consumed or rewritable; before general CFG cleanup | Foundation needed / **Large** | Medium runtime and code size | Exactly-once RHS evaluation, selected-result storage, path conditions, cleanup, and observable failure suppression |
| FMC-12 | Complete unreachable proof-region deletion | After proof normalization; after branch folding and before dead-pure cleanup | Foundation needed / **Medium to large** | Medium MIR size and compile time | Must remove records, blocks, values, storage, guards, and attachments as one coherent unit |
| FMC-13 | Loop canonicalization and natural-loop discovery | After proof normalization and preferably scalar promotion; before loop optimizations | Foundation needed / **Large** | High architectural value | Irreducible CFG, array-generated loops, cleanup edges, failure exits, ownership joins, and stable loop identity |
| FMC-14 | Loop-invariant scalar code motion | After FMC-13 plus effect/alias facts; before induction simplification | Foundation needed / **Large** | High runtime in numeric and array code | Hoisting may change failure timing, destructor timing, loads through aliases, runtime traces, and zero-iteration behavior |
| FMC-15 | Induction-variable simplification and bounds-check reduction | After loop canonicalization, scalar SSA, and range analysis | Foundation needed / **Large to extra large** | High runtime for arrays and ranges | Wrapping induction, negative indices, array length mutation/aliasing, exact bounds failures, and proof of every loop exit |

## Final-MIR storage, alias, effect, and ownership graph

These candidates reason about `StorageId`, places, calls, ownership operations,
and observable lifecycle. Whole-world and single-threaded execution improve
precision, but do not themselves prove two aliases distinct or a load stable.

| ID | Candidate | Placement and ordering | Status / effort | Potential value | Main pitfalls |
|---|---|---|---|---|---|
| FMM-01 | Whole-program callable effect summaries | Seal-scoped analysis after reachability target resolution; before memory and call transformations | Foundation needed / **Large** | Very high architectural value | Must conservatively summarize reads, writes, capture, failure, allocation, ownership, destruction, external calls, recursion, and dynamic targets |
| FMM-02 | Abstract memory-region and `may_alias` queries | After basic effect summaries; consumed by memory optimizations | Foundation needed / **Large** | Very high architectural value | Local, static, field, array, allocation-site, shared-pointee, and unknown regions; projections and call-scoped aliases; deterministic widening |
| FMM-03 | Store-to-load forwarding | After FMM-01/FMM-02; before redundant-load elimination | Foundation needed / **Medium to large** | High runtime and stack-traffic reduction | Intervening aliases, calls, mutable shared pointees, initialization, type/view projections, and failure paths |
| FMM-04 | Redundant-load elimination | After region/effect facts; before local CSE | Foundation needed / **Medium to large** | High runtime with current storage-heavy MIR | Same barriers as forwarding plus storage epochs and path joins |
| FMM-05 | Dead-store elimination | After region/effect and liveness facts; repeated after inlining | Foundation needed / **Large** | High runtime and code size | Stores may be observed through aliases, calls, cleanup, destructors, statics, I/O, failure paths, or later partial projections |
| FMM-06 | Loop-invariant load motion | After loop canonicalization and effect/alias facts | Foundation needed / **Large** | High runtime | Zero-trip loops, failure timing, calls, aliases, mutable shared pointees, array backing stability, and lifetime anchors |
| FMM-07 | Scalar replacement of eligible aggregates or addressable locals | After escape, alias, initialization, and lifecycle analysis | Foundation needed / **Large to extra large** | High runtime and enables SSA | Partial initialization, projections, address exposure, copying, destruction, optional payloads, arrays, and exact object identity |
| FMM-08 | Redundant shared retain/release pair elimination | After ownership and last-owner analysis; before backend lowering | Foundation needed / **Large** | Medium to high runtime | Individual count changes are hidden, but moving the last release changes destructor timing; overflow and static immortal owners must remain correct |
| FMM-09 | Anchor lifetime shortening and redundant anchor elimination | After view/escape/ownership analysis; before frame planning | Foundation needed / **Large** | Medium runtime and frame-size reduction | Checked views, shared/array backing, calls, cleanup order, failure exits, and last-owner timing |
| FMM-10 | Copy-to-move or copy-elision transformation | After exact source-death and ownership analysis; before cleanup simplification | Foundation needed / **Large** | High runtime for class/shared values | User copy operations may be observable, moved-from state is not a general language concept, and destructor/copy failure behavior must remain identical |
| FMM-11 | Stack allocation or complete elimination of non-escaping shared allocations | After escape and ownership analysis | Contract decision / **Extra large** | Potentially very high runtime | Removing heap allocation also removes language-observable allocation failure and may change object identity, destruction, runtime traces, and ABI expectations |
| FMM-12 | Runtime alias-versioned call-site specialization | After points-to/effect facts and cloning infrastructure; before inlining | Research / **Extra large** | High runtime where actual aliases are usually distinct | Requires a sound overlap check, two equivalent paths, code-size policy, cleanup duplication, and exact handling of projected/array ranges |

## Whole-world execution and call graph

These candidates consume the implemented target-independent reachability
graph and finite possible-target queries. They should generally run before the
last local simplification/CFG cleanup cycle and before the final
whole-world-retention occurrence.

| ID | Candidate | Placement and ordering | Status / effort | Potential value | Main pitfalls |
|---|---|---|---|---|---|
| WWE-12 | [Whole-world unreachable executable-definition elimination](../compiler/PHASES_AND_IR.md#target-independent-whole-world-reachability) | Final pass in the current default target-independent schedule; before backend legality and lowering | Implemented / **Large** | High code size and compile-time reduction; strong whole-world foundation | Roots and finite targets must include static lifecycle, dispatch, function values, copies, destruction, arrays, shared ownership, and external boundaries |
| WWE-01 | More precise reachable-type and dispatch-target analysis | After conservative reachability; before devirtualization | Foundation needed / **Large** | High runtime, code size, and enabling value | Object construction, static/shared storage, casts, external boundaries, recursion, and function-value targets require conservative closure |
| WWE-02 | Monomorphic virtual and interface call devirtualization | After WWE-01; before inlining and local simplification | Foundation needed / **Medium to large** | High runtime and enables inlining | Receiver adjustment, selected declarations, witness/family metadata, class initialization, visibility, and exact target availability |
| WWE-03 | Exact indirect function-value call resolution | After flow-sensitive function-value target analysis; before inlining | Foundation needed / **Large** | Medium to high runtime | Function values in storage, joins, arguments, returns, external calls, recursion, and exact signature/ABI identity |
| WWE-04 | Conservative callable inlining | After target resolution and effect summaries; before local scalar/CFG passes | Foundation needed / **Extra large** for the first robust implementation | Very high runtime and broad enabling value | Cross-callable identity import exists, but returns, storage, cleanup, ownership, failure spans, runtime traces, recursion, code-size policy, and static effects must compose |
| WWE-05 | Interprocedural constant propagation and constant-argument specialization | After precise target resolution; before or together with inlining | Foundation needed / **Large to extra large** | High runtime and code size | Cloning policy, recursive SCCs, function values, dynamic targets, diagnostics/traces, code growth, and storage/alias barriers |
| WWE-06 | Call-site specialization for exact dynamic class | After reachable-type analysis; before devirtualization/inlining | Foundation needed / **Large** | High runtime in polymorphic code | Must retain the general path where class is not proven exact and preserve receiver views and failure behavior |
| WWE-07 | Eliminate calls proven total and effect-free when their result is unused | After FMM-01; before dead-pure cleanup | Foundation needed / **Medium to large** | Medium to high runtime and code size | “Pure” must exclude failure, allocation, I/O, static effects, ownership, destruction, runtime trace effects, and opaque external work |
| WWE-08 | Internal dead-parameter and dead-result elimination | After whole-world target/signature analysis; before inlining or final lowering | Foundation needed / **Large** | Medium runtime, code size, and ABI simplification | Function-type identities, indirect calls, virtual/interface slots, external ABI, ownership modes, object-result destinations, and sparse global identities |
| WWE-09 | Recursive SCC specialization and bounded unrolling | After call-graph SCC and cost analysis | Research / **Large to extra large** | Medium runtime in small recursive kernels | Code explosion, termination assumptions, stack behavior, failure spans, cleanup, and runtime traces |
| WWE-10 | Identical internal callable body folding | After final target-independent optimization; before backend lowering | Research / **Large** | Medium code size and compile time | Function identity, callable addresses, runtime traces, panic locations, static ownership, dispatch metadata, and future equality/reflection semantics |
| WWE-11 | Prune or compact unused semantic declarations and global metadata | After final reachability retention; before backend metadata planning | Foundation needed / **Large to extra large** | High compile time and output size in large programs | Global IDs are deliberately stable and dense metadata cross-references types, classes, functions, dispatch, lifecycle, traces, and dumps |

## Static-lifecycle and static-dependency graph

The reachability-gated static-lifecycle contract already removes inactive
static fields from startup and shutdown. These candidates optimize retained
active statics or their dependency schedule.

| ID | Candidate | Placement and ordering | Status / effort | Potential value | Main pitfalls |
|---|---|---|---|---|---|
| SLD-05 | [Reachability-gated static activation and shutdown](../compiler/PHASES_AND_IR.md#frozen-reachability-gated-static-lifecycle-direction) | After preliminary-MIR verification and dependency analysis; before lifecycle planning and final-MIR optimization | Implemented / **Large** | High startup/shutdown and code-size value for loaded but unused statics | Mandatory roots must reflect non-static entry reachability, function values, dispatch, ownership, generated lifecycle work, and exact reverse shutdown |
| SLD-01 | Propagate values from provably immutable initialized static scalar fields | After active-static selection and whole-program write analysis; before local MIR folding | Foundation needed / **Large** | Medium runtime and code size | Initialization order, reads during initialization, external calls, aliases, mutable shared pointees, shutdown, and preserving the static's required initialization effects |
| SLD-02 | Remove retained static initialization whose value and all effects are dead | After effect analysis and active-static reachability; before lifecycle realization is finalized | Foundation needed / **Large** | Medium startup time and code size | Constructor/call failure, allocation, I/O, mutation of other statics, ownership, cleanup, and shutdown obligations |
| SLD-03 | Compile-time materialization of active static initializers | After a deterministic constant-evaluation contract; before final MIR | Contract decision / **Extra large** | High startup reduction for eligible programs | Allocation failure, object identity, addresses, dynamic dispatch, destruction, runtime trace, host independence, and serialized representation |
| SLD-04 | Simplify initialization/shutdown coordinator CFG after active-field gating | After lifecycle realization and final-MIR constant/CFG passes; before backend lowering | Follow-up / **Medium** | Medium code size and compile time | Shutdown is a separate entry region; publication endpoints and exact reverse-destruction order are protected |

## Target-private low-level value and control-flow graph

Skald currently lowers verified MIR directly into physical-register-oriented
x86-64 assembly with fixed stack homes. Most candidates in this group become
practical only after introducing a target-private virtual-register LIR. That
foundation is itself listed because it is the placement boundary for a large
family of optimizations, not because it should be disguised as a pass.

| ID | Candidate | Placement and ordering | Status / effort | Potential value | Main pitfalls |
|---|---|---|---|---|---|
| TLI-01 | Target-private virtual-register LIR with verification | After verified optimized final MIR; before physical assembly | Foundation needed / **Extra large** | Very high architectural and runtime value | ABI, calls, clobbers, failure/reporting pseudo-operations, ownership calls, traces, typed integer/floating registers, CFG, dumps, and lowering parity |
| TLI-02 | Register allocation with spilling | After TLI-01 instruction selection; before physical encoding | Foundation needed / **Large to extra large** | Very high runtime; removes systematic stack traffic | Register classes, fixed ABI operands, calls, loops, spill insertion, rematerialization, exceptional termination, and deterministic allocation |
| TLI-03 | Copy coalescing and redundant move elimination | Around register allocation | Foundation needed / **Medium to large** | High runtime and code quality | Interference, register classes, fixed operands, debug/runtime identity, and spill interactions |
| TLI-04 | Target instruction combining and addressing-mode selection | After target lowering; before allocation when profitable, finalized afterward | Foundation needed / **Large** | High runtime and code size | Flag dependencies, signedness/width, memory effects, immediate ranges, call boundaries, and interaction with allocation |
| TLI-05 | Target-specific constant materialization and strength reduction | After target lowering; before allocation | Foundation needed / **Medium** | Medium runtime and code size | Cost is architecture-specific; must preserve wrapping, floating, flag, and checked-failure semantics |
| TLI-06 | Machine CFG branch inversion, fallthrough selection, and block layout | After target CFG construction; before emission | Foundation needed / **Medium** | Medium runtime and code size | Runtime traces, cold failure blocks, loop layout, deterministic output, and branch-distance constraints |
| TLI-07 | Instruction scheduling | After instruction selection; before or integrated with allocation | Foundation needed / **Large** | Medium to high runtime on suitable targets | Memory/alias dependencies, calls, flags, failure and trace barriers, target latency model, and compile-time cost |
| TLI-08 | Stack-slot coloring and frame compaction | After liveness and spill placement; before frame finalization | Foundation needed / **Medium to large** | Medium runtime and frame-size reduction | Address-taken storage, lifetime epochs, alignment, arrays/aggregates, outgoing calls, trace state, and cleanup |
| TLI-09 | Post-allocation peephole optimization | After allocation and frame layout; before rendering | Foundation needed / **Medium** | Medium code size and runtime | Must use a closed instruction-effect model; flags, stack maps/traces, branch targets, and unwind-like failure sequences are barriers |
| TLI-10 | SIMD/vectorization of eligible array loops | After loop, alias, alignment, and target-feature analysis | Research / **Extra large** | Potentially very high numeric runtime | Array bounds/failure semantics, aliasing, tails, alignment, floating behavior, ownership, target features, and code-size policy |
| TLI-11 | Tail-call and sibling-call lowering | After final call-target selection and frame/liveness planning | Contract decision / **Large** | Medium runtime and stack use | Cleanup/destruction, static shutdown, ABI compatibility, runtime stack traces, panic traces, and tail-recursive source observability |

## Final machine-artifact dependency graph

The backend already performs closed-world retention for target-generated
symbols and data. These candidates operate after semantic MIR identities have
been lowered and should remain target-private.

| ID | Candidate | Placement and ordering | Status / effort | Potential value | Main pitfalls |
|---|---|---|---|---|---|
| ART-05 | [Closed-world target artifact retention](../compiler/BACKEND.md#assembly-emission-and-artifact-retention) | After target lowering has introduced helpers, symbols, trace metadata, and data; immediately before final assembly publication | Implemented / **Medium** | High output-size safety net | Must retain every exported or transitively referenced target artifact, including dependencies with no target-independent MIR identity |
| ART-01 | Deduplicate identical immutable literal and metadata payloads | After target metadata construction; before symbol emission | Follow-up / **Medium** | Medium output size | Address identity, alignment, relocation kind, mutability, linkage, runtime trace identity, and deterministic canonical owner |
| ART-02 | Compact dispatch and witness tables after semantic reachability | After final MIR reachability and target layout; before emission | Foundation needed / **Large** | High output size and cache locality | Stable slots and ABI consumers, external visibility, class/type metadata, receiver adjustment, and conservative dynamic target support |
| ART-03 | Identical machine-code folding | After final machine optimization; before symbol publication | Research / **Large** | Medium output size | Symbol/address identity, runtime traces, panic locations, relocations, alignment, exported symbols, and future debugging/reflection |
| ART-04 | Profile-independent hot/cold partitioning of failure regions | After machine CFG layout; before emission | Research / **Medium to large** | Medium instruction-cache value | No profile data exists initially; preserve deterministic layout, branch distances, traces, and exact failure messages |

## Cross-cutting architecture candidates

These are not optimization passes. They are representation or analysis
boundaries that unlock multiple catalog entries and should receive their own
design proposals before implementation.

| Candidate | Primary consumers | Effort | Expected leverage | Main decision |
|---|---|---|---|---|
| Proof-provenance classification and post-proof normalization | FMC-03 through FMC-15; some inlining | **Large** | High | Which metadata remains semantic after final verification, which is consumed, and what verifier seals the normalized product? |
| Conservative whole-program effect summaries | FMV-08, FMM-03 through FMM-12, WWE-04/WWE-07, SLD-01/SLD-02 | **Large** | Very high | What regions and observable effects form the first sound summary lattice? |
| Points-to, alias, escape, and ownership analysis | Memory, loop, specialization, allocation, retain/release candidates | **Large to extra large** | Very high | How much flow/context sensitivity is justified, and how are recursive/dynamic targets widened deterministically? |
| Scalar SSA or normalized optimization IR | FMV-09 through FMV-11 and advanced loops | **Extra large** | High | Extend MIR with block parameters or maintain a separate optimizer-facing scalar IR? |
| Target virtual-register LIR | TLI-02 through TLI-11 | **Extra large** | Very high runtime | What is the minimal verified target IR that preserves ABI, trace, failure, and effect barriers? |
| Deterministic target-independent IEEE evaluator | FMV-05 through FMV-07 and FMC-04 | **Large** | Medium | Which exact NaN payload, rounding, and conversion contract must compiled constants follow? |

## Suggested evaluation order

This is not a roadmap. It is a default order for deciding which candidate is
worth designing next now that the implemented local-simplification layer has
produced initial measurements:

1. Implement the proposed
   [checked integer constant protocol simplification roadmap](CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_ROADMAP.md),
   because its arithmetic is bounded and it extends local simplification while
   exposing the first dedicated-terminator rewrite.
2. Measure remaining local redundancy and consider redundant cast elimination
   or local primitive common-subexpression elimination.
3. Decide whether proof-provenance normalization is justified by blocked CFG
   candidates rather than implementing isolated metadata rewrites in every
   pass.
4. Build conservative callable effect summaries before attempting memory,
   ownership, pure-call, or aggressive inlining transformations.
5. Improve reachable-type/target precision, then devirtualize before designing
   general inlining.
6. Treat the target virtual-register LIR and register allocator as a separate
   major performance program once target-independent simplification is stable.
7. Introduce scalar SSA or a normalized optimization IR only after measurements
   show that storage boundaries, rather than backend stack homes, are the next
   dominant ceiling.

When measurements contradict this order, update the catalog rather than
preserving stale priority by inertia.
